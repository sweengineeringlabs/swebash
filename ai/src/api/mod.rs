/// L3 API: Consumer interface for AI features.
pub mod error;
pub mod types;

use async_trait::async_trait;

pub use error::{AiError, AiResult};
pub use types::*;

/// L3 API trait: the interface consumed by the host crate.
///
/// All AI features are exposed through this single trait.
/// The host never interacts with the LLM provider directly.
#[async_trait]
pub trait AiService: Send + Sync {
    /// Translate natural language to a shell command.
    async fn translate(&self, request: TranslateRequest) -> AiResult<TranslateResponse>;

    /// Explain what a shell command does.
    async fn explain(&self, request: ExplainRequest) -> AiResult<ExplainResponse>;

    /// Conversational chat with the AI assistant.
    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse>;

    /// Streaming conversational chat — returns a receiver of incremental events.
    async fn chat_streaming(
        &self,
        request: ChatRequest,
    ) -> AiResult<tokio::sync::mpsc::Receiver<ChatStreamEvent>>;

    /// Get autocomplete suggestions based on context.
    async fn autocomplete(&self, request: AutocompleteRequest) -> AiResult<AutocompleteResponse>;

    /// Check if the AI service is available and ready.
    async fn is_available(&self) -> bool;

    /// Get the current status of the AI service.
    async fn status(&self) -> AiStatus;

    /// Switch to a different agent by ID.
    async fn switch_agent(&self, agent_id: &str) -> AiResult<()>;

    /// Get information about the currently active agent.
    async fn current_agent(&self) -> AgentInfo;

    /// List all registered agents with their status.
    async fn list_agents(&self) -> Vec<AgentInfo>;
}
