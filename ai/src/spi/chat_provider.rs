/// L2 SPI implementation: delegates to the `chat` and `llm-provider` crates from rustratify.
///
/// This is the ONLY file in swebash-ai that depends on `chat-engine`, `llm-provider`,
/// and `react`. All other modules program against the `AiClient` trait.
use std::sync::Arc;

use async_trait::async_trait;

use crate::api::error::{AiError, AiResult};
use crate::api::types::{AiMessage, AiResponse, AiRole, CompletionOptions};
use crate::config::AiConfig;
use crate::spi::AiClient;

use llm_provider::{CompletionBuilder, DefaultLlmService, LlmError, LlmService};

/// Wrapper around `llm_provider::DefaultLlmService` with `chat` crate integration.
///
/// Holds a shared `Arc<DefaultLlmService>` so the same service instance can be
/// used by both the `AiClient` (for stateless completions) and the
/// `chat::SimpleChatEngine` (for conversational chat with memory).
pub struct ChatProviderClient {
    service: Arc<DefaultLlmService>,
    provider: String,
    model: String,
}

impl ChatProviderClient {
    /// Create a new client from configuration.
    ///
    /// Async because `llm_provider::create_service()` initializes providers asynchronously.
    pub async fn new(config: &AiConfig) -> AiResult<Self> {
        let service = llm_provider::create_service()
            .await
            .map_err(map_llm_error)?;

        tracing::info!(
            provider = %config.provider,
            model = %config.model,
            "Chat provider client initialized via chat/llm-provider crates"
        );

        Ok(Self {
            service: Arc::new(service),
            provider: config.provider.clone(),
            model: config.model.clone(),
        })
    }

    /// Get the LLM service as an `Arc<dyn LlmService>` for constructing a `SimpleChatEngine`.
    pub fn llm_service(&self) -> Arc<dyn LlmService> {
        self.service.clone()
    }
}

/// Convert `LlmError` to `AiError`.
pub fn map_llm_error(err: LlmError) -> AiError {
    match err {
        LlmError::Configuration(msg) => AiError::NotConfigured(msg),
        LlmError::RateLimited { .. } => AiError::RateLimited,
        LlmError::Timeout(_) => AiError::Timeout,
        LlmError::NetworkError(msg) => AiError::Provider(format!("Network error: {}", msg)),
        LlmError::SerializationError(msg) => AiError::ParseError(msg),
        other => AiError::Provider(other.to_string()),
    }
}

#[async_trait]
impl AiClient for ChatProviderClient {
    async fn complete(
        &self,
        messages: Vec<AiMessage>,
        options: CompletionOptions,
    ) -> AiResult<AiResponse> {
        let mut builder = CompletionBuilder::new(&self.model);

        for msg in messages {
            builder = match msg.role {
                AiRole::System => builder.system(msg.content),
                AiRole::User => builder.user(msg.content),
                AiRole::Assistant => builder.assistant(msg.content),
            };
        }

        if let Some(temp) = options.temperature {
            builder = builder.temperature(temp);
        }
        if let Some(max) = options.max_tokens {
            builder = builder.max_tokens(max);
        }

        let response = builder
            .execute(&*self.service)
            .await
            .map_err(map_llm_error)?;

        Ok(AiResponse {
            content: response.content.unwrap_or_default(),
            model: response.model,
        })
    }

    fn is_ready(&self) -> bool {
        !self.service.providers().is_empty()
    }

    fn description(&self) -> String {
        format!("{}:{}", self.provider, self.model)
    }

    fn provider_name(&self) -> String {
        self.provider.clone()
    }

    fn model_name(&self) -> String {
        self.model.clone()
    }
}
