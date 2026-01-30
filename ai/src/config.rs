/// Configuration from environment variables.

/// AI service configuration.
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// Whether AI features are enabled.
    pub enabled: bool,
    /// LLM provider name (e.g. "openai", "anthropic", "gemini").
    pub provider: String,
    /// Model to use (e.g. "gpt-4o", "claude-sonnet-4-20250514").
    pub model: String,
    /// Maximum chat history messages to retain.
    pub history_size: usize,
}

impl AiConfig {
    /// Load configuration from environment variables.
    ///
    /// | Variable | Default | Purpose |
    /// |----------|---------|---------|
    /// | `SWEBASH_AI_ENABLED` | `true` | Enable/disable AI features |
    /// | `LLM_PROVIDER` | `openai` | Provider: openai, anthropic, gemini |
    /// | `LLM_DEFAULT_MODEL` | `gpt-4o` | Default model |
    /// | `SWEBASH_AI_HISTORY_SIZE` | `20` | Max chat history messages |
    pub fn from_env() -> Self {
        let enabled = std::env::var("SWEBASH_AI_ENABLED")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let provider = std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "openai".to_string());

        let model = std::env::var("LLM_DEFAULT_MODEL").unwrap_or_else(|_| {
            default_model_for_provider(&provider)
        });

        let history_size = std::env::var("SWEBASH_AI_HISTORY_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);

        Self {
            enabled,
            provider,
            model,
            history_size,
        }
    }

    /// Check if an API key is available for the configured provider.
    pub fn has_api_key(&self) -> bool {
        let key_var = match self.provider.as_str() {
            "openai" => "OPENAI_API_KEY",
            "anthropic" => "ANTHROPIC_API_KEY",
            "gemini" => "GEMINI_API_KEY",
            _ => return false,
        };
        std::env::var(key_var).is_ok()
    }
}

/// Return the default model for a given provider.
fn default_model_for_provider(provider: &str) -> String {
    match provider {
        "anthropic" => "claude-sonnet-4-20250514".to_string(),
        "gemini" => "gemini-2.0-flash".to_string(),
        _ => "gpt-4o".to_string(),
    }
}
