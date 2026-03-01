/// SandboxedTool decorator: wraps filesystem tools and enforces path restrictions.
///
/// Intercepts execute() calls and validates path arguments against allowed paths.
/// If the path is outside the sandbox, returns an error instead of delegating.

use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use serde_json::Value;

use llmboot_orchestration::{RiskLevel, Tool, ToolDefinition, ToolOutput, ToolExecResult as ToolResult, ToolError};

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
///
/// The `cwd` field tracks the shell's current working directory for resolving
/// relative paths. This should be updated whenever the shell's virtual_cwd changes.
pub struct ToolSandbox {
    /// Allowed paths with their access modes.
    pub rules: Vec<SandboxRule>,
    /// Whether the sandbox is enabled.
    pub enabled: bool,
    /// Current working directory for resolving relative paths.
    /// This should match the shell's virtual_cwd, not std::env::current_dir().
    cwd: RwLock<PathBuf>,
}

impl Default for ToolSandbox {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            enabled: false,
            cwd: RwLock::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))),
        }
    }
}

impl std::fmt::Debug for ToolSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolSandbox")
            .field("rules", &self.rules)
            .field("enabled", &self.enabled)
            .field("cwd", &self.cwd.read().ok())
            .finish()
    }
}

impl Clone for ToolSandbox {
    fn clone(&self) -> Self {
        Self {
            rules: self.rules.clone(),
            enabled: self.enabled,
            cwd: RwLock::new(
                self.cwd
                    .read()
                    .map(|g| g.clone())
                    .unwrap_or_else(|_| PathBuf::from("/")),
            ),
        }
    }
}

impl ToolSandbox {
    /// Create a new sandbox with a single workspace root.
    pub fn new(workspace_root: PathBuf, mode: SandboxAccessMode) -> Self {
        let cwd = workspace_root.clone();
        Self {
            rules: vec![SandboxRule { path: workspace_root, mode }],
            enabled: true,
            cwd: RwLock::new(cwd),
        }
    }

    /// Create a sandbox with explicit rules and cwd.
    pub fn with_rules_and_cwd(rules: Vec<SandboxRule>, enabled: bool, cwd: PathBuf) -> Self {
        Self {
            rules,
            enabled,
            cwd: RwLock::new(cwd),
        }
    }

    /// Update the current working directory for path resolution.
    ///
    /// Call this whenever the shell's virtual_cwd changes so that relative
    /// paths in AI tool arguments are resolved correctly.
    pub fn set_cwd(&self, cwd: PathBuf) {
        if let Ok(mut guard) = self.cwd.write() {
            *guard = cwd;
        }
    }

    /// Get the current working directory.
    pub fn cwd(&self) -> PathBuf {
        self.cwd
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|_| PathBuf::from("/"))
    }

    /// Check if a path is allowed for the given access mode.
    pub fn check_path(&self, path: &Path, needs_write: bool) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        // Normalize the path using the sandbox's cwd
        let cwd = self.cwd();
        let normalized = normalize_path(path, &cwd);

        for rule in &self.rules {
            let rule_path = normalize_path(&rule.path, &cwd);
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
///
/// Uses the provided `cwd` to resolve relative paths, rather than
/// std::env::current_dir(). This allows the AI sandbox to respect
/// the shell's virtual_cwd.
///
/// Resolves `.` and `..` components without touching the filesystem.
fn normalize_path(path: &Path, cwd: &Path) -> PathBuf {
    // Convert to absolute using the provided cwd
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    // Resolve . and .. components without filesystem access
    let mut components = Vec::new();
    for component in abs.components() {
        match component {
            std::path::Component::ParentDir => {
                // Pop the last component if possible
                components.pop();
            }
            std::path::Component::CurDir => {
                // Skip . components
            }
            c => {
                components.push(c);
            }
        }
    }

    let resolved: PathBuf = components.iter().collect();

    // Convert backslashes to forward slashes and lowercase for comparison
    let s = resolved.to_string_lossy().replace('\\', "/").to_lowercase();
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

    /// Extract path from tool arguments, check sandbox, and rewrite to absolute paths.
    ///
    /// Returns the modified args with relative paths resolved to absolute paths
    /// using the sandbox's cwd.
    fn check_and_rewrite_args(&self, args: &Value, needs_write: bool) -> Result<Value, ToolError> {
        // Common path field names used by filesystem tools
        let path_fields = ["path", "file_path", "directory", "dir", "source", "destination", "target"];
        let cwd = self.sandbox.cwd();

        let mut modified_args = args.clone();
        let mut obj = modified_args.as_object_mut();

        for field in path_fields {
            if let Some(path_str) = args.get(field).and_then(|v| v.as_str()) {
                let path = Path::new(path_str);
                self.sandbox
                    .check_path(path, needs_write)
                    .map_err(|e| ToolError::ExecutionFailed(e))?;

                // Rewrite relative paths to absolute paths using sandbox cwd
                if !path.is_absolute() {
                    let abs_path = cwd.join(path);
                    if let Some(ref mut map) = obj {
                        map.insert(
                            field.to_string(),
                            Value::String(abs_path.to_string_lossy().into_owned()),
                        );
                    }
                }
            }
        }

        // Check array fields (for tools that take multiple paths)
        if let Some(paths) = args.get("paths").and_then(|v| v.as_array()) {
            let mut new_paths = Vec::new();
            for p in paths {
                if let Some(path_str) = p.as_str() {
                    let path = Path::new(path_str);
                    self.sandbox
                        .check_path(path, needs_write)
                        .map_err(|e| ToolError::ExecutionFailed(e))?;

                    // Rewrite relative paths
                    if !path.is_absolute() {
                        let abs_path = cwd.join(path);
                        new_paths.push(Value::String(abs_path.to_string_lossy().into_owned()));
                    } else {
                        new_paths.push(p.clone());
                    }
                } else {
                    new_paths.push(p.clone());
                }
            }
            if let Some(ref mut map) = obj {
                map.insert("paths".to_string(), Value::Array(new_paths));
            }
        }

        Ok(modified_args)
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
        // Check sandbox restrictions and rewrite relative paths to absolute
        let needs_write = self.needs_write(&args);
        let rewritten_args = self.check_and_rewrite_args(&args, needs_write)?;

        // Path is allowed and rewritten, delegate to inner tool
        self.inner.execute(rewritten_args).await
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
        let cwd = PathBuf::from(r"C:\");
        let normalized = normalize_path(path, &cwd);
        assert!(normalized.to_string_lossy().contains("/"));
        assert!(!normalized.to_string_lossy().contains("\\"));
    }

    #[test]
    fn sandbox_multiple_allowed_paths() {
        let sandbox = ToolSandbox::with_rules_and_cwd(
            vec![
                SandboxRule {
                    path: PathBuf::from("/home/user/project1"),
                    mode: SandboxAccessMode::ReadWrite,
                },
                SandboxRule {
                    path: PathBuf::from("/home/user/project2"),
                    mode: SandboxAccessMode::ReadOnly,
                },
            ],
            true,
            PathBuf::from("/home/user"),
        );

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
    fn sandboxed_tool_check_and_rewrite_args_extracts_path() {
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
        assert!(tool.check_and_rewrite_args(&args, false).is_ok());

        // Path outside workspace should fail
        let args = json!({"path": "/etc/passwd"});
        assert!(tool.check_and_rewrite_args(&args, false).is_err());

        // file_path field should also be checked
        let args = json!({"file_path": "/etc/passwd"});
        assert!(tool.check_and_rewrite_args(&args, false).is_err());

        // directory field should also be checked
        let args = json!({"directory": "/etc"});
        assert!(tool.check_and_rewrite_args(&args, false).is_err());
    }

    #[test]
    fn sandboxed_tool_rewrites_relative_paths_to_absolute() {
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

        // Create sandbox with cwd at /workspace/src
        let sandbox = Arc::new(ToolSandbox::with_rules_and_cwd(
            vec![SandboxRule {
                path: PathBuf::from("/workspace"),
                mode: SandboxAccessMode::ReadWrite,
            }],
            true,
            PathBuf::from("/workspace/src"),
        ));

        let tool = SandboxedTool::new(Box::new(MockTool), sandbox);

        // Relative path should be rewritten to absolute
        let args = json!({"path": "main.rs"});
        let rewritten = tool.check_and_rewrite_args(&args, false).unwrap();
        // On Windows, paths may have backslashes, so check contains
        let path_value = rewritten.get("path").unwrap().as_str().unwrap();
        assert!(
            path_value.contains("workspace") && path_value.contains("src") && path_value.contains("main.rs"),
            "Expected absolute path containing workspace/src/main.rs, got: {}",
            path_value
        );

        // Absolute path should remain unchanged
        let args = json!({"path": "/workspace/file.txt"});
        let rewritten = tool.check_and_rewrite_args(&args, false).unwrap();
        assert_eq!(rewritten.get("path").unwrap().as_str().unwrap(), "/workspace/file.txt");

        // Multiple path fields should all be rewritten
        let args = json!({"source": "input.txt", "destination": "output.txt"});
        let rewritten = tool.check_and_rewrite_args(&args, false).unwrap();
        let source = rewritten.get("source").unwrap().as_str().unwrap();
        let dest = rewritten.get("destination").unwrap().as_str().unwrap();
        assert!(source.contains("workspace"), "source should be absolute: {}", source);
        assert!(dest.contains("workspace"), "destination should be absolute: {}", dest);
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

    #[test]
    fn sandbox_set_cwd_updates_path_resolution() {
        let sandbox = ToolSandbox::new(
            PathBuf::from("/workspace"),
            SandboxAccessMode::ReadWrite,
        );

        // Initially cwd is /workspace (same as root)
        assert_eq!(sandbox.cwd(), PathBuf::from("/workspace"));

        // Relative path "file.txt" should resolve to /workspace/file.txt
        assert!(sandbox.check_path(Path::new("file.txt"), false).is_ok());

        // Update cwd to a subdirectory
        sandbox.set_cwd(PathBuf::from("/workspace/src"));
        assert_eq!(sandbox.cwd(), PathBuf::from("/workspace/src"));

        // Relative path "file.txt" should still resolve to /workspace/src/file.txt (inside workspace)
        assert!(sandbox.check_path(Path::new("file.txt"), false).is_ok());

        // Set cwd to outside workspace - relative paths should fail
        sandbox.set_cwd(PathBuf::from("/other"));
        assert!(sandbox.check_path(Path::new("file.txt"), false).is_err());

        // But absolute paths inside workspace should still work
        assert!(sandbox.check_path(Path::new("/workspace/file.txt"), false).is_ok());
    }

    #[test]
    fn sandbox_relative_path_resolution() {
        let sandbox = ToolSandbox::with_rules_and_cwd(
            vec![SandboxRule {
                path: PathBuf::from("/project"),
                mode: SandboxAccessMode::ReadWrite,
            }],
            true,
            PathBuf::from("/project/src"),
        );

        // Relative paths are resolved against cwd (/project/src)
        assert!(sandbox.check_path(Path::new("main.rs"), false).is_ok());          // /project/src/main.rs
        assert!(sandbox.check_path(Path::new("../lib.rs"), false).is_ok());        // /project/lib.rs
        assert!(sandbox.check_path(Path::new("../../other/file"), false).is_err()); // /other/file - outside
    }

    #[test]
    fn sandbox_cwd_defaults_to_workspace_root() {
        let sandbox = ToolSandbox::new(
            PathBuf::from("/my/project"),
            SandboxAccessMode::ReadOnly,
        );

        // cwd should default to the workspace root
        assert_eq!(sandbox.cwd(), PathBuf::from("/my/project"));
    }

    #[test]
    fn sandboxed_tool_rewrites_dot_path_to_cwd() {
        use std::sync::Arc;

        struct MockTool;

        #[async_trait]
        impl Tool for MockTool {
            fn name(&self) -> &str { "list_directory" }
            fn description(&self) -> &str { "List directory" }
            fn parameters_schema(&self) -> Value { json!({}) }
            fn risk_level(&self) -> RiskLevel { RiskLevel::ReadOnly }
            async fn execute(&self, _args: Value) -> ToolResult<ToolOutput> {
                Ok(ToolOutput::text("ok"))
            }
            fn as_any(&self) -> &dyn std::any::Any { self }
        }

        // Create sandbox with cwd at /workspace/features/shell
        let sandbox = Arc::new(ToolSandbox::with_rules_and_cwd(
            vec![SandboxRule {
                path: PathBuf::from("/workspace"),
                mode: SandboxAccessMode::ReadWrite,
            }],
            true,
            PathBuf::from("/workspace/features/shell"),
        ));

        let tool = SandboxedTool::new(Box::new(MockTool), sandbox);

        // "." should be rewritten to the cwd
        let args = json!({"path": "."});
        let rewritten = tool.check_and_rewrite_args(&args, false).unwrap();
        let path_value = rewritten.get("path").unwrap().as_str().unwrap();
        assert!(
            path_value.contains("workspace") && path_value.contains("features") && path_value.contains("shell"),
            "Expected '.' to resolve to cwd (/workspace/features/shell), got: {}",
            path_value
        );

        // ".." should resolve to parent directory
        let args = json!({"directory": ".."});
        let rewritten = tool.check_and_rewrite_args(&args, false).unwrap();
        let dir_value = rewritten.get("directory").unwrap().as_str().unwrap();
        assert!(
            dir_value.contains("workspace") && dir_value.contains("features"),
            "Expected '..' to resolve to parent (/workspace/features), got: {}",
            dir_value
        );
    }
}
