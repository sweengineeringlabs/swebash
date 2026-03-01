/// Tool calling infrastructure using llmboot.
///
/// This module provides swebash-specific tools and sandbox functionality.
pub mod devops;
pub mod error;
pub mod sandboxed;
pub mod web;

pub use error::{ErrorCategory, IntoToolError};
pub use sandboxed::{SandboxedTool, ToolSandbox, SandboxAccessMode, SandboxRule};

// Re-export llmboot's tool types
pub use llmboot_orchestration::{
    Tool,
    ToolDefinition,
    LocalToolRepository as ToolRegistry,
    ToolOutput,
    ToolError,
    ToolExecResult as ToolResult,
    RiskLevel,
};
