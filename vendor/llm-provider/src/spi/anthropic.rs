//! Anthropic provider implementation

use async_trait::async_trait;
use futures::stream::BoxStream;
use crate::api::{
    CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmResult, Message,
    MessageContent, ModelInfo, Role, StreamChunk, StreamDelta, TokenUsage, ToolCall,
    ToolCallDelta, ToolDefinition,
};
use crate::config::{keys, ProviderConfig};
use super::AsyncLlmProvider;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::time::Duration;
use tracing::{debug, warn};

#[cfg(feature = "oauth")]
use std::sync::Arc;
#[cfg(feature = "oauth")]
use llm_oauth::OAuthService;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01"; // Keep this version for backward compatibility
/// Required beta header when authenticating via OAuth.
#[cfg(feature = "oauth")]
const OAUTH_BETA_HEADER: &str = "oauth-2025-04-20";

// =============================================================================
// Authentication strategy
// =============================================================================

pub(crate) enum AnthropicAuth {
    ApiKey(String),
    #[cfg(feature = "oauth")]
    OAuthBearer(Arc<dyn OAuthService>),
}

impl AnthropicAuth {
    async fn get_headers(&self) -> LlmResult<Vec<(&'static str, String)>> {
        match self {
            Self::ApiKey(key) => Ok(vec![("x-api-key", key.clone())]),
            #[cfg(feature = "oauth")]
            Self::OAuthBearer(svc) => {
                let token = svc
                    .get_access_token()
                    .await
                    .map_err(|e| LlmError::AuthenticationFailed(e.to_string()))?;
                Ok(vec![
                    ("Authorization", format!("Bearer {}", token)),
                    ("anthropic-beta", OAUTH_BETA_HEADER.to_string()),
                ])
            }
        }
    }

    fn is_configured(&self) -> bool {
        match self {
            Self::ApiKey(key) => !key.is_empty(),
            #[cfg(feature = "oauth")]
            Self::OAuthBearer(_) => true,
        }
    }
}

impl Clone for AnthropicAuth {
    fn clone(&self) -> Self {
        match self {
            Self::ApiKey(k) => Self::ApiKey(k.clone()),
            #[cfg(feature = "oauth")]
            Self::OAuthBearer(svc) => Self::OAuthBearer(svc.clone()),
        }
    }
}

impl std::fmt::Debug for AnthropicAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiKey(_) => write!(f, "AnthropicAuth::ApiKey(<redacted>)"),
            #[cfg(feature = "oauth")]
            Self::OAuthBearer(_) => write!(f, "AnthropicAuth::OAuthBearer(<service>)"),
        }
    }
}

// =============================================================================
// AnthropicProvider
// =============================================================================

/// Anthropic provider implementation
#[derive(Debug)]
pub struct AnthropicProvider {
    client: Client,
    auth: AnthropicAuth,
    base_url: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider from environment
    ///
    /// Reads ANTHROPIC_API_KEY from environment
    pub fn from_env() -> LlmResult<Self> {
        let api_key = std::env::var(keys::ANTHROPIC_API_KEY).map_err(|_| {
            LlmError::AuthenticationFailed(format!("{} not set", keys::ANTHROPIC_API_KEY))
        })?;

        let base_url = std::env::var(keys::ANTHROPIC_BASE_URL)
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        Ok(Self::new(ProviderConfig {
            name: "anthropic".to_string(),
            api_key: Some(api_key),
            base_url: Some(base_url),
            timeout_ms: 60_000,
            max_retries: 3,
            extra: Default::default(),
        }))
    }

    /// Create a new Anthropic provider with explicit configuration
    pub fn new(config: ProviderConfig) -> Self {
        let api_key = config.api_key.clone().unwrap_or_default();
        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let timeout = Duration::from_millis(config.timeout_ms);
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            auth: AnthropicAuth::ApiKey(api_key),
            base_url,
        }
    }

    /// Create a new Anthropic provider that authenticates via OAuth bearer tokens.
    ///
    /// The `oauth` service is called on every request to obtain a fresh (or cached) token.
    #[cfg(feature = "oauth")]
    pub fn with_oauth(oauth: Arc<dyn OAuthService>, base_url: Option<String>) -> Self {
        let base_url = base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let client = Client::builder()
            .timeout(Duration::from_millis(60_000))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            auth: AnthropicAuth::OAuthBearer(oauth),
            base_url,
        }
    }

    /// Convert our Message format to Anthropic format
    /// Anthropic puts system messages in a separate field
    /// Handles cache_control by using content blocks when present
    fn convert_messages(&self, messages: &[Message]) -> (Option<Vec<SystemBlock>>, Vec<AnthropicMessage>) {
        let mut system_blocks: Vec<SystemBlock> = Vec::new();
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    if let MessageContent::Text(text) = &msg.content {
                        let cache_control = msg.cache_control.as_ref().map(|cc| CacheControlMarker {
                            cache_type: cc.cache_type.clone(),
                        });
                        system_blocks.push(SystemBlock {
                            block_type: "text".to_string(),
                            text: text.clone(),
                            cache_control,
                        });
                    }
                }
                Role::User | Role::Assistant => {
                    let text_content = match &msg.content {
                        MessageContent::Text(text) => text.clone(),
                        MessageContent::Parts(_) => {
                            // For now, flatten to text. TODO: Support multimodal content
                            warn!("Multimodal content not yet fully supported");
                            String::new()
                        }
                    };

                    // For assistant messages with tool_calls, include tool_use blocks
                    // This is required by Anthropic API when tool_result follows
                    let content = if msg.role == Role::Assistant && !msg.tool_calls.is_empty() {
                        // Build content blocks: text (if any) + tool_use blocks
                        let mut blocks: Vec<AssistantContentBlock> = Vec::new();

                        if !text_content.is_empty() {
                            blocks.push(AssistantContentBlock::Text { text: text_content });
                        }

                        for tc in &msg.tool_calls {
                            let input: serde_json::Value = serde_json::from_str(&tc.arguments)
                                .unwrap_or(serde_json::Value::Object(Default::default()));
                            blocks.push(AssistantContentBlock::ToolUse {
                                id: tc.id.clone(),
                                name: tc.name.clone(),
                                input,
                            });
                        }

                        AnthropicMessageContent::AssistantBlocks(blocks)
                    } else if let Some(cc) = &msg.cache_control {
                        AnthropicMessageContent::Blocks(vec![ContentBlock {
                            block_type: "text".to_string(),
                            text: text_content,
                            cache_control: Some(CacheControlMarker {
                                cache_type: cc.cache_type.clone(),
                            }),
                        }])
                    } else {
                        AnthropicMessageContent::Text(text_content)
                    };

                    anthropic_messages.push(AnthropicMessage {
                        role: match msg.role {
                            Role::User => "user".to_string(),
                            Role::Assistant => "assistant".to_string(),
                            _ => "user".to_string(),
                        },
                        content,
                    });
                }
                Role::Tool => {
                    // Tool results are sent as user messages with tool_result content blocks
                    let text_content = match &msg.content {
                        MessageContent::Text(text) => text.clone(),
                        MessageContent::Parts(_) => String::new(),
                    };

                    if let Some(tool_call_id) = &msg.tool_call_id {
                        anthropic_messages.push(AnthropicMessage {
                            role: "user".to_string(),
                            content: AnthropicMessageContent::ToolResult(vec![ToolResultBlock {
                                block_type: "tool_result".to_string(),
                                tool_use_id: tool_call_id.clone(),
                                content: text_content,
                            }]),
                        });
                    } else {
                        warn!("Tool message without tool_call_id, skipping");
                    }
                }
            }
        }

        let system = if system_blocks.is_empty() {
            None
        } else {
            Some(system_blocks)
        };

        (system, anthropic_messages)
    }

    /// Convert Anthropic response to our format
    fn convert_response(&self, model: &str, response: AnthropicResponse) -> CompletionResponse {
        // Extract text content and tool calls from response
        let mut text_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        for content_block in &response.content {
            match content_block {
                AnthropicContent::Text { text } => {
                    text_parts.push(text.clone());
                }
                AnthropicContent::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string()),
                    });
                }
            }
        }

        let content = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join("\n"))
        };

        let finish_reason = match response.stop_reason.as_deref() {
            Some("end_turn") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("stop_sequence") => FinishReason::Stop,
            Some("tool_use") => FinishReason::ToolCalls,
            _ => FinishReason::Error,
        };

        CompletionResponse {
            id: response.id,
            model: model.to_string(),
            content,
            tool_calls,
            finish_reason,
            usage: TokenUsage {
                prompt_tokens: response.usage.input_tokens,
                completion_tokens: response.usage.output_tokens,
                total_tokens: response.usage.input_tokens + response.usage.output_tokens,
                cache_read_input_tokens: response.usage.cache_read_input_tokens,
                cache_creation_input_tokens: response.usage.cache_creation_input_tokens,
            },
        }
    }

    /// Convert tool definitions to Anthropic format
    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<AnthropicTool> {
        tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.parameters.clone(),
            })
            .collect()
    }

    /// Map HTTP status to LlmError
    fn map_error(&self, status: reqwest::StatusCode, body: &str) -> LlmError {
        match status.as_u16() {
            401 => LlmError::AuthenticationFailed(body.to_string()),
            429 => LlmError::RateLimited {
                retry_after_ms: None,
            },
            400 => {
                // Check for context length exceeded error
                // Anthropic format: "prompt is too long: 201814 tokens > 200000 maximum"
                if body.contains("prompt is too long") || body.contains("tokens >") {
                    // Try to parse token counts
                    if let Some(counts) = Self::parse_token_counts(body) {
                        return LlmError::ContextLengthExceeded {
                            used: counts.0,
                            max: counts.1,
                        };
                    }
                }
                LlmError::InvalidRequest(body.to_string())
            }
            500..=599 => LlmError::ProviderError {
                provider: "anthropic".to_string(),
                message: body.to_string(),
            },
            _ => LlmError::NetworkError(format!("HTTP {}: {}", status, body)),
        }
    }

    /// Parse token counts from error message like "201814 tokens > 200000 maximum"
    fn parse_token_counts(body: &str) -> Option<(u32, u32)> {
        // Find "N tokens > M" pattern
        // Example: "prompt is too long: 201814 tokens > 200000 maximum"
        let tokens_idx = body.find("tokens")?;
        let before_tokens = &body[..tokens_idx];

        // Extract the number before "tokens"
        let used: u32 = before_tokens
            .split_whitespace()
            .last()?
            .parse()
            .ok()?;

        // Find the number after ">"
        let after_gt = body.find('>')?.checked_add(1)?;
        let max: u32 = body[after_gt..]
            .split_whitespace()
            .next()?
            .parse()
            .ok()?;

        Some((used, max))
    }
}

#[async_trait]
impl AsyncLlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn models(&self) -> &[&str] {
        &[
            "claude-sonnet-4-20250514",
            "claude-3-5-sonnet-20241022",
            "claude-3-5-haiku-20241022",
            "claude-3-opus-20240229",
            "claude-3-haiku-20240307",
        ]
    }

    fn model_info(&self, model: &str) -> Option<ModelInfo> {
        let context_window = match model {
            "claude-sonnet-4-20250514" => 200_000,
            "claude-3-5-sonnet-20241022" => 200_000,
            "claude-3-5-haiku-20241022" => 200_000,
            "claude-3-opus-20240229" => 200_000,
            "claude-3-haiku-20240307" => 200_000,
            _ => return None,
        };

        Some(ModelInfo {
            id: model.to_string(),
            name: model.to_string(),
            provider: "anthropic".to_string(),
            context_window,
            supports_vision: true,
            supports_function_calling: true,
            supports_streaming: true,
        })
    }

    fn is_configured(&self) -> bool {
        self.auth.is_configured()
    }

    async fn complete(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse> {
        debug!("Anthropic complete: model={}", request.model);

        let (system, messages) = self.convert_messages(&request.messages);
        let tools = request
            .tools
            .as_ref()
            .map(|t| self.convert_tools(t))
            .filter(|t| !t.is_empty());

        let anthropic_request = AnthropicRequest {
            model: request.model.clone(),
            messages,
            system,
            tools,
            max_tokens: request.max_tokens.unwrap_or(4096),
            temperature: request.temperature,
            top_p: request.top_p,
            stop_sequences: request.stop.clone(),
            stream: false,
        };

        let url = format!("{}/messages", self.base_url);
        let auth_headers = self.auth.get_headers().await?;
        let mut req = self
            .client
            .post(&url)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header(header::CONTENT_TYPE, "application/json");
        for (k, v) in &auth_headers {
            req = req.header(*k, v);
        }
        let response = req
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body));
        }

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| LlmError::SerializationError(e.to_string()))?;

        Ok(self.convert_response(&request.model, anthropic_response))
    }

    fn complete_stream(&self, request: &CompletionRequest) -> BoxStream<'static, LlmResult<StreamChunk>> {
        debug!("Anthropic complete_stream: model={}", request.model);

        let (system, messages) = self.convert_messages(&request.messages);
        let tools = request
            .tools
            .as_ref()
            .map(|t| self.convert_tools(t))
            .filter(|t| !t.is_empty());

        let anthropic_request = AnthropicRequest {
            model: request.model.clone(),
            messages,
            system,
            tools,
            max_tokens: request.max_tokens.unwrap_or(4096),
            temperature: request.temperature,
            top_p: request.top_p,
            stop_sequences: request.stop.clone(),
            stream: true,
        };

        let url = format!("{}/messages", self.base_url);
        let client = self.client.clone();
        let auth = self.auth.clone();

        Box::pin(async_stream::stream! {
            let auth_headers = match auth.get_headers().await {
                Ok(h) => h,
                Err(e) => { yield Err(e); return; }
            };
            let mut req = client
                .post(&url)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header(header::CONTENT_TYPE, "application/json");
            for (k, v) in &auth_headers {
                req = req.header(*k, v);
            }
            let response = req
                .json(&anthropic_request)
                .send()
                .await;

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    yield Err(LlmError::NetworkError(e.to_string()));
                    return;
                }
            };

            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                yield Err(LlmError::ProviderError {
                    provider: "anthropic".to_string(),
                    message: format!("HTTP {}: {}", status, body),
                });
                return;
            }

            // Stream the response
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut current_id = String::from("anthropic-stream");

            while let Some(chunk_result) = futures::StreamExt::next(&mut bytes_stream).await {
                let chunk_bytes = match chunk_result {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        yield Err(LlmError::NetworkError(e.to_string()));
                        return;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk_bytes));

                // Process complete lines from buffer
                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    // Anthropic SSE format: "event: <type>" followed by "data: {...}"
                    if let Some(event_type) = line.strip_prefix("event: ") {
                        // Handle different event types
                        match event_type {
                            "message_start" | "content_block_start" | "ping" => {
                                // Skip these events, wait for data
                            }
                            "content_block_delta" => {
                                // Next line should have the data
                            }
                            "message_delta" | "content_block_stop" => {
                                // Skip these
                            }
                            "message_stop" => {
                                return;
                            }
                            _ => {
                                debug!("Unknown Anthropic event type: {}", event_type);
                            }
                        }
                    } else if let Some(data) = line.strip_prefix("data: ") {
                        // Parse the JSON data
                        match serde_json::from_str::<AnthropicStreamEvent>(data) {
                            Ok(event) => {
                                match event {
                                    AnthropicStreamEvent::MessageStart { message } => {
                                        current_id = message.id;
                                    }
                                    AnthropicStreamEvent::ContentBlockStart {
                                        index,
                                        content_block: AnthropicStreamContentBlock::ToolUse { id, name },
                                    } => {
                                        // Emit tool call header (id + name)
                                        yield Ok(StreamChunk {
                                            id: current_id.clone(),
                                            delta: StreamDelta {
                                                content: None,
                                                tool_calls: Some(vec![ToolCallDelta {
                                                    index,
                                                    id: Some(id),
                                                    name: Some(name),
                                                    arguments: None,
                                                }]),
                                            },
                                            finish_reason: None,
                                        });
                                    }
                                    AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                                        match delta {
                                            AnthropicDelta::TextDelta { text } => {
                                                yield Ok(StreamChunk {
                                                    id: current_id.clone(),
                                                    delta: StreamDelta {
                                                        content: Some(text),
                                                        tool_calls: None,
                                                    },
                                                    finish_reason: None,
                                                });
                                            }
                                            AnthropicDelta::InputJsonDelta { partial_json } => {
                                                yield Ok(StreamChunk {
                                                    id: current_id.clone(),
                                                    delta: StreamDelta {
                                                        content: None,
                                                        tool_calls: Some(vec![ToolCallDelta {
                                                            index,
                                                            id: None,
                                                            name: None,
                                                            arguments: Some(partial_json),
                                                        }]),
                                                    },
                                                    finish_reason: None,
                                                });
                                            }
                                        }
                                    }
                                    AnthropicStreamEvent::MessageDelta { delta, .. } => {
                                        let finish_reason = match delta.stop_reason.as_deref() {
                                            Some("end_turn") => Some(FinishReason::Stop),
                                            Some("max_tokens") => Some(FinishReason::Length),
                                            Some("stop_sequence") => Some(FinishReason::Stop),
                                            Some("tool_use") => Some(FinishReason::ToolCalls),
                                            _ => None,
                                        };
                                        if finish_reason.is_some() {
                                            yield Ok(StreamChunk {
                                                id: current_id.clone(),
                                                delta: StreamDelta {
                                                    content: None,
                                                    tool_calls: None,
                                                },
                                                finish_reason,
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                debug!("Failed to parse Anthropic stream event: {} - data: {}", e, data);
                            }
                        }
                    }
                }
            }
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// Anthropic API types

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<Vec<SystemBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    stream: bool,
}

/// Tool definition for Anthropic API
#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

/// System message block with optional cache control
#[derive(Debug, Serialize)]
struct SystemBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControlMarker>,
}

/// Cache control marker for Anthropic API
#[derive(Debug, Clone, Serialize)]
struct CacheControlMarker {
    #[serde(rename = "type")]
    cache_type: String,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicMessageContent,
}

/// Message content - either plain text or content blocks with cache control
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnthropicMessageContent {
    /// Plain text content (no cache control)
    Text(String),
    /// Content blocks with optional cache control
    Blocks(Vec<ContentBlock>),
    /// Tool result content blocks
    ToolResult(Vec<ToolResultBlock>),
    /// Assistant content blocks (text and tool_use)
    AssistantBlocks(Vec<AssistantContentBlock>),
}

/// Content block with optional cache control
#[derive(Debug, Serialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControlMarker>,
}

/// Assistant content block - can be text or tool_use
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AssistantContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Tool result block for sending tool execution results
#[derive(Debug, Serialize)]
struct ToolResultBlock {
    #[serde(rename = "type")]
    block_type: String,
    tool_use_id: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    id: String,
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContent {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: u32,
}

// Streaming types
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)] // index fields required for deserialization but not used
enum AnthropicStreamEvent {
    MessageStart {
        message: AnthropicStreamMessage,
    },
    ContentBlockStart {
        index: u32,
        content_block: AnthropicStreamContentBlock,
    },
    ContentBlockDelta {
        index: u32,
        delta: AnthropicDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: AnthropicMessageDelta,
    },
    MessageStop,
    Ping,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamMessage {
    id: String,
}

/// Content block header emitted during `content_block_start`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicStreamContentBlock {
    Text,
    ToolUse {
        id: String,
        name: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageDelta {
    stop_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::CacheControl;

    fn create_provider() -> AnthropicProvider {
        AnthropicProvider::new(ProviderConfig {
            name: "anthropic".to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
            timeout_ms: 30_000,
            max_retries: 3,
            extra: Default::default(),
        })
    }

    #[test]
    fn test_convert_messages_plain_text() {
        let provider = create_provider();
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Hello".to_string()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
                cache_control: None,
            },
        ];

        let (system, msgs) = provider.convert_messages(&messages);

        assert!(system.is_none());
        assert_eq!(msgs.len(), 1);

        // Plain text without cache_control should serialize as string
        let json = serde_json::to_string(&msgs[0]).unwrap();
        assert!(json.contains(r#""content":"Hello""#));
    }

    #[test]
    fn test_convert_messages_with_cache_control() {
        let provider = create_provider();
        let messages = vec![
            Message {
                role: Role::User,
                content: MessageContent::Text("Cached message".to_string()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
                cache_control: Some(CacheControl::ephemeral()),
            },
        ];

        let (system, msgs) = provider.convert_messages(&messages);

        assert!(system.is_none());
        assert_eq!(msgs.len(), 1);

        // With cache_control should serialize as content blocks
        let json = serde_json::to_string(&msgs[0]).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""cache_control""#));
        assert!(json.contains(r#""type":"ephemeral""#));
    }

    #[test]
    fn test_convert_system_message_with_cache_control() {
        let provider = create_provider();
        let messages = vec![
            Message {
                role: Role::System,
                content: MessageContent::Text("You are a helpful assistant.".to_string()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
                cache_control: Some(CacheControl::ephemeral()),
            },
            Message {
                role: Role::User,
                content: MessageContent::Text("Hello".to_string()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
                cache_control: None,
            },
        ];

        let (system, msgs) = provider.convert_messages(&messages);

        // System should be blocks
        assert!(system.is_some());
        let system_blocks = system.unwrap();
        assert_eq!(system_blocks.len(), 1);
        assert_eq!(system_blocks[0].block_type, "text");
        assert!(system_blocks[0].cache_control.is_some());

        // User message
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_system_block_serialization() {
        let block = SystemBlock {
            block_type: "text".to_string(),
            text: "System prompt".to_string(),
            cache_control: Some(CacheControlMarker {
                cache_type: "ephemeral".to_string(),
            }),
        };

        let json = serde_json::to_string(&block).unwrap();

        // Verify correct JSON structure for Anthropic API
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"System prompt""#));
        assert!(json.contains(r#""cache_control":{"type":"ephemeral"}"#));
    }

    #[test]
    fn test_content_block_serialization() {
        let content = AnthropicMessageContent::Blocks(vec![ContentBlock {
            block_type: "text".to_string(),
            text: "Hello".to_string(),
            cache_control: Some(CacheControlMarker {
                cache_type: "ephemeral".to_string(),
            }),
        }]);

        let json = serde_json::to_string(&content).unwrap();

        // Should be an array with type, text, and cache_control
        assert!(json.starts_with('['));
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"Hello""#));
        assert!(json.contains(r#""cache_control":{"type":"ephemeral"}"#));
    }
}
