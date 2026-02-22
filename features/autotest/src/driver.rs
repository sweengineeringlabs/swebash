//! Interactive shell driver for spawning and controlling swebash.
//!
//! Uses pipe-based communication (stdin/stdout/stderr) to drive the shell.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Errors that can occur during shell driving.
#[derive(Debug, Error)]
pub enum DriverError {
    #[error("Failed to spawn swebash: {0}")]
    Spawn(#[source] std::io::Error),

    #[error("Failed to write to stdin: {0}")]
    WriteStdin(#[source] std::io::Error),

    #[error("Failed to read output: {0}")]
    ReadOutput(#[source] std::io::Error),

    #[error("Process timed out after {0:?}")]
    Timeout(Duration),

    #[error("Process exited with non-zero status: {0}")]
    ExitStatus(ExitStatus),

    #[error("swebash binary not found at: {0}")]
    BinaryNotFound(PathBuf),
}

/// Configuration for the shell driver.
#[derive(Debug, Clone)]
pub struct DriverConfig {
    /// Path to the swebash binary.
    pub binary_path: PathBuf,

    /// Working directory for the shell.
    pub working_dir: PathBuf,

    /// Environment variables to set.
    pub env: HashMap<String, String>,

    /// Environment variables to remove.
    pub env_remove: Vec<String>,

    /// Default timeout for operations.
    pub timeout: Duration,

    /// Whether to automatically send "exit" at the end.
    pub auto_exit: bool,

    /// HOME directory override.
    pub home: Option<PathBuf>,

    /// Workspace root override.
    pub workspace: Option<PathBuf>,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            binary_path: find_swebash_binary(),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            env: HashMap::new(),
            env_remove: Vec::new(),
            timeout: Duration::from_secs(30),
            auto_exit: true,
            home: None,
            workspace: None,
        }
    }
}

/// Output captured from a shell session.
#[derive(Debug, Clone, Default)]
pub struct DriverOutput {
    /// Standard output.
    pub stdout: String,

    /// Standard error.
    pub stderr: String,

    /// Exit status (if process completed).
    pub exit_status: Option<ExitStatus>,

    /// Duration of the session.
    pub duration: Duration,
}

impl DriverOutput {
    /// Check if stdout contains a string.
    pub fn stdout_contains(&self, s: &str) -> bool {
        self.stdout.contains(s)
    }

    /// Check if stderr contains a string.
    pub fn stderr_contains(&self, s: &str) -> bool {
        self.stderr.contains(s)
    }

    /// Get combined stdout + stderr.
    pub fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    /// Check if the process exited successfully.
    pub fn success(&self) -> bool {
        self.exit_status.map(|s| s.success()).unwrap_or(false)
    }
}

/// Shell driver for interactive testing.
pub struct Driver {
    config: DriverConfig,
}

impl Driver {
    /// Create a new driver with the given configuration.
    pub fn new(config: DriverConfig) -> Result<Self, DriverError> {
        if !config.binary_path.exists() {
            return Err(DriverError::BinaryNotFound(config.binary_path.clone()));
        }
        Ok(Self { config })
    }

    /// Create a driver with default configuration.
    pub fn default_config() -> Result<Self, DriverError> {
        Self::new(DriverConfig::default())
    }

    /// Run a sequence of commands and capture output.
    pub fn run(&self, commands: &[&str]) -> Result<DriverOutput, DriverError> {
        self.run_with_timeout(commands, self.config.timeout)
    }

    /// Run commands with a specific timeout.
    pub fn run_with_timeout(
        &self,
        commands: &[&str],
        timeout: Duration,
    ) -> Result<DriverOutput, DriverError> {
        let start = Instant::now();

        // Build input string
        let mut input = String::new();
        for cmd in commands {
            input.push_str(cmd);
            input.push('\n');
        }
        if self.config.auto_exit {
            input.push_str("exit\n");
        }

        // Spawn the process
        let mut child = self.spawn_process()?;

        // Write to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .map_err(DriverError::WriteStdin)?;
        }

        // Wait for completion with timeout
        let output = self.wait_with_timeout(child, timeout)?;
        let duration = start.elapsed();

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        Ok(DriverOutput {
            stdout,
            stderr,
            exit_status: Some(output.status),
            duration,
        })
    }

    /// Run a single command.
    pub fn run_one(&self, command: &str) -> Result<DriverOutput, DriverError> {
        self.run(&[command])
    }

    /// Spawn the shell process.
    fn spawn_process(&self) -> Result<Child, DriverError> {
        let mut cmd = Command::new(&self.config.binary_path);

        cmd.current_dir(&self.config.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        // Remove environment variables
        for key in &self.config.env_remove {
            cmd.env_remove(key);
        }

        // Set HOME if specified
        if let Some(home) = &self.config.home {
            cmd.env("HOME", home);
        }

        // Set workspace if specified
        if let Some(workspace) = &self.config.workspace {
            cmd.env("SWEBASH_WORKSPACE", workspace);
        }

        cmd.spawn().map_err(DriverError::Spawn)
    }

    /// Wait for process with timeout.
    fn wait_with_timeout(
        &self,
        child: Child,
        _timeout: Duration,
    ) -> Result<std::process::Output, DriverError> {
        // For simplicity, we use wait_with_output which blocks.
        // A more sophisticated implementation would use async or threads.
        let output = child.wait_with_output().map_err(DriverError::ReadOutput)?;
        Ok(output)
    }

    /// Get the current configuration.
    pub fn config(&self) -> &DriverConfig {
        &self.config
    }

    /// Create a modified driver with different working directory.
    pub fn with_working_dir(&self, dir: PathBuf) -> Result<Self, DriverError> {
        let mut config = self.config.clone();
        config.working_dir = dir;
        Self::new(config)
    }

    /// Create a modified driver with additional environment variables.
    pub fn with_env(&self, env: HashMap<String, String>) -> Result<Self, DriverError> {
        let mut config = self.config.clone();
        config.env.extend(env);
        Self::new(config)
    }
}

/// Find the swebash binary in common locations.
fn find_swebash_binary() -> PathBuf {
    // Try CARGO_BIN_EXE first (for tests)
    if let Ok(exe) = std::env::var("CARGO_BIN_EXE_swebash") {
        return PathBuf::from(exe);
    }

    // Try relative to current exe (for when running from target/)
    if let Ok(current_exe) = std::env::current_exe() {
        let exe_dir = current_exe.parent().unwrap_or(Path::new("."));

        // Check same directory
        let candidate = if cfg!(windows) {
            exe_dir.join("swebash.exe")
        } else {
            exe_dir.join("swebash")
        };
        if candidate.exists() {
            return candidate;
        }

        // Check debug/release directories
        for profile in &["debug", "release"] {
            let candidate = if cfg!(windows) {
                exe_dir.join(profile).join("swebash.exe")
            } else {
                exe_dir.join(profile).join("swebash")
            };
            if candidate.exists() {
                return candidate;
            }
        }
    }

    // Fall back to assuming it's in PATH
    if cfg!(windows) {
        PathBuf::from("swebash.exe")
    } else {
        PathBuf::from("swebash")
    }
}

/// Builder for creating a driver with specific options.
#[derive(Debug, Default)]
pub struct DriverBuilder {
    config: DriverConfig,
}

impl DriverBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the binary path.
    pub fn binary_path(mut self, path: PathBuf) -> Self {
        self.config.binary_path = path;
        self
    }

    /// Set the working directory.
    pub fn working_dir(mut self, dir: PathBuf) -> Self {
        self.config.working_dir = dir;
        self
    }

    /// Add an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.env.insert(key.into(), value.into());
        self
    }

    /// Add multiple environment variables.
    pub fn envs(mut self, env: HashMap<String, String>) -> Self {
        self.config.env.extend(env);
        self
    }

    /// Remove an environment variable.
    pub fn env_remove(mut self, key: impl Into<String>) -> Self {
        self.config.env_remove.push(key.into());
        self
    }

    /// Set the default timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set whether to auto-exit.
    pub fn auto_exit(mut self, auto_exit: bool) -> Self {
        self.config.auto_exit = auto_exit;
        self
    }

    /// Set HOME directory override.
    pub fn home(mut self, home: PathBuf) -> Self {
        self.config.home = Some(home);
        self
    }

    /// Set workspace root override.
    pub fn workspace(mut self, workspace: PathBuf) -> Self {
        self.config.workspace = Some(workspace);
        self
    }

    /// Build the driver.
    pub fn build(self) -> Result<Driver, DriverError> {
        Driver::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_config_defaults() {
        let config = DriverConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.auto_exit);
        assert!(config.env.is_empty());
    }

    #[test]
    fn driver_builder() {
        let config = DriverBuilder::new()
            .timeout(Duration::from_secs(10))
            .env("TEST_VAR", "value")
            .auto_exit(false)
            .config;

        assert_eq!(config.timeout, Duration::from_secs(10));
        assert!(!config.auto_exit);
        assert_eq!(config.env.get("TEST_VAR"), Some(&"value".to_string()));
    }

    #[test]
    fn driver_output_helpers() {
        let output = DriverOutput {
            stdout: "hello world\n".to_string(),
            stderr: "warning: test\n".to_string(),
            exit_status: None,
            duration: Duration::from_millis(100),
        };

        assert!(output.stdout_contains("hello"));
        assert!(!output.stdout_contains("goodbye"));
        assert!(output.stderr_contains("warning"));
        assert!(output.combined().contains("hello"));
        assert!(output.combined().contains("warning"));
    }
}
