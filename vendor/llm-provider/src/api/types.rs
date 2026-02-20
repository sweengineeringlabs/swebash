use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Cache control for prompt caching (Anthropic API)
///
/// Used to mark content for caching to reduce costs on repeated requests.
/// Currently supports "ephemeral" type which caches content for the
/// duration of the conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: String,
}

impl CacheControl {
    /// Create an ephemeral cache control marker
    pub fn ephemeral() -> Self {
        Self {
            cache_type: "ephemeral".to_string(),
        }
    }

    /// Create a custom cache control with a specific type
    pub fn custom(cache_type: impl Into<String>) -> Self {
        Self {
            cache_type: cache_type.into(),
        }
    }

    /// Check if this is an ephemeral cache control
    pub fn is_ephemeral(&self) -> bool {
        self.cache_type == "ephemeral"
    }
}

/// Trait for adding cache control to messages
///
/// Provides a fluent API for adding cache control markers to messages.
pub trait CacheableMessage: Sized {
    /// Add a cache control marker to this message
    fn with_cache_control(self, cache: CacheControl) -> Self;

    /// Mark this message for ephemeral caching
    fn mark_ephemeral(self) -> Self {
        self.with_cache_control(CacheControl::ephemeral())
    }
}

impl CacheableMessage for Message {
    fn with_cache_control(mut self, cache: CacheControl) -> Self {
        self.cache_control = Some(cache);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool calls made by the assistant (for conversation history)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Cache control for prompt caching (Anthropic API)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    ImageBase64 { data: String, media_type: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    /// Tokens read from cache (Anthropic API)
    #[serde(default)]
    pub cache_read_input_tokens: u32,
    /// Tokens written to cache (Anthropic API)
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_window: u32,
    pub supports_vision: bool,
    pub supports_function_calling: bool,
    pub supports_streaming: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Function { name: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub delta: StreamDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}
