/// L2 SPI implementation: direct HTTP client for LLM providers.
///
/// This is the ONLY file in swebash-ai that makes HTTP calls to LLM APIs.
/// All other modules use the `AiClient` trait.
///
/// When `llm-provider` is published to a registry, this file can be
/// replaced with a thin wrapper around it.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::api::error::{AiError, AiResult};
use crate::api::types::{AiMessage, AiResponse, AiRole, CompletionOptions};
use crate::config::AiConfig;
use crate::spi::AiClient;

/// Direct HTTP client for LLM provider APIs.
pub struct LlmProviderClient {
    http: reqwest::Client,
    provider: String,
    model: String,
    api_key: String,
    base_url: String,
}

impl LlmProviderClient {
    /// Create a new client from configuration.
    pub fn new(config: &AiConfig) -> AiResult<Self> {
        let (api_key, key_env) = resolve_api_key(&config.provider)?;
        let base_url = resolve_base_url(&config.provider);

        tracing::info!(
            provider = %config.provider,
            model = %config.model,
            key_env = %key_env,
            "LLM provider client initialized"
        );

        Ok(Self {
            http: reqwest::Client::new(),
            provider: config.provider.clone(),
            model: config.model.clone(),
            api_key,
            base_url,
        })
    }
}

/// Resolve the API key for the given provider.
fn resolve_api_key(provider: &str) -> AiResult<(String, &'static str)> {
    let key_env = match provider {
        "openai" => "OPENAI_API_KEY",
        "anthropic" => "ANTHROPIC_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        other => {
            return Err(AiError::NotConfigured(format!(
                "Unknown provider '{}'. Supported: openai, anthropic, gemini",
                other
            )))
        }
    };
    let key = std::env::var(key_env).map_err(|_| {
        AiError::NotConfigured(format!(
            "API key not set. Export {}=<your-key>",
            key_env
        ))
    })?;
    Ok((key, key_env))
}

/// Resolve the base URL for the given provider.
fn resolve_base_url(provider: &str) -> String {
    match provider {
        "openai" => std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
        "anthropic" => std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string()),
        "gemini" => std::env::var("GEMINI_BASE_URL")
            .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1beta".to_string()),
        _ => String::new(),
    }
}

// ── OpenAI-compatible request/response types ──

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    model: String,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessageContent,
}

#[derive(Deserialize)]
struct OpenAiMessageContent {
    content: Option<String>,
}

// ── Anthropic request/response types ──

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    model: String,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: Option<String>,
}

// ── Gemini request/response types ──

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
}

#[derive(Serialize)]
struct GeminiContent {
    role: Option<String>,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiCandidateContent,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

// ── Role conversion helpers ──

fn role_str(role: AiRole) -> &'static str {
    match role {
        AiRole::System => "system",
        AiRole::User => "user",
        AiRole::Assistant => "assistant",
    }
}

// ── Provider-specific completion implementations ──

async fn complete_openai(
    client: &LlmProviderClient,
    messages: Vec<AiMessage>,
    options: CompletionOptions,
) -> AiResult<AiResponse> {
    let oai_messages: Vec<OpenAiMessage> = messages
        .into_iter()
        .map(|m| OpenAiMessage {
            role: role_str(m.role).to_string(),
            content: m.content,
        })
        .collect();

    let body = OpenAiRequest {
        model: client.model.clone(),
        messages: oai_messages,
        temperature: options.temperature,
        max_tokens: options.max_tokens,
    };

    let resp = client
        .http
        .post(format!("{}/chat/completions", client.base_url))
        .header("Authorization", format!("Bearer {}", client.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AiError::Provider(format!("HTTP error: {}", e)))?;

    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AiError::RateLimited);
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(AiError::Provider(format!("OpenAI API error ({}): {}", status, text)));
    }

    let oai_resp: OpenAiResponse = resp
        .json()
        .await
        .map_err(|e| AiError::ParseError(format!("Failed to parse OpenAI response: {}", e)))?;

    let content = oai_resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    Ok(AiResponse {
        content,
        model: oai_resp.model,
    })
}

async fn complete_anthropic(
    client: &LlmProviderClient,
    messages: Vec<AiMessage>,
    options: CompletionOptions,
) -> AiResult<AiResponse> {
    // Anthropic handles system messages separately
    let mut system_text: Option<String> = None;
    let mut api_messages: Vec<AnthropicMessage> = Vec::new();

    for msg in messages {
        if msg.role == AiRole::System {
            system_text = Some(msg.content);
        } else {
            api_messages.push(AnthropicMessage {
                role: role_str(msg.role).to_string(),
                content: msg.content,
            });
        }
    }

    let body = AnthropicRequest {
        model: client.model.clone(),
        messages: api_messages,
        max_tokens: options.max_tokens.unwrap_or(1024),
        temperature: options.temperature,
        system: system_text,
    };

    let resp = client
        .http
        .post(format!("{}/messages", client.base_url))
        .header("x-api-key", &client.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AiError::Provider(format!("HTTP error: {}", e)))?;

    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AiError::RateLimited);
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(AiError::Provider(format!("Anthropic API error ({}): {}", status, text)));
    }

    let anth_resp: AnthropicResponse = resp
        .json()
        .await
        .map_err(|e| AiError::ParseError(format!("Failed to parse Anthropic response: {}", e)))?;

    let content = anth_resp
        .content
        .first()
        .and_then(|c| c.text.clone())
        .unwrap_or_default();

    Ok(AiResponse {
        content,
        model: anth_resp.model,
    })
}

async fn complete_gemini(
    client: &LlmProviderClient,
    messages: Vec<AiMessage>,
    options: CompletionOptions,
) -> AiResult<AiResponse> {
    // Gemini handles system instruction separately
    let mut system_instruction: Option<GeminiContent> = None;
    let mut contents: Vec<GeminiContent> = Vec::new();

    for msg in messages {
        if msg.role == AiRole::System {
            system_instruction = Some(GeminiContent {
                role: None,
                parts: vec![GeminiPart { text: msg.content }],
            });
        } else {
            let role = match msg.role {
                AiRole::User => "user",
                AiRole::Assistant => "model",
                AiRole::System => unreachable!(),
            };
            contents.push(GeminiContent {
                role: Some(role.to_string()),
                parts: vec![GeminiPart { text: msg.content }],
            });
        }
    }

    let body = GeminiRequest {
        contents,
        generation_config: Some(GeminiGenerationConfig {
            temperature: options.temperature,
            max_output_tokens: options.max_tokens,
        }),
        system_instruction,
    };

    let url = format!(
        "{}/models/{}:generateContent?key={}",
        client.base_url, client.model, client.api_key
    );

    let resp = client
        .http
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AiError::Provider(format!("HTTP error: {}", e)))?;

    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AiError::RateLimited);
    }
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(AiError::Provider(format!("Gemini API error ({}): {}", status, text)));
    }

    let gem_resp: GeminiResponse = resp
        .json()
        .await
        .map_err(|e| AiError::ParseError(format!("Failed to parse Gemini response: {}", e)))?;

    let content = gem_resp
        .candidates
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.content.parts)
        .and_then(|p| p.into_iter().next())
        .and_then(|p| p.text)
        .unwrap_or_default();

    Ok(AiResponse {
        content,
        model: client.model.clone(),
    })
}

#[async_trait]
impl AiClient for LlmProviderClient {
    async fn complete(
        &self,
        messages: Vec<AiMessage>,
        options: CompletionOptions,
    ) -> AiResult<AiResponse> {
        match self.provider.as_str() {
            "openai" => complete_openai(self, messages, options).await,
            "anthropic" => complete_anthropic(self, messages, options).await,
            "gemini" => complete_gemini(self, messages, options).await,
            _ => Err(AiError::NotConfigured(format!(
                "Unknown provider: {}",
                self.provider
            ))),
        }
    }

    fn is_ready(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn description(&self) -> String {
        format!("{}:{}", self.provider, self.model)
    }

    fn provider_name(&self) -> String {
        self.provider.clone()
    }

    fn model_name(&self) -> String {
        self.model.clone()
    }
}
