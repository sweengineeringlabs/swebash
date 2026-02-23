/// Tool calling infrastructure (from rustratify)
///
/// This module re-exports tool types from rustratify's tool crate
/// for use in swebash-ai.
pub mod cached;
pub mod devops;
pub mod sandboxed;

pub use sandboxed::{SandboxedTool, ToolSandbox, SandboxAccessMode, SandboxRule};

pub use tool::{
    Tool,
    ToolRegistry,
    ToolOutput,
    ToolError,
    ToolResult,
    ToolDefinition,
    RiskLevel,
    ToolConfig,
    create_standard_registry,
};

use std::sync::Arc;

use agent_cache::{CacheConfig, ToolResultCache};

use cached::CachedTool;

/// Create a tool registry where each tool is wrapped in a `CachedTool` decorator.
///
/// Mirrors `create_standard_registry()` but wraps each tool in `CachedTool`
/// before registration. `CachedTool` transparently passes through non-cacheable
/// tools (those with risk levels other than `ReadOnly`), so all tools are
/// wrapped uniformly.
///
/// Returns the registry and a shared cache handle (the cache is also held
/// internally by each `CachedTool` wrapper via `Arc`).
pub fn create_cached_registry(
    config: &ToolConfig,
    cache_config: CacheConfig,
) -> (ToolRegistry, Arc<ToolResultCache>) {
    create_cached_registry_with_sandbox(config, cache_config, None)
}

/// Create a tool registry with optional sandbox restrictions.
///
/// When `sandbox` is provided, filesystem and command executor tools are wrapped
/// in a `SandboxedTool` decorator that enforces path restrictions.
pub fn create_cached_registry_with_sandbox(
    config: &ToolConfig,
    cache_config: CacheConfig,
    sandbox: Option<Arc<ToolSandbox>>,
) -> (ToolRegistry, Arc<ToolResultCache>) {
    let cache = Arc::new(ToolResultCache::new(cache_config));
    let mut registry = ToolRegistry::new();

    if config.enable_fs {
        let fs_tool: Box<dyn Tool> = Box::new(tool::providers::FileSystemTool::new(config.fs_max_size));
        let fs_tool = if let Some(ref sb) = sandbox {
            Box::new(SandboxedTool::new(fs_tool, sb.clone())) as Box<dyn Tool>
        } else {
            fs_tool
        };
        registry.register(Box::new(CachedTool::new(fs_tool, cache.clone())));
    }

    if config.enable_exec {
        let exec_tool: Box<dyn Tool> = Box::new(tool::providers::CommandExecutorTool::new(config.exec_timeout));
        let exec_tool = if let Some(ref sb) = sandbox {
            Box::new(SandboxedTool::new(exec_tool, sb.clone())) as Box<dyn Tool>
        } else {
            exec_tool
        };
        registry.register(Box::new(CachedTool::new(exec_tool, cache.clone())));
    }

    if config.enable_web {
        registry.register(Box::new(CachedTool::new(
            Box::new(tool::providers::WebSearchTool::new()),
            cache.clone(),
        )));
    }

    (registry, cache)
}
