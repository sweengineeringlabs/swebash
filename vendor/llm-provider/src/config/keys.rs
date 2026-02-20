//! LLM provider configuration and API key constants
//!
//! These constants define the environment variable names for:
//! - LLM configuration (provider, model)
//! - API keys for LLM and embedding providers
//!
//! Using centralized constants ensures consistency and makes it easy
//! to update variable names across the codebase.
//!
//! # Example
//!
//! ```rust,ignore
//! use llm_provider::keys;
//! use swe_secrets::{from_env, SecretKey, SecretService};
//!
//! async fn get_openai_key() -> Result<String, Box<dyn std::error::Error>> {
//!     let secrets = from_env().await?;
//!     let secret = secrets.get(&SecretKey::new(keys::OPENAI_API_KEY)).await?;
//!     Ok(secret.value.expose().to_string())
//! }
//! ```

// =============================================================================
// LLM Configuration Keys
// =============================================================================

/// Active LLM provider selection (e.g., "openai", "anthropic", "gemini")
pub const LLM_PROVIDER: &str = "LLM_PROVIDER";

/// Default model to use for LLM requests
pub const LLM_DEFAULT_MODEL: &str = "LLM_DEFAULT_MODEL";

/// Request timeout in milliseconds
pub const LLM_TIMEOUT_MS: &str = "LLM_TIMEOUT_MS";

/// Maximum retries for failed requests
pub const LLM_MAX_RETRIES: &str = "LLM_MAX_RETRIES";

// =============================================================================
// Provider API Keys
// =============================================================================

/// OpenAI API key for GPT models
pub const OPENAI_API_KEY: &str = "OPENAI_API_KEY";

/// Anthropic API key for Claude models
pub const ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";

/// Google Gemini API key
pub const GEMINI_API_KEY: &str = "GEMINI_API_KEY";

/// Google API key (alternative for Gemini)
pub const GOOGLE_API_KEY: &str = "GOOGLE_API_KEY";

/// Voyage AI API key for embeddings
pub const VOYAGE_API_KEY: &str = "VOYAGE_API_KEY";

/// Cohere API key for embeddings and models
pub const COHERE_API_KEY: &str = "COHERE_API_KEY";

/// Azure OpenAI API key
pub const AZURE_OPENAI_API_KEY: &str = "AZURE_OPENAI_API_KEY";

/// Azure OpenAI endpoint URL
pub const AZURE_OPENAI_ENDPOINT: &str = "AZURE_OPENAI_ENDPOINT";

// =============================================================================
// Provider Base URLs
// =============================================================================

/// OpenAI custom base URL
pub const OPENAI_BASE_URL: &str = "OPENAI_BASE_URL";

/// Anthropic custom base URL
pub const ANTHROPIC_BASE_URL: &str = "ANTHROPIC_BASE_URL";

/// Gemini custom base URL
pub const GEMINI_BASE_URL: &str = "GEMINI_BASE_URL";

/// AWS Bedrock region
pub const AWS_BEDROCK_REGION: &str = "AWS_BEDROCK_REGION";

/// Ollama API key (usually not needed for local inference)
pub const OLLAMA_API_KEY: &str = "OLLAMA_API_KEY";

/// Ollama base URL (for local LLM inference)
pub const OLLAMA_BASE_URL: &str = "OLLAMA_BASE_URL";

/// Hugging Face API token
pub const HUGGINGFACE_API_TOKEN: &str = "HUGGINGFACE_API_TOKEN";

/// All LLM configuration keys
pub const CONFIG_KEYS: &[&str] = &[
    LLM_PROVIDER,
    LLM_DEFAULT_MODEL,
    LLM_TIMEOUT_MS,
    LLM_MAX_RETRIES,
];

/// All provider API keys for validation
pub const API_KEYS: &[&str] = &[
    OPENAI_API_KEY,
    ANTHROPIC_API_KEY,
    GEMINI_API_KEY,
    GOOGLE_API_KEY,
    VOYAGE_API_KEY,
    COHERE_API_KEY,
    AZURE_OPENAI_API_KEY,
    OLLAMA_API_KEY,
    HUGGINGFACE_API_TOKEN,
];

/// All base URL keys
pub const BASE_URL_KEYS: &[&str] = &[
    OPENAI_BASE_URL,
    ANTHROPIC_BASE_URL,
    GEMINI_BASE_URL,
    AZURE_OPENAI_ENDPOINT,
    OLLAMA_BASE_URL,
];

/// All well-known LLM keys for validation (config + API + URLs)
pub const ALL_KEYS: &[&str] = &[
    // Config
    LLM_PROVIDER,
    LLM_DEFAULT_MODEL,
    LLM_TIMEOUT_MS,
    LLM_MAX_RETRIES,
    // API keys
    OPENAI_API_KEY,
    ANTHROPIC_API_KEY,
    GEMINI_API_KEY,
    GOOGLE_API_KEY,
    VOYAGE_API_KEY,
    COHERE_API_KEY,
    AZURE_OPENAI_API_KEY,
    OLLAMA_API_KEY,
    HUGGINGFACE_API_TOKEN,
    // Base URLs
    OPENAI_BASE_URL,
    ANTHROPIC_BASE_URL,
    GEMINI_BASE_URL,
    AZURE_OPENAI_ENDPOINT,
    AWS_BEDROCK_REGION,
    OLLAMA_BASE_URL,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_constants() {
        assert_eq!(OPENAI_API_KEY, "OPENAI_API_KEY");
        assert_eq!(ANTHROPIC_API_KEY, "ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_all_keys_contains_main_keys() {
        assert!(ALL_KEYS.contains(&OPENAI_API_KEY));
        assert!(ALL_KEYS.contains(&ANTHROPIC_API_KEY));
        assert!(ALL_KEYS.contains(&GEMINI_API_KEY));
    }
}
