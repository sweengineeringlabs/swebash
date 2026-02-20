//! LLM Metrics - Observability for LLM operations
//!
//! Provides metrics collection for monitoring LLM API usage, costs, and performance.

use crate::api::TokenUsage;
use rustboot_observability::{Counter, Gauge, InMemoryMetrics, Metrics};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// LLM-specific metrics collector
pub struct LlmMetrics {
    inner: Arc<dyn Metrics>,
}

impl LlmMetrics {
    /// Create a new LLM metrics collector
    pub fn new(metrics: Arc<dyn Metrics>) -> Self {
        Self { inner: metrics }
    }

    /// Create with default in-memory metrics
    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryMetrics::new()))
    }

    /// Record a completion request
    pub fn record_request(&self, provider: &str, model: &str) {
        self.inner.counter("llm.requests.total").inc();
        self.inner
            .counter(&format!("llm.requests.{}", provider))
            .inc();
        self.inner
            .counter(&format!("llm.requests.{}.{}", provider, normalize_model(model)))
            .inc();
    }

    /// Record a successful completion
    pub fn record_success(&self, provider: &str) {
        self.inner.counter("llm.requests.success").inc();
        self.inner
            .counter(&format!("llm.requests.{}.success", provider))
            .inc();
    }

    /// Record a failed completion
    pub fn record_failure(&self, provider: &str, error_type: &str) {
        self.inner.counter("llm.requests.failure").inc();
        self.inner
            .counter(&format!("llm.requests.{}.failure", provider))
            .inc();
        self.inner
            .counter(&format!("llm.errors.{}", error_type))
            .inc();
    }

    /// Record request latency
    pub fn record_latency(&self, provider: &str, duration: Duration) {
        let ms = duration.as_millis() as f64;
        self.inner.histogram("llm.latency_ms", ms);
        self.inner
            .histogram(&format!("llm.latency_ms.{}", provider), ms);
    }

    /// Record token usage from a completion response
    pub fn record_tokens(&self, provider: &str, usage: &TokenUsage) {
        // Prompt tokens
        self.inner
            .counter("llm.tokens.prompt")
            .add(usage.prompt_tokens as u64);
        self.inner
            .counter(&format!("llm.tokens.{}.prompt", provider))
            .add(usage.prompt_tokens as u64);

        // Completion tokens
        self.inner
            .counter("llm.tokens.completion")
            .add(usage.completion_tokens as u64);
        self.inner
            .counter(&format!("llm.tokens.{}.completion", provider))
            .add(usage.completion_tokens as u64);

        // Total tokens
        self.inner
            .counter("llm.tokens.total")
            .add(usage.total_tokens as u64);

        // Cache tokens (Anthropic)
        if usage.cache_read_input_tokens > 0 {
            self.inner
                .counter("llm.tokens.cache_read")
                .add(usage.cache_read_input_tokens as u64);
        }
        if usage.cache_creation_input_tokens > 0 {
            self.inner
                .counter("llm.tokens.cache_write")
                .add(usage.cache_creation_input_tokens as u64);
        }
    }

    /// Record a streaming chunk
    pub fn record_stream_chunk(&self, provider: &str) {
        self.inner.counter("llm.stream.chunks").inc();
        self.inner
            .counter(&format!("llm.stream.{}.chunks", provider))
            .inc();
    }

    /// Record rate limit hit
    pub fn record_rate_limit(&self, provider: &str) {
        self.inner.counter("llm.rate_limits").inc();
        self.inner
            .counter(&format!("llm.rate_limits.{}", provider))
            .inc();
    }

    /// Record a retry attempt
    pub fn record_retry(&self, provider: &str, attempt: usize) {
        self.inner.counter("llm.retries").inc();
        self.inner
            .counter(&format!("llm.retries.{}", provider))
            .inc();
        self.inner
            .histogram("llm.retry_attempt", attempt as f64);
    }

    /// Get a counter for custom metrics
    pub fn counter(&self, name: &str) -> Box<dyn Counter> {
        self.inner.counter(name)
    }

    /// Get a gauge for custom metrics
    pub fn gauge(&self, name: &str) -> Box<dyn Gauge> {
        self.inner.gauge(name)
    }
}

impl Default for LlmMetrics {
    fn default() -> Self {
        Self::in_memory()
    }
}

/// Normalize model name for metric labels (remove special chars)
fn normalize_model(model: &str) -> String {
    model
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

/// Global metrics instance
static GLOBAL_METRICS: OnceLock<LlmMetrics> = OnceLock::new();

/// Get the global LLM metrics instance
pub fn global_metrics() -> &'static LlmMetrics {
    GLOBAL_METRICS.get_or_init(LlmMetrics::default)
}

/// Initialize global metrics with a custom implementation
pub fn init_global_metrics(metrics: Arc<dyn Metrics>) {
    let _ = GLOBAL_METRICS.set(LlmMetrics::new(metrics));
}

/// Helper to time an operation and record metrics
pub struct MetricsTimer {
    provider: String,
    #[allow(dead_code)] // Reserved for future per-model metrics
    model: String,
    start: Instant,
}

impl MetricsTimer {
    /// Start timing a request
    pub fn start(provider: &str, model: &str) -> Self {
        global_metrics().record_request(provider, model);
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            start: Instant::now(),
        }
    }

    /// Record success with token usage
    pub fn success(self, usage: &TokenUsage) {
        let duration = self.start.elapsed();
        let metrics = global_metrics();
        metrics.record_success(&self.provider);
        metrics.record_latency(&self.provider, duration);
        metrics.record_tokens(&self.provider, usage);
    }

    /// Record failure
    pub fn failure(self, error_type: &str) {
        let duration = self.start.elapsed();
        let metrics = global_metrics();
        metrics.record_failure(&self.provider, error_type);
        metrics.record_latency(&self.provider, duration);
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = LlmMetrics::in_memory();
        metrics.record_request("openai", "gpt-4");
        metrics.record_success("openai");
    }

    #[test]
    fn test_token_recording() {
        let metrics = LlmMetrics::in_memory();
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        metrics.record_tokens("anthropic", &usage);
    }

    #[test]
    fn test_latency_recording() {
        let metrics = LlmMetrics::in_memory();
        metrics.record_latency("gemini", Duration::from_millis(250));
    }

    #[test]
    fn test_normalize_model() {
        assert_eq!(normalize_model("gpt-4o"), "gpt-4o");
        assert_eq!(normalize_model("claude-3.5-sonnet"), "claude-3_5-sonnet");
        assert_eq!(normalize_model("gemini/1.5-pro"), "gemini_1_5-pro");
    }

    #[test]
    fn test_metrics_timer() {
        let timer = MetricsTimer::start("openai", "gpt-4");
        std::thread::sleep(Duration::from_millis(10));
        assert!(timer.elapsed().as_millis() >= 10);

        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        timer.success(&usage);
    }

    #[test]
    fn test_error_types() {
        let metrics = LlmMetrics::in_memory();
        metrics.record_failure("openai", "rate_limit");
        metrics.record_failure("openai", "network");
        metrics.record_failure("anthropic", "auth");
    }

    #[test]
    fn test_cache_tokens() {
        let metrics = LlmMetrics::in_memory();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 100,
            total_tokens: 1100,
            cache_read_input_tokens: 800,
            cache_creation_input_tokens: 200,
        };
        metrics.record_tokens("anthropic", &usage);
    }
}
