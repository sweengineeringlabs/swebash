/// L1 Common: Error types for the AI service.
use std::fmt;

/// AI-specific errors.
#[derive(Debug)]
pub enum AiError {
    /// AI is not configured (missing API key, disabled, etc.)
    NotConfigured(String),
    /// LLM provider returned an error.
    Provider(String),
    /// Failed to parse LLM response.
    ParseError(String),
    /// Request timed out.
    Timeout,
    /// Rate limited by the provider.
    RateLimited,
    /// RAG index operation failed (build, query, embedding).
    IndexError(String),
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiError::NotConfigured(msg) => write!(f, "AI not configured: {}", msg),
            AiError::Provider(msg) => write!(f, "AI provider error: {}", msg),
            AiError::ParseError(msg) => write!(f, "Failed to parse AI response: {}", msg),
            AiError::Timeout => write!(f, "AI request timed out"),
            AiError::RateLimited => write!(f, "AI rate limited, please try again later"),
            AiError::IndexError(msg) => write!(f, "RAG index error: {}", msg),
        }
    }
}

impl std::error::Error for AiError {}

/// Result type alias for AI operations.
pub type AiResult<T> = Result<T, AiError>;
