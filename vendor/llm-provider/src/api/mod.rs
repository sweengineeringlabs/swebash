//! LLM API - Types, errors, and service contract

mod types;
mod error;
mod builder;

use async_trait::async_trait;
use futures::stream::BoxStream;

// Re-export types
pub use types::{
    CompletionRequest, CompletionResponse, Message, MessageContent, Role,
    ModelInfo, TokenUsage, FinishReason, ToolCall, ToolCallDelta, ToolChoice, ToolDefinition,
    ContentPart, ImageUrl, StreamChunk, StreamDelta, CacheControl, CacheableMessage,
};

// Re-export errors
pub use error::{LlmError, LlmResult};

// Re-export builder
pub use builder::CompletionBuilder;

/// Stream type for completion events
pub type CompletionStream = BoxStream<'static, LlmResult<StreamChunk>>;

/// Main LLM service interface
///
/// This is the primary interface for interacting with LLMs.
/// It abstracts away provider-specific details and provides
/// a unified async API.
///
/// # Example
/// ```ignore
/// let response = service.complete(request).await?;
/// println!("Response: {:?}", response.content);
/// ```
#[async_trait]
pub trait LlmService: Send + Sync {
    /// Complete a request
    async fn complete(&self, request: CompletionRequest) -> LlmResult<CompletionResponse>;

    /// Complete with streaming
    async fn complete_stream(&self, request: CompletionRequest) -> LlmResult<CompletionStream>;

    /// List available models across all providers
    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>>;

    /// Get information about a specific model
    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo>;

    /// Check if a model is available
    async fn is_model_available(&self, model: &str) -> bool;

    /// Get names of active providers
    async fn providers(&self) -> Vec<String>;
}
