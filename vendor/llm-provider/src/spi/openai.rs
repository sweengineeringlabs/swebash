//! OpenAI provider implementation

use async_trait::async_trait;
use futures::stream::BoxStream;
use crate::api::{
    CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmResult, Message,
    MessageContent, ModelInfo, Role, StreamChunk, StreamDelta, TokenUsage, ToolCall,
};
use crate::config::{keys, ProviderConfig};
use super::AsyncLlmProvider;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::time::Duration;
use tracing::{debug, warn};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI provider implementation
#[derive(Debug)]
pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider from environment
    ///
    /// Reads OPENAI_API_KEY from environment
    pub fn from_env() -> LlmResult<Self> {
        let api_key = std::env::var(keys::OPENAI_API_KEY).map_err(|_| {
            LlmError::AuthenticationFailed(format!("{} not set", keys::OPENAI_API_KEY))
        })?;

        let base_url = std::env::var(keys::OPENAI_BASE_URL)
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        Ok(Self::new(ProviderConfig {
            name: "openai".to_string(),
            api_key: Some(api_key),
            base_url: Some(base_url),
            timeout_ms: 60_000,
            max_retries: 3,
            extra: Default::default(),
        }))
    }

    /// Create a new OpenAI provider with explicit configuration
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
            api_key,
            base_url,
        }
    }

    /// Convert our Message format to OpenAI format
    fn convert_messages(&self, messages: &[Message]) -> Vec<OpenAiMessage> {
        messages
            .iter()
            .map(|msg| OpenAiMessage {
                role: match msg.role {
                    Role::System => "system".to_string(),
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::Tool => "tool".to_string(),
                },
                content: match &msg.content {
                    MessageContent::Text(text) => Some(text.clone()),
                    MessageContent::Parts(_) => {
                        // For now, flatten to text. TODO: Support multimodal content
                        warn!("Multimodal content not yet fully supported, using text only");
                        None
                    }
                },
                name: msg.name.clone(),
                tool_call_id: msg.tool_call_id.clone(),
            })
            .collect()
    }

    /// Convert OpenAI response to our format
    fn convert_response(&self, model: &str, response: OpenAiResponse) -> CompletionResponse {
        let choice = response.choices.first();
        let content = choice.and_then(|c| c.message.content.clone());
        let tool_calls = choice
            .map(|c| {
                c.message
                    .tool_calls
                    .as_ref()
                    .map(|calls| {
                        calls
                            .iter()
                            .map(|tc| ToolCall {
                                id: tc.id.clone(),
                                name: tc.function.name.clone(),
                                arguments: tc.function.arguments.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let finish_reason = choice
            .and_then(|c| c.finish_reason.as_deref())
            .and_then(|fr| match fr {
                "stop" => Some(FinishReason::Stop),
                "length" => Some(FinishReason::Length),
                "tool_calls" => Some(FinishReason::ToolCalls),
                "content_filter" => Some(FinishReason::ContentFilter),
                _ => None,
            })
            .unwrap_or(FinishReason::Error);

        CompletionResponse {
            id: response.id,
            model: model.to_string(),
            content,
            tool_calls,
            finish_reason,
            usage: TokenUsage {
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                total_tokens: response.usage.total_tokens,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        }
    }

    /// Map HTTP status to LlmError
    fn map_error(&self, status: reqwest::StatusCode, body: &str) -> LlmError {
        match status.as_u16() {
            401 => LlmError::AuthenticationFailed(body.to_string()),
            429 => LlmError::RateLimited {
                retry_after_ms: None,
            },
            400 => LlmError::InvalidRequest(body.to_string()),
            500..=599 => LlmError::ProviderError {
                provider: "openai".to_string(),
                message: body.to_string(),
            },
            _ => LlmError::NetworkError(format!("HTTP {}: {}", status, body)),
        }
    }
}

#[async_trait]
impl AsyncLlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn models(&self) -> &[&str] {
        &[
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4-turbo",
            "gpt-4",
            "gpt-3.5-turbo",
            "o1-preview",
            "o1-mini",
        ]
    }

    fn model_info(&self, model: &str) -> Option<ModelInfo> {
        let (context_window, supports_vision) = match model {
            "gpt-4o" => (128_000, true),
            "gpt-4o-mini" => (128_000, true),
            "gpt-4-turbo" => (128_000, true),
            "gpt-4" => (8_192, false),
            "gpt-3.5-turbo" => (16_385, false),
            "o1-preview" => (128_000, false),
            "o1-mini" => (128_000, false),
            _ => return None,
        };

        Some(ModelInfo {
            id: model.to_string(),
            name: model.to_string(),
            provider: "openai".to_string(),
            context_window,
            supports_vision,
            supports_function_calling: !model.starts_with("o1"), // o1 models don't support function calling
            supports_streaming: true,
        })
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn complete(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse> {
        debug!("OpenAI complete: model={}", request.model);

        let openai_request = OpenAiRequest {
            model: request.model.clone(),
            messages: self.convert_messages(&request.messages),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            top_p: request.top_p,
            stop: request.stop.clone(),
            stream: false,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body));
        }

        let openai_response: OpenAiResponse = response
            .json()
            .await
            .map_err(|e| LlmError::SerializationError(e.to_string()))?;

        Ok(self.convert_response(&request.model, openai_response))
    }

    fn complete_stream(&self, request: &CompletionRequest) -> BoxStream<'static, LlmResult<StreamChunk>> {
        debug!("OpenAI complete_stream: model={}", request.model);

        let openai_request = OpenAiRequest {
            model: request.model.clone(),
            messages: self.convert_messages(&request.messages),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            top_p: request.top_p,
            stop: request.stop.clone(),
            stream: true,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let client = self.client.clone();
        let api_key = self.api_key.clone();

        Box::pin(async_stream::stream! {
            let response = client
                .post(&url)
                .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
                .header(header::CONTENT_TYPE, "application/json")
                .json(&openai_request)
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
                    provider: "openai".to_string(),
                    message: format!("HTTP {}: {}", status, body),
                });
                return;
            }

            // Stream the response
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();

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

                    // OpenAI SSE format: "data: {...}" or "data: [DONE]"
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            return;
                        }

                        // Parse the JSON chunk
                        match serde_json::from_str::<OpenAiStreamChunk>(data) {
                            Ok(chunk) => {
                                if let Some(choice) = chunk.choices.first() {
                                    let content = choice.delta.content.clone();
                                    let finish_reason = choice.finish_reason.as_deref().and_then(|fr| match fr {
                                        "stop" => Some(FinishReason::Stop),
                                        "length" => Some(FinishReason::Length),
                                        "tool_calls" => Some(FinishReason::ToolCalls),
                                        "content_filter" => Some(FinishReason::ContentFilter),
                                        _ => None,
                                    });

                                    yield Ok(StreamChunk {
                                        id: chunk.id,
                                        delta: StreamDelta {
                                            content,
                                            tool_calls: None,
                                        },
                                        finish_reason,
                                    });
                                }
                            }
                            Err(e) => {
                                debug!("Failed to parse OpenAI stream chunk: {} - data: {}", e, data);
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

// OpenAI API types

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    id: String,
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    _type: String,
    function: OpenAiFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// Streaming types
#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    id: String,
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
}
