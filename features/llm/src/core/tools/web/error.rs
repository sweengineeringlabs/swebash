//! Structured error types for web tools.
//!
//! Provides rich error context for HTTP operations, scraping, and API interactions.

use std::time::Duration;

use thiserror::Error;
use llmboot_orchestration::ToolError;

use super::super::error::{ErrorCategory, IntoToolError};

/// HTTP client specific errors.
#[derive(Debug, Error)]
pub enum HttpError {
    #[error("Invalid URL: {url}")]
    InvalidUrl {
        url: String,
        reason: String,
    },

    #[error("Request failed: {status} {status_text}")]
    RequestFailed {
        url: String,
        status: u16,
        status_text: String,
        body: Option<String>,
    },

    #[error("Connection failed: {message}")]
    ConnectionFailed {
        url: String,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Request timed out after {timeout:?}")]
    Timeout {
        url: String,
        timeout: Duration,
    },

    #[error("Too many redirects (max: {max_redirects})")]
    TooManyRedirects {
        url: String,
        max_redirects: u8,
    },

    #[error("SSL/TLS error: {message}")]
    TlsError {
        url: String,
        message: String,
    },

    #[error("Rate limited: retry after {retry_after:?}")]
    RateLimited {
        url: String,
        retry_after: Option<Duration>,
    },

    #[error("Response too large: {size} bytes (max: {max_size})")]
    ResponseTooLarge {
        url: String,
        size: u64,
        max_size: u64,
    },

    #[error("Invalid response: {reason}")]
    InvalidResponse {
        url: String,
        reason: String,
    },
}

impl HttpError {
    /// Get the error category.
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::InvalidUrl { .. } => ErrorCategory::Validation,
            Self::RequestFailed { status, .. } => {
                if *status >= 500 {
                    ErrorCategory::Network
                } else if *status == 401 || *status == 403 {
                    ErrorCategory::Permission
                } else {
                    ErrorCategory::Validation
                }
            }
            Self::ConnectionFailed { .. } => ErrorCategory::Network,
            Self::Timeout { .. } => ErrorCategory::Timeout,
            Self::TooManyRedirects { .. } => ErrorCategory::Validation,
            Self::TlsError { .. } => ErrorCategory::Configuration,
            Self::RateLimited { .. } => ErrorCategory::Network,
            Self::ResponseTooLarge { .. } => ErrorCategory::Validation,
            Self::InvalidResponse { .. } => ErrorCategory::Validation,
        }
    }

    /// Get an actionable suggestion for the user.
    pub fn suggestion(&self) -> String {
        match self {
            Self::InvalidUrl { .. } => {
                "Verify the URL is correctly formatted with scheme (http/https)".to_string()
            }
            Self::RequestFailed { status, .. } => {
                match *status {
                    400 => "Check the request parameters".to_string(),
                    401 => "Authentication required - provide valid credentials".to_string(),
                    403 => "Access forbidden - check permissions".to_string(),
                    404 => "Resource not found - verify the URL".to_string(),
                    429 => "Rate limited - wait before retrying".to_string(),
                    500..=599 => "Server error - try again later".to_string(),
                    _ => format!("HTTP {} error - check the response body for details", status),
                }
            }
            Self::ConnectionFailed { .. } => {
                "Check network connectivity and DNS resolution".to_string()
            }
            Self::Timeout { timeout, .. } => {
                format!("Request exceeded {:?} timeout - consider increasing timeout or checking server responsiveness", timeout)
            }
            Self::TooManyRedirects { max_redirects, .. } => {
                format!("Redirect loop detected (>{} redirects) - check the URL", max_redirects)
            }
            Self::TlsError { .. } => {
                "SSL/TLS handshake failed - verify certificate validity".to_string()
            }
            Self::RateLimited { retry_after, .. } => {
                match retry_after {
                    Some(d) => format!("Wait {:?} before retrying", d),
                    None => "Wait before retrying the request".to_string(),
                }
            }
            Self::ResponseTooLarge { max_size, .. } => {
                format!("Response exceeds {} byte limit", max_size)
            }
            Self::InvalidResponse { reason, .. } => {
                format!("Could not parse response: {}", reason)
            }
        }
    }

    /// Whether this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        self.category().is_retryable()
    }

    /// Convert to a user-friendly message with context.
    pub fn to_user_message(&self) -> String {
        format!("{}\n\nSuggestion: {}", self, self.suggestion())
    }
}

impl IntoToolError for HttpError {
    fn into_tool_error(self) -> ToolError {
        match self.category() {
            ErrorCategory::Validation => {
                ToolError::InvalidArguments(self.to_user_message())
            }
            ErrorCategory::Permission => {
                ToolError::PermissionDenied(self.to_user_message())
            }
            ErrorCategory::Timeout => {
                if let HttpError::Timeout { timeout, .. } = &self {
                    ToolError::Timeout(timeout.as_millis() as u64)
                } else {
                    ToolError::ExecutionFailed(self.to_user_message())
                }
            }
            _ => ToolError::ExecutionFailed(self.to_user_message()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_error_categories() {
        let err = HttpError::InvalidUrl {
            url: "not-a-url".into(),
            reason: "missing scheme".into(),
        };
        assert_eq!(err.category(), ErrorCategory::Validation);
        assert!(!err.is_retryable());

        let err = HttpError::Timeout {
            url: "https://example.com".into(),
            timeout: Duration::from_secs(30),
        };
        assert_eq!(err.category(), ErrorCategory::Timeout);
        assert!(err.is_retryable());

        let err = HttpError::RateLimited {
            url: "https://api.example.com".into(),
            retry_after: Some(Duration::from_secs(60)),
        };
        assert_eq!(err.category(), ErrorCategory::Network);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_http_error_suggestions() {
        let err = HttpError::RequestFailed {
            url: "https://example.com".into(),
            status: 404,
            status_text: "Not Found".into(),
            body: None,
        };
        assert!(err.suggestion().contains("not found"));

        let err = HttpError::RequestFailed {
            url: "https://example.com".into(),
            status: 503,
            status_text: "Service Unavailable".into(),
            body: None,
        };
        assert!(err.suggestion().contains("later"));
    }

    #[test]
    fn test_http_error_to_tool_error() {
        let err = HttpError::InvalidUrl {
            url: "bad".into(),
            reason: "missing scheme".into(),
        };
        let tool_err = err.into_tool_error();
        assert!(matches!(tool_err, ToolError::InvalidArguments(_)));

        let err = HttpError::RequestFailed {
            url: "https://example.com".into(),
            status: 403,
            status_text: "Forbidden".into(),
            body: None,
        };
        let tool_err = err.into_tool_error();
        assert!(matches!(tool_err, ToolError::PermissionDenied(_)));
    }
}
