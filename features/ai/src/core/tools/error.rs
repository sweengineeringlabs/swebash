//! Shared error infrastructure for tool implementations.
//!
//! Provides categorization and conversion traits that tool-specific
//! error types can implement for consistent error handling.

use tool::ToolError;

/// Error categories for tool operations.
///
/// Used to classify errors semantically, enabling:
/// - Retry logic (network/timeout errors are retryable)
/// - User guidance (validation errors need different suggestions than execution errors)
/// - Metrics/observability (categorize failures by type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Configuration or environment issue - user action required.
    Configuration,
    /// Invalid input from the LLM - prompt engineering issue.
    Validation,
    /// Network or external service issue - may be transient.
    Network,
    /// Execution failed - check logs/output.
    Execution,
    /// Operation timed out - may retry with longer timeout.
    Timeout,
    /// Permission denied - requires elevated privileges.
    Permission,
}

impl ErrorCategory {
    /// Whether this error category is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self, ErrorCategory::Network | ErrorCategory::Timeout)
    }
}

/// Trait for converting domain-specific errors to `ToolError`.
///
/// Implement this for tool-specific error types to provide
/// consistent conversion to the generic `ToolError` used by
/// the tool infrastructure.
///
/// # Example
///
/// ```ignore
/// impl IntoToolError for MyToolError {
///     fn into_tool_error(self) -> ToolError {
///         match self.category() {
///             ErrorCategory::Validation => ToolError::InvalidArguments(self.to_string()),
///             _ => ToolError::ExecutionFailed(self.to_string()),
///         }
///     }
/// }
/// ```
pub trait IntoToolError {
    fn into_tool_error(self) -> ToolError;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_category_retryable() {
        // Only Network and Timeout are retryable
        assert!(!ErrorCategory::Configuration.is_retryable());
        assert!(!ErrorCategory::Validation.is_retryable());
        assert!(ErrorCategory::Network.is_retryable());
        assert!(!ErrorCategory::Execution.is_retryable());
        assert!(ErrorCategory::Timeout.is_retryable());
        assert!(!ErrorCategory::Permission.is_retryable());
    }

    #[test]
    fn test_error_category_equality() {
        assert_eq!(ErrorCategory::Network, ErrorCategory::Network);
        assert_ne!(ErrorCategory::Network, ErrorCategory::Timeout);
    }

    #[test]
    fn test_error_category_clone() {
        let category = ErrorCategory::Execution;
        let cloned = category.clone();
        assert_eq!(category, cloned);
    }

    #[test]
    fn test_error_category_copy() {
        let category = ErrorCategory::Permission;
        let copied: ErrorCategory = category; // Copy
        assert_eq!(category, copied);
    }

    #[test]
    fn test_error_category_debug() {
        let category = ErrorCategory::Validation;
        let debug_str = format!("{:?}", category);
        assert_eq!(debug_str, "Validation");
    }

    #[test]
    fn test_error_category_all_variants() {
        // Ensure all variants are covered
        let categories = [
            ErrorCategory::Configuration,
            ErrorCategory::Validation,
            ErrorCategory::Network,
            ErrorCategory::Execution,
            ErrorCategory::Timeout,
            ErrorCategory::Permission,
        ];
        assert_eq!(categories.len(), 6);
    }

    // Test IntoToolError with a mock implementation
    #[derive(Debug)]
    struct MockError {
        category: ErrorCategory,
        message: String,
    }

    impl IntoToolError for MockError {
        fn into_tool_error(self) -> ToolError {
            match self.category {
                ErrorCategory::Validation => ToolError::InvalidArguments(self.message),
                ErrorCategory::Permission => ToolError::PermissionDenied(self.message),
                ErrorCategory::Timeout => ToolError::Timeout(30000),
                _ => ToolError::ExecutionFailed(self.message),
            }
        }
    }

    #[test]
    fn test_into_tool_error_validation() {
        let err = MockError {
            category: ErrorCategory::Validation,
            message: "invalid input".into(),
        };
        let tool_err = err.into_tool_error();
        assert!(matches!(tool_err, ToolError::InvalidArguments(msg) if msg == "invalid input"));
    }

    #[test]
    fn test_into_tool_error_permission() {
        let err = MockError {
            category: ErrorCategory::Permission,
            message: "access denied".into(),
        };
        let tool_err = err.into_tool_error();
        assert!(matches!(tool_err, ToolError::PermissionDenied(msg) if msg == "access denied"));
    }

    #[test]
    fn test_into_tool_error_timeout() {
        let err = MockError {
            category: ErrorCategory::Timeout,
            message: "timed out".into(),
        };
        let tool_err = err.into_tool_error();
        assert!(matches!(tool_err, ToolError::Timeout(30000)));
    }

    #[test]
    fn test_into_tool_error_execution() {
        let err = MockError {
            category: ErrorCategory::Execution,
            message: "command failed".into(),
        };
        let tool_err = err.into_tool_error();
        assert!(matches!(tool_err, ToolError::ExecutionFailed(msg) if msg == "command failed"));
    }

    #[test]
    fn test_into_tool_error_network_falls_through() {
        let err = MockError {
            category: ErrorCategory::Network,
            message: "connection refused".into(),
        };
        let tool_err = err.into_tool_error();
        // Network falls through to ExecutionFailed in our mock
        assert!(matches!(tool_err, ToolError::ExecutionFailed(msg) if msg == "connection refused"));
    }
}
