/// L5 Facade: swebash-ai crate entry point.
///
/// Re-exports the public API and provides the `create_ai_service()` factory.
///
/// # Architecture (SEA Pattern)
///
/// ```text
/// L5 Facade   - lib.rs (this file): re-exports, factory
/// L4 Core     - core/: DefaultAiService, feature modules
/// L3 API      - api/: AiService trait (consumer interface)
/// L2 SPI      - spi/: AiClient trait + ChatProviderClient (chat/llm-provider)
/// L1 Common   - api/types.rs, api/error.rs: shared types
/// ```

pub mod api;
pub mod config;
pub mod core;
pub mod spi;

// ── Public re-exports (L3 API surface) ──

pub use api::error::{AiError, AiResult};
pub use api::types::{
    AiMessage, AiResponse, AiRole, AiStatus, AutocompleteRequest, AutocompleteResponse,
    ChatRequest, ChatResponse, CompletionOptions, ExplainRequest, ExplainResponse,
    TranslateRequest, TranslateResponse,
};
pub use api::AiService;
pub use config::AiConfig;
pub use core::DefaultAiService;

/// Factory: create the AI service from environment configuration.
///
/// Returns `Ok(service)` if the provider initializes successfully,
/// or `Err` if configuration is missing or invalid.
///
/// Creates a `SimpleChatEngine` (from the `chat` crate) for conversational
/// chat with built-in memory management, and an `AiClient` backed by
/// `llm-provider` for stateless features (translate, explain, autocomplete).
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

    // Build the chat engine from the shared LLM service
    let chat_config = chat_engine::ChatConfig {
        model: config.model.clone(),
        temperature: 0.5,
        max_tokens: 1024,
        system_prompt: Some(core::prompt::chat_system_prompt()),
        max_history: config.history_size,
        enable_summarization: false,
    };
    let chat_engine = chat_engine::SimpleChatEngine::new(client.llm_service(), chat_config);

    Ok(DefaultAiService::new(Box::new(client), chat_engine, config))
}
