/// L3 Core: Tool execution infrastructure
///
/// This module provides the ToolExecutor SPI and ToolRegistry implementation
/// for managing and executing tools that the LLM can call.

pub mod fs;
pub mod exec;
pub mod web;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use llm_provider::ToolDefinition;
use thiserror::Error;

use crate::config::ToolConfig;

/// Result type for tool operations
pub type ToolResult<T> = Result<T, ToolError>;

/// Errors that can occur during tool execution
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Trait for executable tools that can be called by the LLM.
///
/// This is an internal SPI (not exposed to consumers) that allows
/// pluggable tool implementations.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Get the tool definition (name, description, JSON schema)
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments (JSON string)
    /// Returns JSON string result or error
    async fn execute(&self, arguments: &str) -> ToolResult<String>;

    /// Check if this tool requires user confirmation
    fn requires_confirmation(&self) -> bool {
        false
    }

    /// Get a human-readable description of what this tool call will do
    /// Used for confirmation prompts
    fn describe_call(&self, arguments: &str) -> String {
        format!("Call {} with {}", self.definition().name, arguments)
    }
}

/// Registry of available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolExecutor>>,
    /// Configuration for tool execution
    pub config: ToolConfig,
}

impl ToolRegistry {
    /// Create a new tool registry with the given configuration
    pub fn new(config: ToolConfig) -> Self {
        Self {
            tools: HashMap::new(),
            config,
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn ToolExecutor>) {
        let name = tool.definition().name.clone();
        self.tools.insert(name, tool);
    }

    /// Get all tool definitions for LLM request
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name
    pub async fn execute(&self, name: &str, arguments: &str) -> ToolResult<String> {
        let tool = self.tools.get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        tool.execute(arguments).await
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ToolExecutor>> {
        self.tools.get(name)
    }
}

/// Create a tool registry with all enabled tools based on config
pub fn create_tool_registry(config: &crate::config::AiConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new(config.tools.clone());

    if config.tools.enable_fs {
        registry.register(Arc::new(fs::FileSystemTool::new(
            config.tools.fs_max_size,
        )));
    }

    if config.tools.enable_exec {
        registry.register(Arc::new(exec::CommandExecutorTool::new(
            config.tools.exec_timeout,
        )));
    }

    if config.tools.enable_web {
        registry.register(Arc::new(web::WebSearchTool::new()));
    }

    registry
}
