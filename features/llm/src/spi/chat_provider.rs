/// L2 SPI implementation: delegates to the `chat` and `llm-provider` crates from rustratify.
///
/// This is the ONLY file in swebash-llm that depends on `chat-engine`, `llm-provider`,
/// and `react`. All other modules program against the `AiClient` trait.
use std::sync::Arc;

use async_trait::async_trait;

use crate::api::error::{AiError, AiResult};
use crate::api::types::{AiMessage, AiResponse, AiRole, CompletionOptions};
use super::config::AiConfig;
use crate::spi::AiClient;

use llm_provider::{CompletionBuilder, DefaultLlmService, LlmError, LlmService, LlmServiceBuilder, ProviderConfig};

/// Wrapper around `llm_provider::DefaultLlmService` with `chat` crate integration.
///
/// Holds a shared `Arc<DefaultLlmService>` so the same service instance can be
/// used by both the `AiClient` (for stateless completions) and the
/// `chat::SimpleChatEngine` (for conversational chat with memory).
pub struct ChatProviderClient {
    service: Arc<DefaultLlmService>,
    llm: Arc<dyn LlmService>,
    provider: String,
    model: String,
}

impl ChatProviderClient {
    /// Create a new client from configuration.
    ///
    /// Async because provider initialization is asynchronous. Prefers Claude Code OAuth
    /// credentials for the `anthropic` provider; falls back to `ANTHROPIC_API_KEY` when
    /// OAuth credentials are unavailable.
    pub async fn new(config: &AiConfig) -> AiResult<Self> {
        let service = Arc::new(build_llm_service(config).await?);

        tracing::info!(
            provider = %config.provider,
            model = %config.model,
            "Chat provider client initialized via chat/llm-provider crates"
        );

        let llm = super::logging::LoggingLlmService::wrap(
            service.clone(),
            config.log_dir.clone(),
        );

        Ok(Self {
            service,
            llm,
            provider: config.provider.clone(),
            model: config.model.clone(),
        })
    }

    /// Get the LLM service as an `Arc<dyn LlmService>` for constructing a `SimpleChatEngine`.
    ///
    /// When `log_dir` is configured, this returns the logging-wrapped service
    /// so that all LLM calls (stateless and chat) are logged.
    pub fn llm_service(&self) -> Arc<dyn LlmService> {
        self.llm.clone()
    }
}

/// Build the `DefaultLlmService`, choosing OAuth credentials or API key automatically.
///
/// Priority:
/// 1. Claude Code OAuth credentials from `~/.claude/.credentials.json` (anthropic only)
/// 2. `ANTHROPIC_API_KEY` env var (when provider == "anthropic")
/// 3. `create_service()` env-driven defaults (openai / gemini)
async fn build_llm_service(config: &AiConfig) -> AiResult<DefaultLlmService> {
    if config.provider == "anthropic" {
        if let Ok(oauth) = llm_oauth::from_claude_credentials() {
            Ok(LlmServiceBuilder::new()
                .with_anthropic_oauth(std::sync::Arc::new(oauth), None)
                .build()
                .await)
        } else if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            let mut pc = ProviderConfig::default();
            pc.api_key = Some(key);
            Ok(LlmServiceBuilder::new().with_anthropic(pc).build().await)
        } else {
            Err(AiError::NotConfigured(
                "No credentials found for provider 'anthropic'. Configure Claude Code OAuth or set ANTHROPIC_API_KEY.".into()
            ))
        }
    } else {
        llm_provider::create_service().await.map_err(map_llm_error)
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
            .execute(&*self.llm)
            .await
            .map_err(map_llm_error)?;

        Ok(AiResponse {
            content: response.content.unwrap_or_default(),
            model: response.model,
        })
    }

    async fn is_ready(&self) -> bool {
        !self.service.providers().await.is_empty()
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
