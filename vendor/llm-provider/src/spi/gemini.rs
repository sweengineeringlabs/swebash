//! Google Gemini provider implementation

use async_trait::async_trait;
use futures::stream::BoxStream;
use crate::api::{
    CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmResult, Message,
    MessageContent, ModelInfo, Role, StreamChunk, StreamDelta, TokenUsage,
};
use crate::config::{keys, ProviderConfig};
use super::AsyncLlmProvider;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::time::Duration;
use tracing::{debug, warn};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Google Gemini provider implementation
#[derive(Debug)]
pub struct GeminiProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl GeminiProvider {
    /// Create a new Gemini provider from environment
    ///
    /// Reads GOOGLE_API_KEY or GEMINI_API_KEY from environment
    pub fn from_env() -> LlmResult<Self> {
        let api_key = std::env::var(keys::GOOGLE_API_KEY)
            .or_else(|_| std::env::var(keys::GEMINI_API_KEY))
            .map_err(|_| {
                LlmError::AuthenticationFailed(format!(
                    "{} or {} not set",
                    keys::GOOGLE_API_KEY,
                    keys::GEMINI_API_KEY
                ))
            })?;

        let base_url = std::env::var(keys::GEMINI_BASE_URL)
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        Ok(Self::new(ProviderConfig {
            name: "gemini".to_string(),
            api_key: Some(api_key),
            base_url: Some(base_url),
            timeout_ms: 60_000,
            max_retries: 3,
            extra: Default::default(),
        }))
    }

    /// Create a new Gemini provider with explicit configuration
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

    /// Convert our Message format to Gemini format
    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<GeminiContent>) {
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    if let MessageContent::Text(text) = &msg.content {
                        system_instruction = Some(text.clone());
                    }
                }
                Role::User | Role::Assistant => {
                    let parts = match &msg.content {
                        MessageContent::Text(text) => {
                            vec![GeminiPart::Text { text: text.clone() }]
                        }
                        MessageContent::Parts(_) => {
                            // For now, flatten to text. TODO: Support multimodal content
                            warn!("Multimodal content not yet fully supported");
                            vec![GeminiPart::Text {
                                text: String::new(),
                            }]
                        }
                    };

                    contents.push(GeminiContent {
                        role: match msg.role {
                            Role::User => "user".to_string(),
                            Role::Assistant => "model".to_string(), // Gemini uses "model" for assistant
                            _ => "user".to_string(),
                        },
                        parts,
                    });
                }
                Role::Tool => {
                    warn!("Tool messages not yet supported for Gemini");
                }
            }
        }

        (system_instruction, contents)
    }

    /// Convert Gemini response to our format
    fn convert_response(&self, model: &str, response: GeminiResponse) -> CompletionResponse {
        let candidate = response.candidates.first();
        let content = candidate
            .and_then(|c| c.content.parts.first())
            .and_then(|part| match part {
                GeminiPart::Text { text } => Some(text.clone()),
            });

        let finish_reason = candidate
            .and_then(|c| c.finish_reason.as_deref())
            .and_then(|fr| match fr {
                "STOP" => Some(FinishReason::Stop),
                "MAX_TOKENS" => Some(FinishReason::Length),
                "SAFETY" => Some(FinishReason::ContentFilter),
                _ => None,
            })
            .unwrap_or(FinishReason::Error);

        // Gemini doesn't always provide token usage in the same way
        let usage = response.usage_metadata.map(|u| TokenUsage {
            prompt_tokens: u.prompt_token_count.unwrap_or(0),
            completion_tokens: u.candidates_token_count.unwrap_or(0),
            total_tokens: u.total_token_count.unwrap_or(0),
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        }).unwrap_or(TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });

        CompletionResponse {
            id: "gemini-response".to_string(), // Gemini doesn't provide IDs
            model: model.to_string(),
            content,
            tool_calls: Vec::new(), // TODO: Support tool calls
            finish_reason,
            usage,
        }
    }

    /// Map HTTP status to LlmError
    fn map_error(&self, status: reqwest::StatusCode, body: &str) -> LlmError {
        match status.as_u16() {
            401 | 403 => LlmError::AuthenticationFailed(body.to_string()),
            429 => LlmError::RateLimited {
                retry_after_ms: None,
            },
            400 => LlmError::InvalidRequest(body.to_string()),
            500..=599 => LlmError::ProviderError {
                provider: "gemini".to_string(),
                message: body.to_string(),
            },
            _ => LlmError::NetworkError(format!("HTTP {}: {}", status, body)),
        }
    }
}

#[async_trait]
impl AsyncLlmProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn models(&self) -> &[&str] {
        &[
            "gemini-2.0-flash-exp",
            "gemini-1.5-pro",
            "gemini-1.5-flash",
        ]
    }

    fn model_info(&self, model: &str) -> Option<ModelInfo> {
        let (context_window, supports_vision) = match model {
            "gemini-2.0-flash-exp" => (1_000_000, true),
            "gemini-1.5-pro" => (2_000_000, true),
            "gemini-1.5-flash" => (1_000_000, true),
            _ => return None,
        };

        Some(ModelInfo {
            id: model.to_string(),
            name: model.to_string(),
            provider: "gemini".to_string(),
            context_window,
            supports_vision,
            supports_function_calling: true,
            supports_streaming: true,
        })
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn complete(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse> {
        debug!("Gemini complete: model={}", request.model);

        let (system_instruction, contents) = self.convert_messages(&request.messages);

        let generation_config = GeminiGenerationConfig {
            temperature: request.temperature,
            max_output_tokens: request.max_tokens,
            top_p: request.top_p,
            stop_sequences: request.stop.clone(),
        };

        let gemini_request = GeminiRequest {
            contents,
            system_instruction: system_instruction.map(|text| GeminiSystemInstruction {
                parts: vec![GeminiPart::Text { text }],
            }),
            generation_config: Some(generation_config),
        };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, request.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&gemini_request)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_error(status, &body));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .map_err(|e| LlmError::SerializationError(e.to_string()))?;

        Ok(self.convert_response(&request.model, gemini_response))
    }

    fn complete_stream(&self, request: &CompletionRequest) -> BoxStream<'static, LlmResult<StreamChunk>> {
        debug!("Gemini complete_stream: model={}", request.model);

        let (system_instruction, contents) = self.convert_messages(&request.messages);

        let generation_config = GeminiGenerationConfig {
            temperature: request.temperature,
            max_output_tokens: request.max_tokens,
            top_p: request.top_p,
            stop_sequences: request.stop.clone(),
        };

        let gemini_request = GeminiRequest {
            contents,
            system_instruction: system_instruction.map(|text| GeminiSystemInstruction {
                parts: vec![GeminiPart::Text { text }],
            }),
            generation_config: Some(generation_config),
        };

        let url = format!(
            "{}/models/{}:streamGenerateContent?key={}",
            self.base_url, request.model, self.api_key
        );
        let client = self.client.clone();

        Box::pin(async_stream::stream! {
            let response = client
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .json(&gemini_request)
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
                    provider: "gemini".to_string(),
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

                // Gemini returns newline-delimited JSON objects
                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() || line == "," {
                        continue;
                    }

                    // Skip array delimiters
                    if line == "[" || line == "]" {
                        continue;
                    }

                    // Remove trailing comma if present
                    let line = line.trim_end_matches(',');

                    // Parse the JSON chunk
                    match serde_json::from_str::<GeminiResponse>(line) {
                        Ok(chunk) => {
                            if let Some(candidate) = chunk.candidates.first() {
                                let content = candidate.content.parts.first().and_then(|part| match part {
                                    GeminiPart::Text { text } => Some(text.clone()),
                                });

                                let finish_reason = candidate.finish_reason.as_deref().and_then(|fr| match fr {
                                    "STOP" => Some(FinishReason::Stop),
                                    "MAX_TOKENS" => Some(FinishReason::Length),
                                    "SAFETY" => Some(FinishReason::ContentFilter),
                                    _ => None,
                                });

                                yield Ok(StreamChunk {
                                    id: "gemini-stream".to_string(),
                                    delta: StreamDelta {
                                        content,
                                        tool_calls: None,
                                    },
                                    finish_reason,
                                });
                            }
                        }
                        Err(e) => {
                            debug!("Failed to parse Gemini stream chunk: {} - data: {}", e, line);
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

// Gemini API types

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
}

#[derive(Debug, Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
    #[serde(rename = "totalTokenCount")]
    total_token_count: Option<u32>,
}
