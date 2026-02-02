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
    /// Tool calling configuration.
    pub tools: ToolConfig,
}

/// Tool calling configuration.
#[derive(Debug, Clone)]
pub struct ToolConfig {
    /// Enable file system tools.
    pub enable_fs: bool,
    /// Enable command execution tools.
    pub enable_exec: bool,
    /// Enable web search tools.
    pub enable_web: bool,
    /// Require confirmation for dangerous operations.
    pub require_confirmation: bool,
    /// Maximum number of tool calls per turn.
    pub max_tool_calls_per_turn: usize,
    /// Maximum number of tool iterations (to prevent infinite loops).
    pub max_iterations: usize,
    /// Maximum file read size in bytes.
    pub fs_max_size: usize,
    /// Command execution timeout in seconds.
    pub exec_timeout: u64,
}

impl ToolConfig {
    /// Check if any tools are enabled.
    pub fn enabled(&self) -> bool {
        self.enable_fs || self.enable_exec || self.enable_web
    }
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            enable_fs: true,
            enable_exec: true,
            enable_web: true,
            require_confirmation: true,
            max_tool_calls_per_turn: 10,
            max_iterations: 10,
            fs_max_size: 1_048_576, // 1MB
            exec_timeout: 30,
        }
    }
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
    /// | `SWEBASH_AI_TOOLS_FS` | `true` | Enable file system tools |
    /// | `SWEBASH_AI_TOOLS_EXEC` | `true` | Enable command execution |
    /// | `SWEBASH_AI_TOOLS_WEB` | `true` | Enable web search |
    /// | `SWEBASH_AI_TOOLS_CONFIRM` | `true` | Require confirmation for dangerous ops |
    /// | `SWEBASH_AI_TOOLS_MAX_ITER` | `10` | Max tool loop iterations |
    /// | `SWEBASH_AI_FS_MAX_SIZE` | `1048576` | Max file read size (bytes) |
    /// | `SWEBASH_AI_EXEC_TIMEOUT` | `30` | Command timeout (seconds) |
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

        let tools = ToolConfig {
            enable_fs: std::env::var("SWEBASH_AI_TOOLS_FS")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            enable_exec: std::env::var("SWEBASH_AI_TOOLS_EXEC")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            enable_web: std::env::var("SWEBASH_AI_TOOLS_WEB")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            require_confirmation: std::env::var("SWEBASH_AI_TOOLS_CONFIRM")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            max_tool_calls_per_turn: 10,
            max_iterations: std::env::var("SWEBASH_AI_TOOLS_MAX_ITER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            fs_max_size: std::env::var("SWEBASH_AI_FS_MAX_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1_048_576),
            exec_timeout: std::env::var("SWEBASH_AI_EXEC_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        };

        Self {
            enabled,
            provider,
            model,
            history_size,
            tools,
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
