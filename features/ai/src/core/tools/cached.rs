/// CachedTool decorator: wraps any Tool and caches ReadOnly results.
///
/// Uses `agent_cache::ToolResultCache` to store successful results keyed
/// by tool name + arguments. Non-cacheable tools (HighRisk, LowRisk, etc.)
/// pass through transparently — the decorator is safe to wrap all tools.
///
/// Cache errors degrade to cache miss (fail-open), never blocking execution.

use std::any::Any;
use std::sync::Arc;

use agent_cache::ToolResultCache;
use async_trait::async_trait;
use serde_json::Value;
use tracing;

use tool::{RiskLevel, Tool, ToolDefinition, ToolOutput, ToolResult};

/// A decorator that caches results from ReadOnly tools.
///
/// Wraps an inner `Tool` and delegates all trait methods transparently.
/// On `execute()`, checks the cache first for cacheable tools; on miss,
/// delegates to the inner tool and stores successful results.
pub struct CachedTool {
    inner: Box<dyn Tool>,
    cache: Arc<ToolResultCache>,
}

impl CachedTool {
    /// Create a new CachedTool wrapping the given tool with a shared cache.
    pub fn new(inner: Box<dyn Tool>, cache: Arc<ToolResultCache>) -> Self {
        Self { inner, cache }
    }
}

#[async_trait]
impl Tool for CachedTool {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    fn risk_level(&self) -> RiskLevel {
        self.inner.risk_level()
    }

    async fn execute(&self, args: Value) -> ToolResult<ToolOutput> {
        // Non-cacheable tools pass through immediately
        if !self.cache.is_cacheable(self.inner.risk_level()) {
            return self.inner.execute(args).await;
        }

        // Check cache
        if let Some(cached) = self.cache.get(self.inner.name(), &args) {
            tracing::debug!(tool = self.inner.name(), "cache hit");
            return Ok(cached);
        }

        // Cache miss — execute and store on success
        tracing::debug!(tool = self.inner.name(), "cache miss");
        let result = self.inner.execute(args.clone()).await;

        if let Ok(ref output) = result {
            self.cache.set(self.inner.name(), &args, output.clone());
        }

        result
    }

    fn to_definition(&self) -> ToolDefinition {
        self.inner.to_definition()
    }

    fn default_timeout_ms(&self) -> u64 {
        self.inner.default_timeout_ms()
    }

    fn requires_confirmation(&self) -> bool {
        self.inner.requires_confirmation()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_cache::CacheConfig;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tool::ToolError;

    /// A mock tool that counts executions.
    struct CountingTool {
        name: String,
        risk: RiskLevel,
        call_count: Arc<AtomicUsize>,
    }

    impl CountingTool {
        fn new(name: &str, risk: RiskLevel) -> (Self, Arc<AtomicUsize>) {
            let count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    name: name.to_string(),
                    risk,
                    call_count: count.clone(),
                },
                count,
            )
        }
    }

    #[async_trait]
    impl Tool for CountingTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A counting test tool"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                }
            })
        }

        fn risk_level(&self) -> RiskLevel {
            self.risk
        }

        async fn execute(&self, _args: Value) -> ToolResult<ToolOutput> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(ToolOutput::text("result"))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    /// A mock tool that always fails.
    struct FailingTool;

    #[async_trait]
    impl Tool for FailingTool {
        fn name(&self) -> &str {
            "failing"
        }

        fn description(&self) -> &str {
            "Always fails"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }

        fn risk_level(&self) -> RiskLevel {
            RiskLevel::ReadOnly
        }

        async fn execute(&self, _args: Value) -> ToolResult<ToolOutput> {
            Err(ToolError::ExecutionFailed("boom".into()))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    fn make_cache(ttl: Duration) -> Arc<ToolResultCache> {
        Arc::new(ToolResultCache::new(
            CacheConfig::with_ttl(ttl).with_max_entries(100),
        ))
    }

    #[tokio::test]
    async fn readonly_tool_cached_on_second_call() {
        let (tool, count) = CountingTool::new("fs", RiskLevel::ReadOnly);
        let cache = make_cache(Duration::from_secs(300));
        let cached = CachedTool::new(Box::new(tool), cache);

        let args = serde_json::json!({"path": "/tmp/foo"});
        let r1 = cached.execute(args.clone()).await.unwrap();
        let r2 = cached.execute(args).await.unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(r1.content, r2.content);
    }

    #[tokio::test]
    async fn highrisk_tool_never_cached() {
        let (tool, count) = CountingTool::new("exec", RiskLevel::HighRisk);
        let cache = make_cache(Duration::from_secs(300));
        let cached = CachedTool::new(Box::new(tool), cache);

        let args = serde_json::json!({"cmd": "ls"});
        cached.execute(args.clone()).await.unwrap();
        cached.execute(args).await.unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn different_args_produce_different_entries() {
        let (tool, count) = CountingTool::new("fs", RiskLevel::ReadOnly);
        let cache = make_cache(Duration::from_secs(300));
        let cached = CachedTool::new(Box::new(tool), cache);

        let args_a = serde_json::json!({"path": "/a"});
        let args_b = serde_json::json!({"path": "/b"});

        cached.execute(args_a.clone()).await.unwrap();
        cached.execute(args_b.clone()).await.unwrap();
        // Both should be cached now — re-execute both
        cached.execute(args_a).await.unwrap();
        cached.execute(args_b).await.unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 2); // only 2 real executions
    }

    #[tokio::test]
    async fn cache_ttl_expiration() {
        let (tool, count) = CountingTool::new("fs", RiskLevel::ReadOnly);
        let cache = make_cache(Duration::from_millis(50));
        let cached = CachedTool::new(Box::new(tool), cache);

        let args = serde_json::json!({"path": "/tmp/foo"});
        cached.execute(args.clone()).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        cached.execute(args).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cached_tool_preserves_name_and_definition() {
        let (tool, _) = CountingTool::new("my_tool", RiskLevel::ReadOnly);
        let cache = make_cache(Duration::from_secs(300));
        let cached = CachedTool::new(Box::new(tool), cache);

        assert_eq!(cached.name(), "my_tool");
        assert_eq!(cached.description(), "A counting test tool");
        assert_eq!(cached.risk_level(), RiskLevel::ReadOnly);

        let def = cached.to_definition();
        assert_eq!(def.name, "my_tool");
    }

    #[tokio::test]
    async fn failed_results_not_cached() {
        let cache = make_cache(Duration::from_secs(300));
        let cached = CachedTool::new(Box::new(FailingTool), cache.clone());

        let args = serde_json::json!({"path": "/nope"});
        assert!(cached.execute(args.clone()).await.is_err());

        // Cache should be empty — errors are not stored
        assert!(cache.is_empty());
    }
}
