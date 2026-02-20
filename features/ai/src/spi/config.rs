/// Configuration from environment variables.
use std::path::PathBuf;

use llmrag::VectorStoreConfig;

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
    /// Default agent ID to activate on startup (e.g. "shell").
    pub default_agent: String,
    /// Whether to auto-detect the best agent from input keywords.
    pub agent_auto_detect: bool,
    /// Optional directory for logging LLM request/response JSON files.
    pub log_dir: Option<PathBuf>,
    /// Base directory for resolving `docs_context` source paths in built-in agents.
    ///
    /// When set, agents that define a `docs` block in their YAML will have
    /// their source globs resolved relative to this directory.
    /// Defaults to the current working directory when not explicitly set.
    pub docs_base_dir: Option<PathBuf>,
    /// RAG (Retrieval-Augmented Generation) configuration.
    pub rag: RagConfig,
}

/// RAG (Retrieval-Augmented Generation) configuration.
#[derive(Debug, Clone)]
pub struct RagConfig {
    /// Vector store backend configuration.
    pub vector_store: VectorStoreConfig,
    /// Chunk size for document splitting (characters).
    pub chunk_size: usize,
    /// Overlap between chunks (characters).
    pub chunk_overlap: usize,
    /// Whether to include `score: X.XXX` in `rag_search` tool output.
    ///
    /// When `true` (default), the LLM sees raw cosine similarity scores
    /// and may use them as a relevance gate.  Set to `false` to hide scores
    /// and let the LLM rely solely on content quality.
    pub show_scores: bool,
    /// Minimum cosine similarity threshold for RAG results.
    ///
    /// Results below this score are dropped before being returned to the LLM.
    /// `None` (default) means no server-side threshold is applied.
    pub min_score: Option<f32>,
    /// Normalize Markdown tables to prose before embedding.
    ///
    /// When `true`, table cells are converted to `Header: value.` sentences
    /// before chunking, improving embedding quality for tabular data.
    pub normalize_markdown: bool,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            vector_store: VectorStoreConfig::Memory,
            chunk_size: 2000,
            chunk_overlap: 200,
            show_scores: true,
            min_score: None,
            normalize_markdown: false,
        }
    }
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
    /// Enable RAG document search tools.
    pub enable_rag: bool,
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
    /// Tool result caching configuration.
    pub cache: ToolCacheConfig,
}

/// Configuration for tool result caching.
///
/// When enabled, ReadOnly tool results (file reads, directory listings, metadata checks)
/// are cached to eliminate redundant tool round-trips within agent sessions.
/// Each agent gets its own isolated cache, scoped to the engine's lifetime.
#[derive(Debug, Clone)]
pub struct ToolCacheConfig {
    /// Whether tool result caching is enabled.
    pub enabled: bool,
    /// Time-to-live for cached entries in seconds.
    pub ttl_secs: u64,
    /// Maximum number of cached entries.
    pub max_entries: usize,
}

impl Default for ToolCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: 300,
            max_entries: 200,
        }
    }
}

impl ToolConfig {
    /// Check if any tools are enabled.
    pub fn enabled(&self) -> bool {
        self.enable_fs || self.enable_exec || self.enable_web || self.enable_rag
    }
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            enable_fs: true,
            enable_exec: true,
            enable_web: true,
            enable_rag: false,
            require_confirmation: true,
            max_tool_calls_per_turn: 10,
            max_iterations: 10,
            fs_max_size: 1_048_576, // 1MB
            exec_timeout: 30,
            cache: ToolCacheConfig::default(),
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
    /// | `SWEBASH_AI_DEFAULT_AGENT` | `shell` | Default agent on startup |
    /// | `SWEBASH_AI_AGENT_AUTO_DETECT` | `true` | Auto-detect agent from keywords |
    /// | `SWEBASH_AI_LOG_DIR` | _(none)_ | Directory for LLM request/response JSON logs |
    /// | `SWEBASH_AI_TOOL_CACHE` | `true` | Enable/disable tool result caching |
    /// | `SWEBASH_AI_TOOL_CACHE_TTL` | `300` | Cache TTL in seconds |
    /// | `SWEBASH_AI_TOOL_CACHE_MAX` | `200` | Max cached entries |
    /// | `SWEBASH_AI_TOOLS_RAG` | `false` | Enable RAG document search tools |
    /// | `SWEBASH_AI_DOCS_BASE_DIR` | _(cwd)_ | Base dir for agent docs_context source paths |
    /// | `SWEBASH_AI_RAG_STORE` | `memory` | Vector store: memory, file, sqlite, swevecdb |
    /// | `SWEBASH_AI_RAG_STORE_PATH` | `.swebash/rag` | Path for file/sqlite store |
    /// | `SWEBASH_AI_RAG_SWEVECDB_ENDPOINT` | `http://localhost:8080` | SweVecDB server endpoint |
    /// | `SWEBASH_AI_RAG_CHUNK_SIZE` | `2000` | Document chunk size (chars) |
    /// | `SWEBASH_AI_RAG_CHUNK_OVERLAP` | `200` | Chunk overlap (chars) |
    /// | `SWEBASH_AI_RAG_SHOW_SCORES` | `true` | Include `score: X.XXX` in rag_search output |
    /// | `SWEBASH_AI_RAG_MIN_SCORE` | _(none)_ | Drop results below this cosine similarity threshold |
    /// | `SWEBASH_AI_RAG_NORMALIZE_MARKDOWN` | `false` | Convert Markdown tables to prose before embedding |
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
            enable_rag: std::env::var("SWEBASH_AI_TOOLS_RAG")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
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
            cache: ToolCacheConfig {
                enabled: std::env::var("SWEBASH_AI_TOOL_CACHE")
                    .map(|v| v != "false" && v != "0")
                    .unwrap_or(true),
                ttl_secs: std::env::var("SWEBASH_AI_TOOL_CACHE_TTL")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(300),
                max_entries: std::env::var("SWEBASH_AI_TOOL_CACHE_MAX")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(200),
            },
        };

        let default_agent = std::env::var("SWEBASH_AI_DEFAULT_AGENT")
            .unwrap_or_else(|_| "shell".to_string());

        let agent_auto_detect = std::env::var("SWEBASH_AI_AGENT_AUTO_DETECT")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        let log_dir = std::env::var("SWEBASH_AI_LOG_DIR")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);

        let docs_base_dir = std::env::var("SWEBASH_AI_DOCS_BASE_DIR")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok());

        // RAG configuration
        let vector_store = match std::env::var("SWEBASH_AI_RAG_STORE")
            .unwrap_or_else(|_| "memory".to_string())
            .to_lowercase()
            .as_str()
        {
            "file" => {
                let path = std::env::var("SWEBASH_AI_RAG_STORE_PATH")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from(".swebash/rag"));
                VectorStoreConfig::File { path }
            }
            "sqlite" => {
                let path = std::env::var("SWEBASH_AI_RAG_STORE_PATH")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from(".swebash/rag.db"));
                VectorStoreConfig::Sqlite { path }
            }
            "swevecdb" => {
                let endpoint = std::env::var("SWEBASH_AI_RAG_SWEVECDB_ENDPOINT")
                    .unwrap_or_else(|_| "http://localhost:8080".to_string());
                VectorStoreConfig::Swevecdb { endpoint }
            }
            _ => VectorStoreConfig::Memory,
        };

        let rag = RagConfig {
            vector_store,
            chunk_size: std::env::var("SWEBASH_AI_RAG_CHUNK_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2000),
            chunk_overlap: std::env::var("SWEBASH_AI_RAG_CHUNK_OVERLAP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
            show_scores: std::env::var("SWEBASH_AI_RAG_SHOW_SCORES")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            min_score: std::env::var("SWEBASH_AI_RAG_MIN_SCORE")
                .ok()
                .and_then(|v| v.parse::<f32>().ok()),
            normalize_markdown: std::env::var("SWEBASH_AI_RAG_NORMALIZE_MARKDOWN")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        };

        Self {
            enabled,
            provider,
            model,
            history_size,
            tools,
            default_agent,
            agent_auto_detect,
            log_dir,
            docs_base_dir,
            rag,
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

    /// Check if Claude Code OAuth credentials are available (`~/.claude/.credentials.json`).
    /// Only meaningful when `provider == "anthropic"`.
    pub fn has_oauth_credentials(&self) -> bool {
        self.provider == "anthropic" && llm_oauth::from_claude_credentials().is_ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn config_reads_log_dir_from_env() {
        std::env::set_var("SWEBASH_AI_LOG_DIR", "/tmp/ai-logs");
        let config = AiConfig::from_env();
        assert_eq!(config.log_dir, Some(PathBuf::from("/tmp/ai-logs")));
        std::env::remove_var("SWEBASH_AI_LOG_DIR");
    }

    #[test]
    #[serial]
    fn config_log_dir_none_when_unset() {
        std::env::remove_var("SWEBASH_AI_LOG_DIR");
        let config = AiConfig::from_env();
        assert_eq!(config.log_dir, None);
    }
}
