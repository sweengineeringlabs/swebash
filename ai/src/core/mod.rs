/// L4 Core: DefaultAiService orchestration.
///
/// Wires the SPI client to the API service trait, delegating
/// to feature-specific modules for each operation.
///
/// The chat feature uses a ChatEngine implementation (SimpleChatEngine
/// or ToolAwareChatEngine) for conversation management with built-in memory.
/// Stateless features (translate, explain, autocomplete) use the `AiClient` directly.
pub mod chat;
pub mod complete;
pub mod explain;
pub mod prompt;
pub mod translate;
pub mod tools;

use std::sync::Arc;

use async_trait::async_trait;

use crate::api::error::{AiError, AiResult};
use crate::api::types::*;
use crate::api::AiService;
use crate::config::AiConfig;
use crate::spi::AiClient;

use chat_engine::ChatEngine;

/// The default implementation of `AiService`.
///
/// Holds the LLM client for stateless features and a `ChatEngine`
/// implementation for conversational chat with memory management.
/// The engine uses the provider pattern - can be SimpleChatEngine
/// or ToolAwareChatEngine depending on configuration.
pub struct DefaultAiService {
    client: Box<dyn AiClient>,
    config: AiConfig,
    chat_engine: Arc<dyn ChatEngine>,
}

impl DefaultAiService {
    /// Create a new service with the given client, chat engine, and config.
    pub fn new(
        client: Box<dyn AiClient>,
        chat_engine: Arc<dyn ChatEngine>,
        config: AiConfig,
    ) -> Self {
        Self {
            client,
            config,
            chat_engine,
        }
    }
}

#[async_trait]
impl AiService for DefaultAiService {
    async fn translate(&self, request: TranslateRequest) -> AiResult<TranslateResponse> {
        self.ensure_ready().await?;
        translate::translate(self.client.as_ref(), request).await
    }

    async fn explain(&self, request: ExplainRequest) -> AiResult<ExplainResponse> {
        self.ensure_ready().await?;
        explain::explain(self.client.as_ref(), request).await
    }

    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        self.ensure_ready().await?;
        chat::chat(self.chat_engine.as_ref(), request).await
    }

    async fn chat_streaming(
        &self,
        request: ChatRequest,
    ) -> AiResult<tokio::sync::mpsc::Receiver<ChatStreamEvent>> {
        self.ensure_ready().await?;
        chat::chat_streaming(&self.chat_engine, request).await
    }

    async fn autocomplete(&self, request: AutocompleteRequest) -> AiResult<AutocompleteResponse> {
        self.ensure_ready().await?;
        complete::autocomplete(self.client.as_ref(), request).await
    }

    async fn is_available(&self) -> bool {
        self.config.enabled && self.client.is_ready().await
    }

    async fn status(&self) -> AiStatus {
        AiStatus {
            enabled: self.config.enabled,
            provider: self.client.provider_name(),
            model: self.client.model_name(),
            ready: self.client.is_ready().await,
            description: self.client.description(),
        }
    }
}

impl DefaultAiService {
    async fn ensure_ready(&self) -> AiResult<()> {
        if !self.config.enabled {
            return Err(AiError::NotConfigured("AI features are disabled. Set SWEBASH_AI_ENABLED=true to enable.".into()));
        }
        if !self.client.is_ready().await {
            return Err(AiError::NotConfigured(
                "AI provider is not ready. Check your API key and provider configuration.".into(),
            ));
        }
        Ok(())
    }

    /// Get a formatted display of conversation history.
    pub async fn format_history(&self) -> String {
        let messages = self
            .chat_engine
            .memory()
            .get_all_messages()
            .await
            .unwrap_or_default();

        let mut output = String::new();
        for msg in &messages {
            let role_label = match msg.role {
                chat_engine::ChatRole::System => continue,
                chat_engine::ChatRole::User => "You",
                chat_engine::ChatRole::Assistant => "AI",
            };
            output.push_str(&format!("[{}] {}\n", role_label, msg.content));
        }
        if output.is_empty() {
            output.push_str("(no chat history)");
        }
        output
    }

    /// Clear conversation history.
    pub async fn clear_history(&self) {
        let _ = self.chat_engine.new_conversation().await;
    }
}
