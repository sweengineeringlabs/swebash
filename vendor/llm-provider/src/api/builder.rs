//! Fluent builder for completion requests

use super::{
    CompletionRequest, CompletionResponse, LlmResult, Message, MessageContent, Role, ToolDefinition,
};
use super::{CompletionStream, LlmService};

/// Builder for constructing completion requests with a fluent API
///
/// # Example
/// ```ignore
/// let response = CompletionBuilder::new("gpt-4o")
///     .system("You are a helpful assistant.")
///     .user("Hello!")
///     .temperature(0.7)
///     .execute(&service)
///     .await?;
/// ```
pub struct CompletionBuilder {
    request: CompletionRequest,
}

impl CompletionBuilder {
    /// Create a new builder for the specified model
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            request: CompletionRequest {
                model: model.into(),
                messages: Vec::new(),
                temperature: None,
                max_tokens: None,
                top_p: None,
                stop: None,
                tools: None,
                tool_choice: None,
            },
        }
    }

    /// Add a system message
    pub fn system(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(Message {
            role: Role::System,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
            cache_control: None,
        });
        self
    }

    /// Add a user message
    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(Message {
            role: Role::User,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
            cache_control: None,
        });
        self
    }

    /// Add an assistant message
    pub fn assistant(mut self, content: impl Into<String>) -> Self {
        self.request.messages.push(Message {
            role: Role::Assistant,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
            cache_control: None,
        });
        self
    }

    /// Add a custom message
    pub fn message(mut self, message: Message) -> Self {
        self.request.messages.push(message);
        self
    }

    /// Set all messages at once
    pub fn messages(mut self, messages: Vec<Message>) -> Self {
        self.request.messages = messages;
        self
    }

    /// Set the temperature (0.0 - 2.0)
    pub fn temperature(mut self, temp: f32) -> Self {
        self.request.temperature = Some(temp);
        self
    }

    /// Set maximum tokens to generate
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.request.max_tokens = Some(tokens);
        self
    }

    /// Set top_p (nucleus sampling)
    pub fn top_p(mut self, top_p: f32) -> Self {
        self.request.top_p = Some(top_p);
        self
    }

    /// Set stop sequences
    pub fn stop(mut self, sequences: Vec<String>) -> Self {
        self.request.stop = Some(sequences);
        self
    }

    /// Set available tools/functions
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.request.tools = Some(tools);
        self
    }

    /// Build the completion request
    pub fn build(self) -> CompletionRequest {
        self.request
    }

    /// Build and execute the request with the provided service
    pub async fn execute<S: LlmService + ?Sized>(
        self,
        service: &S,
    ) -> LlmResult<CompletionResponse> {
        service.complete(self.request).await
    }

    /// Build and execute with streaming
    pub async fn execute_stream<S: LlmService + ?Sized>(
        self,
        service: &S,
    ) -> LlmResult<CompletionStream> {
        service.complete_stream(self.request).await
    }
}

impl Default for CompletionBuilder {
    fn default() -> Self {
        Self::new("gpt-4o")
    }
}
