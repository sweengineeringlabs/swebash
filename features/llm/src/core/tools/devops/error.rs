//! Structured error types for DevOps tools.
//!
//! Provides rich error context, actionable suggestions, and proper error chaining.

use thiserror::Error;
use tool::ToolError;

use super::super::error::{ErrorCategory, IntoToolError};

/// Package manager specific errors.
#[derive(Debug, Error)]
pub enum PackageManagerError {
    #[error("No supported package manager found")]
    NoPackageManager {
        checked: Vec<&'static str>,
        suggestion: &'static str,
    },

    #[error("Invalid package name: {name}")]
    InvalidPackageName {
        name: String,
        reason: String,
        suggestion: String,
    },

    #[error("Operation '{operation}' requires packages")]
    MissingPackages {
        operation: String,
    },

    #[error("Package manager command failed")]
    CommandFailed {
        package_manager: String,
        command: String,
        exit_code: i32,
        stderr: String,
        suggestion: Option<String>,
    },

    #[error("Command execution failed")]
    ExecutionFailed {
        package_manager: String,
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Operation timed out after {timeout_secs}s")]
    Timeout {
        package_manager: String,
        operation: String,
        timeout_secs: u64,
    },

    #[error("Unsupported operation '{operation}' for {package_manager}")]
    UnsupportedOperation {
        package_manager: String,
        operation: String,
        suggestion: String,
    },
}

impl PackageManagerError {
    /// Get the error category.
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::NoPackageManager { .. } => ErrorCategory::Configuration,
            Self::InvalidPackageName { .. } => ErrorCategory::Validation,
            Self::MissingPackages { .. } => ErrorCategory::Validation,
            Self::CommandFailed { .. } => ErrorCategory::Execution,
            Self::ExecutionFailed { .. } => ErrorCategory::Execution,
            Self::Timeout { .. } => ErrorCategory::Timeout,
            Self::UnsupportedOperation { .. } => ErrorCategory::Validation,
        }
    }

    /// Get an actionable suggestion for the user.
    pub fn suggestion(&self) -> String {
        match self {
            Self::NoPackageManager { suggestion, .. } => suggestion.to_string(),
            Self::InvalidPackageName { suggestion, .. } => suggestion.clone(),
            Self::MissingPackages { operation } => {
                format!("Provide package names for the '{}' operation", operation)
            }
            Self::CommandFailed { suggestion, stderr, .. } => {
                suggestion.clone().unwrap_or_else(|| {
                    if stderr.contains("permission denied") || stderr.contains("Permission denied") {
                        "Try running with elevated privileges (sudo/administrator)".to_string()
                    } else if stderr.contains("not found") {
                        "Check if the package name is correct".to_string()
                    } else {
                        "Check the error output for details".to_string()
                    }
                })
            }
            Self::ExecutionFailed { .. } => {
                "Verify the package manager is installed and in PATH".to_string()
            }
            Self::Timeout { operation, .. } => {
                format!("The '{}' operation is taking longer than expected. Consider retrying or checking network connectivity.", operation)
            }
            Self::UnsupportedOperation { suggestion, .. } => suggestion.clone(),
        }
    }

    /// Whether this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        self.category().is_retryable()
    }

    /// Convert to a user-friendly message with context.
    pub fn to_user_message(&self) -> String {
        format!(
            "{}\n\nSuggestion: {}",
            self,
            self.suggestion()
        )
    }
}

/// Download tool specific errors.
#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("Invalid URL: {url}")]
    InvalidUrl {
        url: String,
        reason: String,
    },

    #[error("URL scheme '{scheme}' is not supported")]
    UnsupportedScheme {
        scheme: String,
        suggestion: &'static str,
    },

    #[error("Downloads from private networks are blocked")]
    PrivateNetwork {
        host: String,
    },

    #[error("Invalid checksum format")]
    InvalidChecksum {
        provided: String,
        expected_format: &'static str,
    },

    #[error("Checksum verification failed")]
    ChecksumMismatch {
        algorithm: String,
        expected: String,
        actual: String,
    },

    #[error("HTTP request failed: {status}")]
    HttpError {
        url: String,
        status: u16,
        message: String,
    },

    #[error("Download failed")]
    NetworkError {
        url: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("File size {size} exceeds maximum allowed {max_size}")]
    SizeExceeded {
        size: u64,
        max_size: u64,
    },

    #[error("Failed to write to file: {path}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Cannot determine output filename from URL")]
    NoFilename {
        url: String,
    },
}

impl DownloadError {
    /// Get the error category.
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::InvalidUrl { .. } => ErrorCategory::Validation,
            Self::UnsupportedScheme { .. } => ErrorCategory::Validation,
            Self::PrivateNetwork { .. } => ErrorCategory::Validation,
            Self::InvalidChecksum { .. } => ErrorCategory::Validation,
            Self::ChecksumMismatch { .. } => ErrorCategory::Validation,
            Self::HttpError { status, .. } if *status >= 500 => ErrorCategory::Network,
            Self::HttpError { .. } => ErrorCategory::Validation,
            Self::NetworkError { .. } => ErrorCategory::Network,
            Self::SizeExceeded { .. } => ErrorCategory::Validation,
            Self::IoError { .. } => ErrorCategory::Permission,
            Self::NoFilename { .. } => ErrorCategory::Validation,
        }
    }

    /// Get an actionable suggestion for the user.
    pub fn suggestion(&self) -> String {
        match self {
            Self::InvalidUrl { .. } => "Verify the URL is correctly formatted".to_string(),
            Self::UnsupportedScheme { suggestion, .. } => suggestion.to_string(),
            Self::PrivateNetwork { .. } => {
                "Use a public URL or download the file manually".to_string()
            }
            Self::InvalidChecksum { expected_format, .. } => {
                format!("Use format: {}", expected_format)
            }
            Self::ChecksumMismatch { .. } => {
                "The file may be corrupted or tampered with. Verify the checksum and retry.".to_string()
            }
            Self::HttpError { status, .. } => {
                if *status == 404 {
                    "The file was not found. Verify the URL is correct.".to_string()
                } else if *status >= 500 {
                    "The server is experiencing issues. Try again later.".to_string()
                } else {
                    "Check the URL and your network connection.".to_string()
                }
            }
            Self::NetworkError { .. } => {
                "Check your network connection and try again.".to_string()
            }
            Self::SizeExceeded { max_size, .. } => {
                format!("Maximum download size is {} bytes", max_size)
            }
            Self::IoError { path, source } => {
                if source.kind() == std::io::ErrorKind::PermissionDenied {
                    format!("Permission denied writing to '{}'. Check file permissions.", path)
                } else {
                    format!("Check that the directory exists and is writable: {}", path)
                }
            }
            Self::NoFilename { .. } => {
                "Specify the output filename with the 'output' parameter".to_string()
            }
        }
    }

    /// Whether this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        self.category().is_retryable()
    }

    /// Convert to a user-friendly message with context.
    pub fn to_user_message(&self) -> String {
        format!(
            "{}\n\nSuggestion: {}",
            self,
            self.suggestion()
        )
    }
}

impl IntoToolError for PackageManagerError {
    fn into_tool_error(self) -> ToolError {
        match self.category() {
            ErrorCategory::Validation => {
                ToolError::InvalidArguments(self.to_user_message())
            }
            ErrorCategory::Timeout => {
                // Extract timeout in ms if available
                if let PackageManagerError::Timeout { timeout_secs, .. } = &self {
                    ToolError::Timeout(timeout_secs * 1000)
                } else {
                    ToolError::ExecutionFailed(self.to_user_message())
                }
            }
            _ => ToolError::ExecutionFailed(self.to_user_message()),
        }
    }
}

impl IntoToolError for DownloadError {
    fn into_tool_error(self) -> ToolError {
        match self.category() {
            ErrorCategory::Validation => {
                ToolError::InvalidArguments(self.to_user_message())
            }
            ErrorCategory::Permission => {
                ToolError::PermissionDenied(self.to_user_message())
            }
            _ => ToolError::ExecutionFailed(self.to_user_message()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_manager_error_categories() {
        let err = PackageManagerError::NoPackageManager {
            checked: vec!["apt", "yum", "choco"],
            suggestion: "Install a package manager",
        };
        assert_eq!(err.category(), ErrorCategory::Configuration);
        assert!(!err.is_retryable());

        let err = PackageManagerError::Timeout {
            package_manager: "apt".into(),
            operation: "install".into(),
            timeout_secs: 300,
        };
        assert_eq!(err.category(), ErrorCategory::Timeout);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_download_error_suggestions() {
        let err = DownloadError::ChecksumMismatch {
            algorithm: "sha256".into(),
            expected: "abc123".into(),
            actual: "def456".into(),
        };
        assert!(err.suggestion().contains("corrupted"));

        let err = DownloadError::HttpError {
            url: "https://example.com".into(),
            status: 404,
            message: "Not Found".into(),
        };
        assert!(err.suggestion().contains("not found"));
    }

    #[test]
    fn test_error_to_user_message() {
        let err = PackageManagerError::InvalidPackageName {
            name: "bad;pkg".into(),
            reason: "contains semicolon".into(),
            suggestion: "Use alphanumeric characters only".into(),
        };
        let msg = err.to_user_message();
        assert!(msg.contains("bad;pkg"));
        assert!(msg.contains("Suggestion:"));
        assert!(msg.contains("alphanumeric"));
    }
}
