/// L5 Facade: swebash-ai crate entry point.
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
    AgentInfo, AiMessage, AiResponse, AiRole, AiStatus, AutocompleteRequest, AutocompleteResponse,
    ChatRequest, ChatResponse, ChatStreamEvent, CompletionOptions, ExplainRequest,
    ExplainResponse, TranslateRequest, TranslateResponse,
};
pub use api::AiService;
pub use api::cli::handle_ai_command;
pub use api::commands::{self, AiCommand};
pub use api::output;
pub use spi::config::{AiConfig, ToolConfig};
pub use core::DefaultAiService;

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
/// let ai_service = swebash_ai::create_ai_service().ok();
/// ```
pub async fn create_ai_service() -> AiResult<DefaultAiService> {
    let config = AiConfig::from_env();

    if !config.enabled {
        return Err(AiError::NotConfigured(
            "AI features disabled (SWEBASH_AI_ENABLED=false)".into(),
        ));
    }

    if !config.has_api_key() {
        return Err(AiError::NotConfigured(format!(
            "No API key found for provider '{}'. Set the appropriate environment variable.",
            config.provider
        )));
    }

    // Create the SPI client (initializes the LLM provider)
    let client = spi::chat_provider::ChatProviderClient::new(&config).await?;
    let llm = client.llm_service();

    // Build the agent registry with built-in agents
    let agents = core::agents::builtins::create_default_registry(llm, config.clone());

    Ok(DefaultAiService::new(Box::new(client), agents, config))
}
