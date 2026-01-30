/// L4 Core: DefaultAiService orchestration.
///
/// Wires the SPI client to the API service trait, delegating
/// to feature-specific modules for each operation.
pub mod chat;
pub mod complete;
pub mod explain;
pub mod history;
pub mod prompt;
pub mod translate;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::api::error::{AiError, AiResult};
use crate::api::types::*;
use crate::api::AiService;
use crate::config::AiConfig;
use crate::spi::AiClient;

/// The default implementation of `AiService`.
///
/// Holds the LLM client and conversation history.
/// All AI features are delegated to the core sub-modules.
pub struct DefaultAiService {
    client: Box<dyn AiClient>,
    config: AiConfig,
    history: Mutex<history::ConversationHistory>,
}

impl DefaultAiService {
    /// Create a new service with the given client and config.
    pub fn new(client: Box<dyn AiClient>, config: AiConfig) -> Self {
        let history = history::ConversationHistory::new(config.history_size);
        Self {
            client,
            config,
            history: Mutex::new(history),
        }
    }
}

#[async_trait]
impl AiService for DefaultAiService {
    async fn translate(&self, request: TranslateRequest) -> AiResult<TranslateResponse> {
        self.ensure_ready()?;
        translate::translate(self.client.as_ref(), request).await
    }

    async fn explain(&self, request: ExplainRequest) -> AiResult<ExplainResponse> {
        self.ensure_ready()?;
        explain::explain(self.client.as_ref(), request).await
    }

    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        self.ensure_ready()?;
        let mut history = self.history.lock().await;
        chat::chat(self.client.as_ref(), request, &mut history).await
    }

    async fn autocomplete(&self, request: AutocompleteRequest) -> AiResult<AutocompleteResponse> {
        self.ensure_ready()?;
        complete::autocomplete(self.client.as_ref(), request).await
    }

    fn is_available(&self) -> bool {
        self.config.enabled && self.client.is_ready()
    }

    fn status(&self) -> AiStatus {
        AiStatus {
            enabled: self.config.enabled,
            provider: self.client.provider_name(),
            model: self.client.model_name(),
            ready: self.client.is_ready(),
            description: self.client.description(),
        }
    }
}

impl DefaultAiService {
    fn ensure_ready(&self) -> AiResult<()> {
        if !self.config.enabled {
            return Err(AiError::NotConfigured("AI features are disabled. Set SWEBASH_AI_ENABLED=true to enable.".into()));
        }
        if !self.client.is_ready() {
            return Err(AiError::NotConfigured(
                "AI provider is not ready. Check your API key and provider configuration.".into(),
            ));
        }
        Ok(())
    }

    /// Get a formatted display of conversation history.
    pub async fn format_history(&self) -> String {
        let history = self.history.lock().await;
        history.format_display()
    }

    /// Clear conversation history.
    pub async fn clear_history(&self) {
        let mut history = self.history.lock().await;
        history.clear();
    }
}
