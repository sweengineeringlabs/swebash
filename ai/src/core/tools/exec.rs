/// Command execution tool implementation
///
/// Executes shell commands with safety checks and timeouts.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use llm_provider::ToolDefinition;
use super::{ToolExecutor, ToolError, ToolResult};

/// Command executor tool with safety checks
pub struct CommandExecutorTool {
    default_timeout: u64,
}

impl CommandExecutorTool {
    pub fn new(default_timeout: u64) -> Self {
        Self { default_timeout }
    }

    /// Check if command contains dangerous patterns
    fn is_dangerous_command(&self, command: &str) -> bool {
        let dangerous_patterns = [
            "rm -rf",
            "rm -fr",
            "rm -r",
            "dd if=",
            "mkfs",
            "format",
            "> /dev/",
            "sudo",
            "su ",
            "chmod 777",
            "chown",
            ":(){:|:&};:", // Fork bomb
        ];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return true;
            }
        }

        false
    }

    async fn execute_command(&self, command: &str, timeout_seconds: Option<u64>) -> ToolResult<String> {
        // Validate command length
        if command.len() > 10000 {
            return Err(ToolError::InvalidArguments(
                "Command too long (max 10000 characters)".to_string()
            ));
        }

        // Check for dangerous commands
        if self.is_dangerous_command(command) {
            return Err(ToolError::PermissionDenied(format!(
                "Dangerous command detected: {}",
                command
            )));
        }

        // Determine timeout
        let timeout_secs = timeout_seconds.unwrap_or(self.default_timeout);
        if timeout_secs > 300 {
            return Err(ToolError::InvalidArguments(
                "Timeout too long (max 300 seconds)".to_string()
            ));
        }

        // Execute command with timeout
        let start = std::time::Instant::now();

        let output_result = timeout(
            Duration::from_secs(timeout_secs),
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
        ).await;

        let duration_ms = start.elapsed().as_millis();

        let output = match output_result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Failed to execute command: {}",
                    e
                )));
            }
            Err(_) => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Command timed out after {} seconds",
                    timeout_secs
                )));
            }
        };

        // Limit output size
        let max_output_size = 100_000; // 100KB
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let stdout_truncated = if stdout.len() > max_output_size {
            format!("{}... (truncated)", &stdout[..max_output_size])
        } else {
            stdout.to_string()
        };

        let stderr_truncated = if stderr.len() > max_output_size {
            format!("{}... (truncated)", &stderr[..max_output_size])
        } else {
            stderr.to_string()
        };

        Ok(json!({
            "success": output.status.success(),
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": stdout_truncated,
            "stderr": stderr_truncated,
            "duration_ms": duration_ms
        }).to_string())
    }
}

#[derive(Debug, Deserialize)]
struct CommandArgs {
    command: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

#[async_trait]
impl ToolExecutor for CommandExecutorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "execute_command".to_string(),
            description: "Execute a shell command and return its output. Use this to run terminal commands, check system status, or perform system operations. Commands run in a shell environment with a configurable timeout.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute (will be run with 'sh -c')"
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Maximum execution time in seconds (default: 30, max: 300)",
                        "default": 30
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> ToolResult<String> {
        let args: CommandArgs = serde_json::from_str(arguments)?;
        self.execute_command(&args.command, args.timeout_seconds).await
    }

    fn requires_confirmation(&self) -> bool {
        // Dangerous commands are blocked, so no need for confirmation
        false
    }

    fn describe_call(&self, arguments: &str) -> String {
        if let Ok(args) = serde_json::from_str::<CommandArgs>(arguments) {
            format!("Execute command: {}", args.command)
        } else {
            "Execute command".to_string()
        }
    }
}
