/// File system tool implementation
///
/// Provides read-only file operations for the LLM:
/// - read: Read file contents
/// - list: List directory contents
/// - exists: Check if path exists
/// - metadata: Get file metadata

use std::path::PathBuf;
use std::fs;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use llm_provider::ToolDefinition;
use super::{ToolExecutor, ToolError, ToolResult};

/// File system tool for read-only operations
pub struct FileSystemTool {
    max_size: usize,
}

impl FileSystemTool {
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }

    /// Validate path for security
    fn validate_path(&self, path: &str) -> ToolResult<PathBuf> {
        let path = PathBuf::from(path);

        // Resolve to absolute path
        let abs_path = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()
                .map_err(|e| ToolError::Io(e))?
                .join(&path)
        };

        // Canonicalize to resolve .. and symlinks
        let canonical = abs_path.canonicalize()
            .map_err(|e| ToolError::ExecutionFailed(format!("Path not found: {}", e)))?;

        // Check for sensitive paths (blacklist)
        let path_str = canonical.to_string_lossy();
        let sensitive_paths = [
            "/etc/passwd",
            "/etc/shadow",
            "/etc/sudoers",
            "/.ssh/",
            "/root/.ssh/",
        ];

        for sensitive in &sensitive_paths {
            if path_str.contains(sensitive) {
                return Err(ToolError::PermissionDenied(format!(
                    "Access denied: {} is a sensitive path",
                    sensitive
                )));
            }
        }

        Ok(canonical)
    }

    fn read_file(&self, path: &str) -> ToolResult<String> {
        let validated_path = self.validate_path(path)?;

        // Check if file exists and is a file
        if !validated_path.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "Path does not exist: {}",
                path
            )));
        }

        if !validated_path.is_file() {
            return Err(ToolError::ExecutionFailed(format!(
                "Path is not a file: {}",
                path
            )));
        }

        // Check file size
        let metadata = fs::metadata(&validated_path)?;
        if metadata.len() > self.max_size as u64 {
            return Err(ToolError::ExecutionFailed(format!(
                "File too large: {} bytes (max: {} bytes)",
                metadata.len(),
                self.max_size
            )));
        }

        // Read file contents
        let content = fs::read_to_string(&validated_path)
            .map_err(|e| ToolError::ExecutionFailed(format!(
                "Failed to read file as UTF-8: {}",
                e
            )))?;

        Ok(json!({
            "success": true,
            "path": validated_path.to_string_lossy(),
            "content": content,
            "size": metadata.len()
        }).to_string())
    }

    fn list_directory(&self, path: &str) -> ToolResult<String> {
        let validated_path = self.validate_path(path)?;

        if !validated_path.is_dir() {
            return Err(ToolError::ExecutionFailed(format!(
                "Path is not a directory: {}",
                path
            )));
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(&validated_path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let file_type = if metadata.is_dir() {
                "directory"
            } else if metadata.is_file() {
                "file"
            } else if metadata.is_symlink() {
                "symlink"
            } else {
                "other"
            };

            entries.push(json!({
                "name": entry.file_name().to_string_lossy(),
                "type": file_type,
                "size": metadata.len()
            }));
        }

        Ok(json!({
            "success": true,
            "path": validated_path.to_string_lossy(),
            "entries": entries
        }).to_string())
    }

    fn check_exists(&self, path: &str) -> ToolResult<String> {
        let validated_path = self.validate_path(path);

        let exists = validated_path.is_ok() && validated_path.as_ref().unwrap().exists();

        Ok(json!({
            "success": true,
            "exists": exists,
            "path": path
        }).to_string())
    }

    fn get_metadata(&self, path: &str) -> ToolResult<String> {
        let validated_path = self.validate_path(path)?;

        if !validated_path.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "Path does not exist: {}",
                path
            )));
        }

        let metadata = fs::metadata(&validated_path)?;

        Ok(json!({
            "success": true,
            "path": validated_path.to_string_lossy(),
            "is_file": metadata.is_file(),
            "is_dir": metadata.is_dir(),
            "size": metadata.len(),
            "read_only": metadata.permissions().readonly(),
            "modified": metadata.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
        }).to_string())
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "lowercase")]
enum FileSystemOperation {
    Read { path: String },
    List { path: String },
    Exists { path: String },
    Metadata { path: String },
}

#[async_trait]
impl ToolExecutor for FileSystemTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "filesystem".to_string(),
            description: "Read, list, and inspect files and directories. Can read file contents, list directory contents, check if files exist, and get file metadata. All operations are read-only.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["read", "list", "exists", "metadata"],
                        "description": "The operation to perform: read (file contents), list (directory contents), exists (check if path exists), metadata (file info)"
                    },
                    "path": {
                        "type": "string",
                        "description": "The file or directory path (absolute or relative to current directory)"
                    }
                },
                "required": ["operation", "path"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> ToolResult<String> {
        let operation: FileSystemOperation = serde_json::from_str(arguments)?;

        let result = match operation {
            FileSystemOperation::Read { path } => self.read_file(&path),
            FileSystemOperation::List { path } => self.list_directory(&path),
            FileSystemOperation::Exists { path } => self.check_exists(&path),
            FileSystemOperation::Metadata { path } => self.get_metadata(&path),
        };

        result
    }

    fn requires_confirmation(&self) -> bool {
        false // Read-only operations don't require confirmation
    }

    fn describe_call(&self, arguments: &str) -> String {
        if let Ok(op) = serde_json::from_str::<FileSystemOperation>(arguments) {
            match op {
                FileSystemOperation::Read { path } => format!("Read file: {}", path),
                FileSystemOperation::List { path } => format!("List directory: {}", path),
                FileSystemOperation::Exists { path } => format!("Check if exists: {}", path),
                FileSystemOperation::Metadata { path } => format!("Get metadata: {}", path),
            }
        } else {
            "Filesystem operation".to_string()
        }
    }
}
