#![forbid(unsafe_code)]

/// L5 Facade: swebash-llm crate entry point.
///
/// Re-exports the public API and provides the `create_ai_service()` factory.
///
/// # Architecture (SEA Pattern)
///
/// ```text
/// L4 Facade   - lib.rs (this file): re-exports, factory
/// L3 Core     - core/: DefaultAiService, feature modules, agent framework
/// L2 API      - api/: AiService trait (consumer interface)
/// L1 SPI      - spi/: AiClient trait, config, ChatProviderClient
/// ```
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
pub use core::DefaultAiService;
pub use core::tools::{ToolSandbox, SandboxAccessMode, SandboxRule};

/// Factory: create the AI service from environment configuration.
///
/// Returns `Ok(service)` if the provider initializes successfully,
/// or `Err` if configuration is missing or invalid.
///
/// Creates an `AgentRegistry` with built-in agents (shell, review, devops, git),
/// each with its own `ChatEngine` (lazily created on first use).
/// Stateless features (translate, explain, autocomplete) use the `AiClient` directly.
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
/// When `sandbox` is provided, filesystem and command executor tools are
/// wrapped to enforce path restrictions, preventing AI tools from accessing
/// files outside the allowed workspace.
///
/// ## Mock Provider
///
/// When `LLM_PROVIDER=mock`, creates a mock service that doesn't require
/// API keys. This enables deterministic agent behavior testing in autotest.
///
/// Mock behaviour is configured via environment variables:
/// - `SWEBASH_MOCK_RESPONSE`: Fixed response text
/// - `SWEBASH_MOCK_RESPONSE_FILE`: Path to file containing response
/// - `SWEBASH_MOCK_ERROR`: Force error mode
pub async fn create_ai_service_with_sandbox(
    sandbox: Option<std::sync::Arc<ToolSandbox>>,
) -> AiResult<DefaultAiService> {
    let mut config = AiConfig::from_env();

    if !config.enabled {
        return Err(AiError::NotConfigured(
            "AI features disabled (SWEBASH_AI_ENABLED=false)".into(),
        ));
    }

    // Mock provider path: no API key required
    if config.provider == "mock" {
        config.tool_sandbox = sandbox;
        return create_mock_ai_service(config).await;
    }

    if !config.has_api_key() && !config.has_oauth_credentials() {
        return Err(AiError::NotConfigured(format!(
            "No credentials found for provider '{}'. Set the API key env var or configure Claude Code OAuth.",
            config.provider
        )));
    }

    // Store sandbox in config for agent registry to use
    config.tool_sandbox = sandbox;

    // Create the SPI client (initializes the LLM provider)
    let client = spi::chat_provider::ChatProviderClient::new(&config).await?;
    let llm = client.llm_service();

    // Wrap with AiClient-level logging when log_dir is configured.
    // LoggingLlmService (inside ChatProviderClient) handles lower-level LLM
    // request/response logging; LoggingAiClient logs the higher-level
    // AiMessage / CompletionOptions / AiResponse boundary.
    let client = spi::logging::LoggingAiClient::wrap(Box::new(client), config.log_dir.clone());

    // Build the agent registry with built-in agents
    let agents = core::agents::builtins::create_default_registry(llm, config.clone());

    Ok(DefaultAiService::new(client, agents, config))
}

/// Create a mock AI service for testing.
///
/// Uses `MockAiClient` which delegates to `MockLlmService`. Behaviour
/// is determined by environment variables (see `mock_client` module).
async fn create_mock_ai_service(config: AiConfig) -> AiResult<DefaultAiService> {
    use spi::mock_client::MockAiClient;

    let mock_client = MockAiClient::new();
    let llm = mock_client.llm_service();

    // Wrap with logging if configured
    let client = spi::logging::LoggingAiClient::wrap(Box::new(mock_client), config.log_dir.clone());

    // Build the agent registry with built-in agents
    let agents = core::agents::builtins::create_default_registry(llm, config.clone());

    tracing::info!(
        provider = "mock",
        model = "mock-model",
        "Mock AI service initialized for testing"
    );

    Ok(DefaultAiService::new(client, agents, config))
}
