#![forbid(unsafe_code)]

/// L5 Facade: swebash-llm crate entry point.
///
/// Re-exports the public API and provides the `create_ai_service()` factory.
///
/// # Architecture (SEA Pattern)
///
/// ```text
/// L4 Facade   - lib.rs (this file): re-exports, factory
/// L3 Core     - core/: DefaultAiService, feature modules
/// L2 API      - api/: AiService trait (consumer interface)
/// L1 SPI      - spi/: GatewayClient (llmboot integration)
/// ```
///
/// All LLM operations are routed through llmboot's Gateway API, which provides:
/// - Input validation and guardrails
/// - Agent management and execution
/// - Tool orchestration
/// - Pattern execution (ReAct, CoT, etc.)
pub mod api;
pub mod core;
pub mod spi;

// ── Public re-exports (L3 API surface) ──

pub use api::error::{AiError, AiResult};
pub use api::types::{
    AgentInfo, AiEvent, AiMessage, AiResponse, AiRole, AiStatus, AutocompleteRequest,
    AutocompleteResponse, ChatRequest, ChatResponse, CompletionOptions, ExplainRequest,
    ExplainResponse, TranslateRequest, TranslateResponse,
};
pub use api::AiService;
pub use api::cli::handle_ai_command;
pub use api::commands::{self, AiCommand};
pub use api::output;
pub use spi::config::{AiConfig, ToolCacheConfig, ToolConfig};
pub use spi::GatewayClient;
pub use core::DefaultAiService;
pub use core::tools::{ToolSandbox, SandboxAccessMode, SandboxRule};

/// Factory: create the AI service from environment configuration.
///
/// Returns `Ok(service)` if the provider initializes successfully,
/// or `Err` if configuration is missing or invalid.
///
/// Uses llmboot's GatewayClient for all agent operations.
/// Agents are loaded from the default YAML file (`.swebash/agents.yaml`).
///
/// The host should call this at startup and store the result as `Option`:
/// ```ignore
/// let ai_service = swebash_llm::create_ai_service().ok();
/// ```
pub async fn create_ai_service() -> AiResult<DefaultAiService> {
    create_ai_service_with_sandbox(None).await
}

/// Factory: create the AI service with optional sandbox restrictions.
///
/// When `sandbox` is provided, the sandbox configuration is stored for
/// use by AI tools that need path restriction enforcement.
pub async fn create_ai_service_with_sandbox(
    sandbox: Option<std::sync::Arc<ToolSandbox>>,
) -> AiResult<DefaultAiService> {
    let mut config = AiConfig::from_env();

    if !config.enabled {
        return Err(AiError::NotConfigured(
            "AI features disabled (SWEBASH_AI_ENABLED=false)".into(),
        ));
    }

    if !config.has_api_key() && !config.has_oauth_credentials() {
        return Err(AiError::NotConfigured(format!(
            "No credentials found for provider '{}'. Set the API key env var or configure Claude Code OAuth.",
            config.provider
        )));
    }

    // Store sandbox in config
    config.tool_sandbox = sandbox;

    // Try to find agents YAML file
    let agents_path = find_agents_yaml();

    // Create the gateway client
    let gateway = spi::GatewayClient::new(&agents_path, &config.provider, &config.model).await?;

    tracing::info!(
        provider = %config.provider,
        model = %config.model,
        agents_path = %agents_path.display(),
        "AI service initialized via llmboot gateway"
    );

    Ok(DefaultAiService::new(gateway, config))
}

/// Find the agents YAML file.
///
/// Searches in order:
/// 1. `SWEBASH_AGENTS_YAML` environment variable
/// 2. `.swebash/agents.yaml` in current directory
/// 3. Built-in default agents
fn find_agents_yaml() -> std::path::PathBuf {
    // Check environment variable first
    if let Ok(path) = std::env::var("SWEBASH_AGENTS_YAML") {
        let p = std::path::PathBuf::from(&path);
        if p.exists() {
            return p;
        }
    }

    // Check .swebash/agents.yaml in current directory
    if let Ok(cwd) = std::env::current_dir() {
        let local_path = cwd.join(".swebash/agents.yaml");
        if local_path.exists() {
            return local_path;
        }
    }

    // Use crate's built-in agents
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/agents/default.yaml"))
}

// ============================================================================
// llmboot Gateway API Integration
// ============================================================================

/// Factory: create a GatewayClient from an agents YAML file.
///
/// This uses llmboot's L1 Gateway API, which provides:
/// - Input validation and sanitization
/// - Guardrails (injection detection, PII masking)
/// - Agent runtime with pattern execution
/// - Tool orchestration
///
/// # Arguments
/// * `agents_path` - Path to the agents YAML configuration file (llmboot format)
///
/// # Example
/// ```ignore
/// let client = swebash_llm::create_gateway_client(".swebash/agents.yaml").await?;
/// let response = client.execute("shell", "list files in current directory").await?;
/// println!("{}", response.content);
/// ```
pub async fn create_gateway_client(
    agents_path: impl AsRef<std::path::Path>,
) -> AiResult<spi::GatewayClient> {
    let config = AiConfig::from_env();

    if !config.enabled {
        return Err(AiError::NotConfigured(
            "AI features disabled (SWEBASH_AI_ENABLED=false)".into(),
        ));
    }

    if !config.has_api_key() && !config.has_oauth_credentials() {
        return Err(AiError::NotConfigured(format!(
            "No credentials found for provider '{}'. Set the API key env var or configure Claude Code OAuth.",
            config.provider
        )));
    }

    spi::GatewayClient::new(agents_path, &config.provider, &config.model).await
}

/// Factory: create a GatewayClient with explicit provider and model.
///
/// This is useful when you want to override the environment configuration.
///
/// # Arguments
/// * `agents_path` - Path to the agents YAML configuration file
/// * `provider` - LLM provider name (e.g., "openai", "anthropic")
/// * `model` - Model to use (e.g., "gpt-4o", "claude-sonnet-4-20250514")
pub async fn create_gateway_client_with_config(
    agents_path: impl AsRef<std::path::Path>,
    provider: &str,
    model: &str,
) -> AiResult<spi::GatewayClient> {
    spi::GatewayClient::new(agents_path, provider, model).await
}
