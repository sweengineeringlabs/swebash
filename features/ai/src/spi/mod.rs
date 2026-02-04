/// L1 SPI: Provider plugin point.
///
/// The `AiClient` trait abstracts over the underlying LLM provider.
/// `chat_provider.rs` implements this trait, keeping the chat/llm-provider
/// dependency isolated.
pub mod chat_provider;
pub mod config;
pub mod logging;

use async_trait::async_trait;

use crate::api::types::{AiMessage, AiResponse, CompletionOptions};
use crate::api::error::AiResult;

/// L2 SPI trait: plugin point for LLM backends.
///
/// This is the isolation boundary. All core logic programs against
/// this trait. Swapping the LLM backend requires changing only
/// the `chat_provider` module.
#[async_trait]
pub trait AiClient: Send + Sync {
    /// Send a completion request to the LLM.
    async fn complete(
        &self,
        messages: Vec<AiMessage>,
        options: CompletionOptions,
    ) -> AiResult<AiResponse>;

    /// Check if the client is configured and ready.
    async fn is_ready(&self) -> bool;

    /// Human-readable description of the provider and model.
    fn description(&self) -> String;

    /// The provider name (e.g. "openai", "anthropic").
    fn provider_name(&self) -> String;

    /// The model being used (e.g. "gpt-4o").
    fn model_name(&self) -> String;
}
