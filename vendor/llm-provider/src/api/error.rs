use thiserror::Error;

/// LLM-specific errors with retry classification
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Rate limited{}", match .retry_after_ms {
        Some(ms) => format!(" (retry after {}ms)", ms),
        None => String::new(),
    })]
    RateLimited { retry_after_ms: Option<u64> },

    #[error("Context length exceeded: used {used} tokens, max {max} tokens")]
    ContextLengthExceeded { used: u32, max: u32 },

    #[error("Content filtered: {0}")]
    ContentFiltered(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Timeout after {0}ms")]
    Timeout(u64),

    #[error("Provider error ({provider}): {message}")]
    ProviderError { provider: String, message: String },

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

impl LlmError {
    /// Check if this error is retryable
    ///
    /// Retryable errors are transient failures that might succeed on retry:
    /// - Rate limiting (with backoff)
    /// - Network errors (connectivity issues)
    /// - Timeouts (server overload)
    /// - Provider errors (5xx server errors)
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LlmError::RateLimited { .. }
                | LlmError::NetworkError(_)
                | LlmError::Timeout(_)
                | LlmError::ProviderError { .. }
        )
    }

    /// Get retry delay hint if available (e.g., from rate limit response)
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            LlmError::RateLimited { retry_after_ms: Some(ms) } => {
                Some(std::time::Duration::from_millis(*ms))
            }
            _ => None,
        }
    }
}

pub type LlmResult<T> = Result<T, LlmError>;
