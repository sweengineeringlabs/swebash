//! Cross-platform package manager tool.
//!
//! Provides unified interface for system package management across different
//! platforms and package managers (apt, yum, dnf, brew, choco).

use std::any::Any;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info, instrument, warn};

use super::error::{IntoToolError, PackageManagerError};
use tool::{RiskLevel, Tool, ToolDefinition, ToolError, ToolOutput, ToolResult};

/// Detected package manager on the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Yum,
    Dnf,
    Brew,
    Choco,
}

impl PackageManager {
    /// Returns the command name for this package manager.
    fn command(&self) -> &'static str {
        match self {
            PackageManager::Apt => "apt-get",
            PackageManager::Yum => "yum",
            PackageManager::Dnf => "dnf",
            PackageManager::Brew => "brew",
            PackageManager::Choco => "choco",
        }
    }

    /// Returns human-readable name for this package manager.
    fn display_name(&self) -> &'static str {
        match self {
            PackageManager::Apt => "APT (Debian/Ubuntu)",
            PackageManager::Yum => "YUM (RHEL/CentOS)",
            PackageManager::Dnf => "DNF (Fedora)",
            PackageManager::Brew => "Homebrew (macOS)",
            PackageManager::Choco => "Chocolatey (Windows)",
        }
    }
}

/// Package operation to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageOperation {
    Install,
    Uninstall,
    Search,
    List,
    Update,
}

/// Arguments for the package manager tool.
#[derive(Debug, Deserialize)]
struct PackageManagerArgs {
    operation: PackageOperation,
    #[serde(default)]
    packages: Vec<String>,
}

/// Cross-platform package management tool.
///
/// Automatically detects the system's package manager and provides a unified
/// interface for common package operations.
pub struct PackageManagerTool {
    timeout: Duration,
}

impl PackageManagerTool {
    /// Create a new package manager tool with the specified timeout.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Detect the available package manager on the current system.
    async fn detect_package_manager() -> Option<PackageManager> {
        // Check platform-specific package managers first
        #[cfg(target_os = "windows")]
        {
            if Self::command_exists("choco").await {
                return Some(PackageManager::Choco);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if Self::command_exists("brew").await {
                return Some(PackageManager::Brew);
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Check DNF first (Fedora), then apt-get (Debian/Ubuntu), then yum (RHEL/CentOS)
            if Self::command_exists("dnf").await {
                return Some(PackageManager::Dnf);
            }
            if Self::command_exists("apt-get").await {
                return Some(PackageManager::Apt);
            }
            if Self::command_exists("yum").await {
                return Some(PackageManager::Yum);
            }
        }

        // Cross-platform fallback checks
        if Self::command_exists("brew").await {
            return Some(PackageManager::Brew);
        }
        if Self::command_exists("choco").await {
            return Some(PackageManager::Choco);
        }
        if Self::command_exists("dnf").await {
            return Some(PackageManager::Dnf);
        }
        if Self::command_exists("apt-get").await {
            return Some(PackageManager::Apt);
        }
        if Self::command_exists("yum").await {
            return Some(PackageManager::Yum);
        }

        None
    }

    /// Check if a command exists in PATH.
    async fn command_exists(cmd: &str) -> bool {
        #[cfg(target_os = "windows")]
        let check_cmd = format!("where {}", cmd);
        #[cfg(not(target_os = "windows"))]
        let check_cmd = format!("which {}", cmd);

        #[cfg(target_os = "windows")]
        let shell = "cmd";
        #[cfg(target_os = "windows")]
        let shell_arg = "/C";
        #[cfg(not(target_os = "windows"))]
        let shell = "sh";
        #[cfg(not(target_os = "windows"))]
        let shell_arg = "-c";

        Command::new(shell)
            .arg(shell_arg)
            .arg(&check_cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Build the command arguments for a package operation.
    fn build_command_args(
        pm: PackageManager,
        op: PackageOperation,
        packages: &[String],
    ) -> Result<Vec<String>, PackageManagerError> {
        let mut args = Vec::new();

        match (pm, op) {
            // APT
            (PackageManager::Apt, PackageOperation::Install) => {
                args.push("-y".to_string());
                args.push("install".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Apt, PackageOperation::Uninstall) => {
                args.push("-y".to_string());
                args.push("remove".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Apt, PackageOperation::Search) => {
                return Err(PackageManagerError::UnsupportedOperation {
                    package_manager: "apt-get".to_string(),
                    operation: "search".to_string(),
                    suggestion: "Use 'apt-cache search <package>' command directly".to_string(),
                });
            }
            (PackageManager::Apt, PackageOperation::List) => {
                return Err(PackageManagerError::UnsupportedOperation {
                    package_manager: "apt-get".to_string(),
                    operation: "list".to_string(),
                    suggestion: "Use 'dpkg -l' or 'apt list --installed' command directly".to_string(),
                });
            }
            (PackageManager::Apt, PackageOperation::Update) => {
                args.push("update".to_string());
            }

            // YUM
            (PackageManager::Yum, PackageOperation::Install) => {
                args.push("-y".to_string());
                args.push("install".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Yum, PackageOperation::Uninstall) => {
                args.push("-y".to_string());
                args.push("remove".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Yum, PackageOperation::Search) => {
                args.push("search".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Yum, PackageOperation::List) => {
                args.push("list".to_string());
                args.push("installed".to_string());
            }
            (PackageManager::Yum, PackageOperation::Update) => {
                args.push("-y".to_string());
                args.push("update".to_string());
            }

            // DNF
            (PackageManager::Dnf, PackageOperation::Install) => {
                args.push("-y".to_string());
                args.push("install".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Dnf, PackageOperation::Uninstall) => {
                args.push("-y".to_string());
                args.push("remove".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Dnf, PackageOperation::Search) => {
                args.push("search".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Dnf, PackageOperation::List) => {
                args.push("list".to_string());
                args.push("installed".to_string());
            }
            (PackageManager::Dnf, PackageOperation::Update) => {
                args.push("-y".to_string());
                args.push("update".to_string());
            }

            // Homebrew
            (PackageManager::Brew, PackageOperation::Install) => {
                args.push("install".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Brew, PackageOperation::Uninstall) => {
                args.push("uninstall".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Brew, PackageOperation::Search) => {
                args.push("search".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Brew, PackageOperation::List) => {
                args.push("list".to_string());
            }
            (PackageManager::Brew, PackageOperation::Update) => {
                args.push("update".to_string());
            }

            // Chocolatey
            (PackageManager::Choco, PackageOperation::Install) => {
                args.push("install".to_string());
                args.push("-y".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Choco, PackageOperation::Uninstall) => {
                args.push("uninstall".to_string());
                args.push("-y".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Choco, PackageOperation::Search) => {
                args.push("search".to_string());
                args.extend(packages.iter().cloned());
            }
            (PackageManager::Choco, PackageOperation::List) => {
                args.push("list".to_string());
                args.push("--local-only".to_string());
            }
            (PackageManager::Choco, PackageOperation::Update) => {
                args.push("upgrade".to_string());
                args.push("all".to_string());
                args.push("-y".to_string());
            }
        }

        Ok(args)
    }

    /// Validate package names to prevent command injection.
    fn validate_packages(packages: &[String]) -> Result<(), PackageManagerError> {
        for pkg in packages {
            // Package names should be alphanumeric with allowed special chars
            if pkg.is_empty() {
                return Err(PackageManagerError::InvalidPackageName {
                    name: pkg.clone(),
                    reason: "Package name cannot be empty".to_string(),
                    suggestion: "Provide a valid package name".to_string(),
                });
            }
            if pkg.len() > 256 {
                return Err(PackageManagerError::InvalidPackageName {
                    name: pkg.clone(),
                    reason: format!("Name too long ({} characters, max 256)", pkg.len()),
                    suggestion: "Use a shorter package name".to_string(),
                });
            }
            // Allow alphanumeric, hyphens, underscores, dots, plus signs, and colons (for versions)
            let invalid_chars: Vec<char> = pkg.chars().filter(|c| {
                !c.is_ascii_alphanumeric()
                    && *c != '-'
                    && *c != '_'
                    && *c != '.'
                    && *c != '+'
                    && *c != ':'
                    && *c != '@'
            }).collect();

            if !invalid_chars.is_empty() {
                return Err(PackageManagerError::InvalidPackageName {
                    name: pkg.clone(),
                    reason: format!("Contains invalid characters: {:?}", invalid_chars),
                    suggestion: "Use only alphanumeric characters, hyphens, underscores, dots, plus signs, colons, or @".to_string(),
                });
            }
            // Reject obvious injection attempts
            if pkg.contains("..") || pkg.starts_with('-') {
                return Err(PackageManagerError::InvalidPackageName {
                    name: pkg.clone(),
                    reason: "Potentially dangerous pattern detected".to_string(),
                    suggestion: "Package names cannot start with '-' or contain '..'".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Analyze stderr output to provide actionable suggestions.
    fn analyze_error(stderr: &str) -> Option<String> {
        let stderr_lower = stderr.to_lowercase();

        if stderr_lower.contains("permission denied") || stderr_lower.contains("access denied") {
            return Some("Try running with elevated privileges (sudo on Linux/macOS, Administrator on Windows)".to_string());
        }
        if stderr_lower.contains("not found") || stderr_lower.contains("no match") {
            return Some("The package may not exist or the package name may be misspelled. Try searching for similar packages.".to_string());
        }
        if stderr_lower.contains("dependency") || stderr_lower.contains("conflict") {
            return Some("There may be dependency conflicts. Try updating the package index first.".to_string());
        }
        if stderr_lower.contains("network") || stderr_lower.contains("connection") || stderr_lower.contains("timeout") {
            return Some("Network issue detected. Check your internet connection and try again.".to_string());
        }
        if stderr_lower.contains("lock") || stderr_lower.contains("in use") {
            return Some("Another package manager process may be running. Wait for it to complete or terminate it.".to_string());
        }

        None
    }
}

#[async_trait]
impl Tool for PackageManagerTool {
    fn name(&self) -> &str {
        "package_manager"
    }

    fn description(&self) -> &str {
        "Manage system packages. Auto-detects apt/yum/dnf/brew/choco based on the platform."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["install", "uninstall", "search", "list", "update"],
                    "description": "The package operation to perform"
                },
                "packages": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Package names (required for install/uninstall/search)"
                }
            },
            "required": ["operation"]
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::HighRisk
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn default_timeout_ms(&self) -> u64 {
        self.timeout.as_millis() as u64
    }

    #[instrument(skip(self, args), fields(operation, packages))]
    async fn execute(&self, args: Value) -> ToolResult<ToolOutput> {
        let args: PackageManagerArgs = serde_json::from_value(args).map_err(|e| {
            ToolError::InvalidArguments(format!("Invalid arguments: {}", e))
        })?;

        debug!(operation = ?args.operation, packages = ?args.packages, "Executing package manager tool");

        // Validate packages if provided
        Self::validate_packages(&args.packages).map_err(IntoToolError::into_tool_error)?;

        // Require packages for install/uninstall/search
        match args.operation {
            PackageOperation::Install | PackageOperation::Uninstall | PackageOperation::Search => {
                if args.packages.is_empty() {
                    let err = PackageManagerError::MissingPackages {
                        operation: format!("{:?}", args.operation).to_lowercase(),
                    };
                    warn!(?err, "Missing packages for operation");
                    return Err(err.into_tool_error());
                }
            }
            _ => {}
        }

        // Detect package manager
        debug!("Detecting package manager");
        let pm = Self::detect_package_manager().await.ok_or_else(|| {
            let err = PackageManagerError::NoPackageManager {
                checked: vec!["apt-get", "yum", "dnf", "brew", "choco"],
                suggestion: "Install a supported package manager for your platform",
            };
            warn!(?err, "No package manager found");
            err.into_tool_error()
        })?;
        info!(package_manager = %pm.display_name(), "Detected package manager");

        // Build command
        let cmd_args = Self::build_command_args(pm, args.operation, &args.packages)
            .map_err(IntoToolError::into_tool_error)?;
        let full_command = format!("{} {}", pm.command(), cmd_args.join(" "));
        debug!(command = %full_command, "Built package manager command");

        // Execute command
        let start = std::time::Instant::now();
        let result = timeout(
            self.timeout,
            Command::new(pm.command())
                .args(&cmd_args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                // Truncate output if too large
                let max_output = 50_000;
                let stdout_truncated = if stdout.len() > max_output {
                    format!("{}... (truncated)", &stdout[..max_output])
                } else {
                    stdout.to_string()
                };
                let stderr_truncated = if stderr.len() > max_output {
                    format!("{}... (truncated)", &stderr[..max_output])
                } else {
                    stderr.to_string()
                };

                let result = json!({
                    "package_manager": pm.display_name(),
                    "command": full_command,
                    "operation": format!("{:?}", args.operation).to_lowercase(),
                    "packages": args.packages,
                    "exit_code": exit_code,
                    "stdout": stdout_truncated,
                    "stderr": stderr_truncated,
                    "duration_ms": duration_ms,
                    "success": output.status.success()
                });

                if output.status.success() {
                    info!(duration_ms, exit_code, "Package manager command succeeded");
                    Ok(ToolOutput::success(result))
                } else {
                    // Analyze stderr to provide actionable suggestion
                    let suggestion = Self::analyze_error(&stderr_truncated);
                    let err = PackageManagerError::CommandFailed {
                        package_manager: pm.display_name().to_string(),
                        command: full_command,
                        exit_code,
                        stderr: stderr_truncated.clone(),
                        suggestion,
                    };
                    warn!(?err, duration_ms, "Package manager command failed");

                    // Return as error output with metadata for LLM context
                    Ok(ToolOutput::error(err.to_user_message()).with_metadata(result))
                }
            }
            Ok(Err(e)) => {
                let err = PackageManagerError::ExecutionFailed {
                    package_manager: pm.display_name().to_string(),
                    command: full_command,
                    source: e,
                };
                warn!(?err, "Failed to execute package manager");
                Err(err.into_tool_error())
            }
            Err(_) => {
                let err = PackageManagerError::Timeout {
                    package_manager: pm.display_name().to_string(),
                    operation: format!("{:?}", args.operation).to_lowercase(),
                    timeout_secs: self.timeout.as_secs(),
                };
                warn!(?err, "Package manager operation timed out");
                Err(err.into_tool_error())
            }
        }
    }

    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_packages_valid() {
        assert!(PackageManagerTool::validate_packages(&["nginx".to_string()]).is_ok());
        assert!(PackageManagerTool::validate_packages(&["vim".to_string(), "git".to_string()]).is_ok());
        assert!(PackageManagerTool::validate_packages(&["python3.10".to_string()]).is_ok());
        assert!(PackageManagerTool::validate_packages(&["nodejs@18".to_string()]).is_ok());
        assert!(PackageManagerTool::validate_packages(&["gcc-c++".to_string()]).is_ok());
    }

    #[test]
    fn test_validate_packages_invalid() {
        assert!(PackageManagerTool::validate_packages(&["".to_string()]).is_err());
        assert!(PackageManagerTool::validate_packages(&["pkg;rm -rf /".to_string()]).is_err());
        assert!(PackageManagerTool::validate_packages(&["--help".to_string()]).is_err());
        assert!(PackageManagerTool::validate_packages(&["../etc/passwd".to_string()]).is_err());
        assert!(PackageManagerTool::validate_packages(&["pkg$(whoami)".to_string()]).is_err());
    }

    #[test]
    fn test_validate_packages_error_details() {
        let err = PackageManagerTool::validate_packages(&["bad;pkg".to_string()]).unwrap_err();
        match err {
            PackageManagerError::InvalidPackageName { name, reason, suggestion } => {
                assert_eq!(name, "bad;pkg");
                assert!(reason.contains("invalid characters"));
                assert!(!suggestion.is_empty());
            }
            _ => panic!("Expected InvalidPackageName error"),
        }
    }

    #[test]
    fn test_build_command_args_brew_install() {
        let args = PackageManagerTool::build_command_args(
            PackageManager::Brew,
            PackageOperation::Install,
            &["nginx".to_string()],
        )
        .unwrap();
        assert_eq!(args, vec!["install", "nginx"]);
    }

    #[test]
    fn test_build_command_args_apt_install() {
        let args = PackageManagerTool::build_command_args(
            PackageManager::Apt,
            PackageOperation::Install,
            &["nginx".to_string()],
        )
        .unwrap();
        assert_eq!(args, vec!["-y", "install", "nginx"]);
    }

    #[test]
    fn test_build_command_args_choco_install() {
        let args = PackageManagerTool::build_command_args(
            PackageManager::Choco,
            PackageOperation::Install,
            &["git".to_string()],
        )
        .unwrap();
        assert_eq!(args, vec!["install", "-y", "git"]);
    }

    #[test]
    fn test_build_command_args_apt_search_unsupported() {
        let err = PackageManagerTool::build_command_args(
            PackageManager::Apt,
            PackageOperation::Search,
            &["nginx".to_string()],
        ).unwrap_err();
        match err {
            PackageManagerError::UnsupportedOperation { operation, suggestion, .. } => {
                assert_eq!(operation, "search");
                assert!(suggestion.contains("apt-cache"));
            }
            _ => panic!("Expected UnsupportedOperation error"),
        }
    }

    #[test]
    fn test_package_manager_display_names() {
        assert_eq!(PackageManager::Apt.display_name(), "APT (Debian/Ubuntu)");
        assert_eq!(PackageManager::Brew.display_name(), "Homebrew (macOS)");
        assert_eq!(PackageManager::Choco.display_name(), "Chocolatey (Windows)");
    }

    #[test]
    fn test_analyze_error_suggestions() {
        assert!(PackageManagerTool::analyze_error("Permission denied: /var/lib")
            .unwrap().contains("elevated privileges"));
        assert!(PackageManagerTool::analyze_error("Package 'foo' not found")
            .unwrap().contains("may not exist"));
        assert!(PackageManagerTool::analyze_error("dependency conflict detected")
            .unwrap().contains("dependency"));
        assert!(PackageManagerTool::analyze_error("network unreachable")
            .unwrap().contains("internet connection"));
        assert!(PackageManagerTool::analyze_error("lock file in use")
            .unwrap().contains("Another package manager"));
        assert!(PackageManagerTool::analyze_error("some random error").is_none());
    }
}
