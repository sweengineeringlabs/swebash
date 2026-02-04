/// Tool calling infrastructure (from rustratify)
///
/// This module re-exports tool types from rustratify's tool crate
/// for use in swebash-ai.

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
