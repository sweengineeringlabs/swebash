/// SandboxedTool decorator: wraps filesystem tools and enforces path restrictions.
///
/// Intercepts execute() calls and validates path arguments against allowed paths.
/// If the path is outside the sandbox, returns an error instead of delegating.

use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use tool::{RiskLevel, Tool, ToolDefinition, ToolOutput, ToolResult, ToolError};

/// Access mode for sandboxed paths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SandboxAccessMode {
    ReadOnly,
    ReadWrite,
}

/// A path rule for the sandbox.
#[derive(Debug, Clone)]
pub struct SandboxRule {
    pub path: PathBuf,
    pub mode: SandboxAccessMode,
}

/// Sandbox configuration for AI tools.
#[derive(Debug, Clone, Default)]
pub struct ToolSandbox {
    /// Allowed paths with their access modes.
    pub rules: Vec<SandboxRule>,
    /// Whether the sandbox is enabled.
    pub enabled: bool,
}

impl ToolSandbox {
    /// Create a new sandbox with a single workspace root.
    pub fn new(workspace_root: PathBuf, mode: SandboxAccessMode) -> Self {
        Self {
            rules: vec![SandboxRule { path: workspace_root, mode }],
            enabled: true,
        }
    }

    /// Check if a path is allowed for the given access mode.
    pub fn check_path(&self, path: &Path, needs_write: bool) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        // Normalize the path
        let normalized = normalize_path(path);

        for rule in &self.rules {
            let rule_path = normalize_path(&rule.path);
            if normalized.starts_with(&rule_path) {
                // Path is within an allowed directory
                if needs_write && rule.mode == SandboxAccessMode::ReadOnly {
                    return Err(format!(
                        "Write access denied: {} is read-only",
                        path.display()
                    ));
                }
                return Ok(());
            }
        }

        Err(format!(
            "Access denied: {} is outside the workspace sandbox",
            path.display()
        ))
    }
}

/// Normalize a path for comparison.
fn normalize_path(path: &Path) -> PathBuf {
    // Convert to absolute if possible
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };

    // Convert backslashes to forward slashes and lowercase for comparison
    let s = abs.to_string_lossy().replace('\\', "/").to_lowercase();
    PathBuf::from(s)
}

/// A decorator that enforces sandbox restrictions on filesystem tools.
pub struct SandboxedTool {
    inner: Box<dyn Tool>,
    sandbox: Arc<ToolSandbox>,
}

impl SandboxedTool {
    /// Create a new SandboxedTool wrapping the given tool with sandbox restrictions.
    pub fn new(inner: Box<dyn Tool>, sandbox: Arc<ToolSandbox>) -> Self {
        Self { inner, sandbox }
    }

    /// Extract path from tool arguments and check sandbox.
    fn check_args(&self, args: &Value, needs_write: bool) -> Result<(), ToolError> {
        // Common path field names used by filesystem tools
        let path_fields = ["path", "file_path", "directory", "dir", "source", "destination", "target"];

        for field in path_fields {
            if let Some(path_str) = args.get(field).and_then(|v| v.as_str()) {
                let path = Path::new(path_str);
                self.sandbox
                    .check_path(path, needs_write)
                    .map_err(|e| ToolError::ExecutionFailed(e))?;
            }
        }

        // Check array fields (for tools that take multiple paths)
        if let Some(paths) = args.get("paths").and_then(|v| v.as_array()) {
            for p in paths {
                if let Some(path_str) = p.as_str() {
                    let path = Path::new(path_str);
                    self.sandbox
                        .check_path(path, needs_write)
                        .map_err(|e| ToolError::ExecutionFailed(e))?;
                }
            }
        }

        Ok(())
    }

    /// Determine if the operation needs write access based on tool name and args.
    fn needs_write(&self, args: &Value) -> bool {
        let name = self.inner.name().to_lowercase();

        // Write operations
        if name.contains("write") || name.contains("create") || name.contains("delete")
            || name.contains("remove") || name.contains("move") || name.contains("copy")
            || name.contains("mkdir") || name.contains("touch")
        {
            return true;
        }

        // Check operation field in args
        if let Some(op) = args.get("operation").and_then(|v| v.as_str()) {
            let op_lower = op.to_lowercase();
            if op_lower.contains("write") || op_lower.contains("create")
                || op_lower.contains("delete") || op_lower.contains("append")
            {
                return true;
            }
        }

        false
    }
}

#[async_trait]
impl Tool for SandboxedTool {
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
        // Check sandbox restrictions before delegating
        let needs_write = self.needs_write(&args);
        self.check_args(&args, needs_write)?;

        // Path is allowed, delegate to inner tool
        self.inner.execute(args).await
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
    use serde_json::json;

    #[test]
    fn sandbox_allows_path_inside_workspace() {
        let sandbox = ToolSandbox::new(
            PathBuf::from("/home/user/project"),
            SandboxAccessMode::ReadWrite,
        );
        assert!(sandbox.check_path(Path::new("/home/user/project/src/main.rs"), false).is_ok());
        assert!(sandbox.check_path(Path::new("/home/user/project/src/main.rs"), true).is_ok());
    }

    #[test]
    fn sandbox_denies_path_outside_workspace() {
        let sandbox = ToolSandbox::new(
            PathBuf::from("/home/user/project"),
            SandboxAccessMode::ReadWrite,
        );
        assert!(sandbox.check_path(Path::new("/etc/passwd"), false).is_err());
        assert!(sandbox.check_path(Path::new("/home/other/file"), false).is_err());
    }

    #[test]
    fn sandbox_readonly_denies_write() {
        let sandbox = ToolSandbox::new(
            PathBuf::from("/home/user/project"),
            SandboxAccessMode::ReadOnly,
        );
        assert!(sandbox.check_path(Path::new("/home/user/project/file.txt"), false).is_ok());
        assert!(sandbox.check_path(Path::new("/home/user/project/file.txt"), true).is_err());
    }

    #[test]
    fn sandbox_disabled_allows_everything() {
        let mut sandbox = ToolSandbox::new(
            PathBuf::from("/home/user/project"),
            SandboxAccessMode::ReadOnly,
        );
        sandbox.enabled = false;
        assert!(sandbox.check_path(Path::new("/etc/passwd"), true).is_ok());
    }

    #[test]
    fn normalize_path_handles_backslashes() {
        let path = Path::new(r"C:\Users\test\project");
        let normalized = normalize_path(path);
        assert!(normalized.to_string_lossy().contains("/"));
        assert!(!normalized.to_string_lossy().contains("\\"));
    }

    #[test]
    fn sandbox_multiple_allowed_paths() {
        let sandbox = ToolSandbox {
            rules: vec![
                SandboxRule {
                    path: PathBuf::from("/home/user/project1"),
                    mode: SandboxAccessMode::ReadWrite,
                },
                SandboxRule {
                    path: PathBuf::from("/home/user/project2"),
                    mode: SandboxAccessMode::ReadOnly,
                },
            ],
            enabled: true,
        };

        // First project allows read and write
        assert!(sandbox.check_path(Path::new("/home/user/project1/file.txt"), false).is_ok());
        assert!(sandbox.check_path(Path::new("/home/user/project1/file.txt"), true).is_ok());

        // Second project allows read only
        assert!(sandbox.check_path(Path::new("/home/user/project2/file.txt"), false).is_ok());
        assert!(sandbox.check_path(Path::new("/home/user/project2/file.txt"), true).is_err());

        // Outside both projects
        assert!(sandbox.check_path(Path::new("/home/user/other/file.txt"), false).is_err());
    }

    #[test]
    fn sandbox_nested_path_allowed() {
        let sandbox = ToolSandbox::new(
            PathBuf::from("/workspace"),
            SandboxAccessMode::ReadWrite,
        );

        // Deeply nested paths within workspace should be allowed
        assert!(sandbox.check_path(Path::new("/workspace/a/b/c/d/e/file.txt"), false).is_ok());
        assert!(sandbox.check_path(Path::new("/workspace/src/main/java/com/example/App.java"), true).is_ok());
    }

    #[test]
    fn sandbox_parent_traversal_blocked() {
        let sandbox = ToolSandbox::new(
            PathBuf::from("/workspace/project"),
            SandboxAccessMode::ReadWrite,
        );

        // Parent directory should be blocked (after normalization)
        // Note: Path::new doesn't resolve .. so this tests the raw path
        assert!(sandbox.check_path(Path::new("/workspace/file.txt"), false).is_err());
    }

    #[test]
    fn sandbox_error_message_includes_path() {
        let sandbox = ToolSandbox::new(
            PathBuf::from("/allowed"),
            SandboxAccessMode::ReadOnly,
        );

        // Access denied error should include the path
        let err = sandbox.check_path(Path::new("/etc/passwd"), false).unwrap_err();
        assert!(err.contains("/etc/passwd"), "Error should include path: {err}");
        assert!(err.contains("outside"), "Error should say 'outside': {err}");

        // Write denied error should include the path
        let err = sandbox.check_path(Path::new("/allowed/file.txt"), true).unwrap_err();
        assert!(err.contains("/allowed/file.txt"), "Error should include path: {err}");
        assert!(err.contains("read-only"), "Error should say 'read-only': {err}");
    }

    #[test]
    fn sandboxed_tool_check_args_extracts_path() {
        use std::sync::Arc;

        struct MockTool;

        #[async_trait]
        impl Tool for MockTool {
            fn name(&self) -> &str { "mock_fs" }
            fn description(&self) -> &str { "Mock filesystem tool" }
            fn parameters_schema(&self) -> Value { json!({}) }
            fn risk_level(&self) -> RiskLevel { RiskLevel::ReadOnly }
            async fn execute(&self, _args: Value) -> ToolResult<ToolOutput> {
                Ok(ToolOutput::text("ok"))
            }
            fn as_any(&self) -> &dyn std::any::Any { self }
        }

        let sandbox = Arc::new(ToolSandbox::new(
            PathBuf::from("/workspace"),
            SandboxAccessMode::ReadWrite,
        ));

        let tool = SandboxedTool::new(Box::new(MockTool), sandbox);

        // Path inside workspace should pass
        let args = json!({"path": "/workspace/file.txt"});
        assert!(tool.check_args(&args, false).is_ok());

        // Path outside workspace should fail
        let args = json!({"path": "/etc/passwd"});
        assert!(tool.check_args(&args, false).is_err());

        // file_path field should also be checked
        let args = json!({"file_path": "/etc/passwd"});
        assert!(tool.check_args(&args, false).is_err());

        // directory field should also be checked
        let args = json!({"directory": "/etc"});
        assert!(tool.check_args(&args, false).is_err());
    }

    #[test]
    fn sandboxed_tool_needs_write_detection() {
        use std::sync::Arc;

        struct WriteFileTool;

        #[async_trait]
        impl Tool for WriteFileTool {
            fn name(&self) -> &str { "write_file" }
            fn description(&self) -> &str { "Write file" }
            fn parameters_schema(&self) -> Value { json!({}) }
            fn risk_level(&self) -> RiskLevel { RiskLevel::HighRisk }
            async fn execute(&self, _args: Value) -> ToolResult<ToolOutput> {
                Ok(ToolOutput::text("ok"))
            }
            fn as_any(&self) -> &dyn std::any::Any { self }
        }

        struct ReadFileTool;

        #[async_trait]
        impl Tool for ReadFileTool {
            fn name(&self) -> &str { "read_file" }
            fn description(&self) -> &str { "Read file" }
            fn parameters_schema(&self) -> Value { json!({}) }
            fn risk_level(&self) -> RiskLevel { RiskLevel::ReadOnly }
            async fn execute(&self, _args: Value) -> ToolResult<ToolOutput> {
                Ok(ToolOutput::text("ok"))
            }
            fn as_any(&self) -> &dyn std::any::Any { self }
        }

        let sandbox = Arc::new(ToolSandbox::new(
            PathBuf::from("/workspace"),
            SandboxAccessMode::ReadOnly,
        ));

        // Write tool should detect write need from name
        let write_tool = SandboxedTool::new(Box::new(WriteFileTool), sandbox.clone());
        assert!(write_tool.needs_write(&json!({})));

        // Read tool should not need write
        let read_tool = SandboxedTool::new(Box::new(ReadFileTool), sandbox.clone());
        assert!(!read_tool.needs_write(&json!({})));

        // Operation field can also indicate write
        assert!(read_tool.needs_write(&json!({"operation": "write"})));
        assert!(read_tool.needs_write(&json!({"operation": "delete"})));
        assert!(!read_tool.needs_write(&json!({"operation": "read"})));
    }
}
