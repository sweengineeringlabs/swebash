//! Mock LLM service for testing
//!
//! `MockLlmService` implements `LlmService` without making any API calls.
//! Tests that only need metadata operations (registry, descriptor, cache)
//! can use this to satisfy type constraints without network dependencies.

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use futures::stream;

use crate::api::{
    CompletionRequest, CompletionResponse, CompletionStream, FinishReason, LlmError, LlmResult,
    LlmService, ModelInfo, TokenUsage,
};

/// Behaviour when `complete()` or `complete_stream()` is called
#[derive(Debug, Clone)]
pub enum MockBehaviour {
    /// Return a canned text response (default)
    Echo,
    /// Return a fixed response string
    Fixed(String),
    /// Always fail with this error message
    Error(String),
}

impl Default for MockBehaviour {
    fn default() -> Self {
        Self::Echo
    }
}

/// Mock implementation of [`LlmService`]
///
/// Never contacts an LLM provider. Configurable response behaviour
/// and call counters for test assertions.
///
/// # Example
///
/// ```rust,ignore
/// use llm_provider::testing::MockLlmService;
///
/// let mock = MockLlmService::new();
/// // Use Arc::new(mock) wherever Arc<dyn LlmService> is needed
/// ```
pub struct MockLlmService {
    behaviour: MockBehaviour,
    model: String,
    provider_name: String,
    complete_calls: AtomicU64,
    stream_calls: AtomicU64,
}

impl MockLlmService {
    /// Create a mock that echoes user input back
    pub fn new() -> Self {
        Self {
            behaviour: MockBehaviour::Echo,
            model: "mock-model".to_owned(),
            provider_name: "mock".to_owned(),
            complete_calls: AtomicU64::new(0),
            stream_calls: AtomicU64::new(0),
        }
    }

    /// Set the response behaviour
    pub fn with_behaviour(mut self, behaviour: MockBehaviour) -> Self {
        self.behaviour = behaviour;
        self
    }

    /// Set the model name reported by `model_info()` and `list_models()`
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_owned();
        self
    }

    /// Set the provider name reported by `providers()`
    pub fn with_provider_name(mut self, name: &str) -> Self {
        self.provider_name = name.to_owned();
        self
    }

    /// Number of times `complete()` was called
    pub fn complete_calls(&self) -> u64 {
        self.complete_calls.load(Ordering::Relaxed)
    }

    /// Number of times `complete_stream()` was called
    pub fn stream_calls(&self) -> u64 {
        self.stream_calls.load(Ordering::Relaxed)
    }

    fn make_response(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse> {
        match &self.behaviour {
            MockBehaviour::Echo => {
                let echo = request
                    .messages
                    .last()
                    .map(|m| match &m.content {
                        crate::MessageContent::Text(t) => t.clone(),
                        crate::MessageContent::Parts(_) => "[multipart]".to_owned(),
                    })
                    .unwrap_or_default();

                Ok(CompletionResponse {
                    id: format!("mock-{}", self.complete_calls.load(Ordering::Relaxed)),
                    model: self.model.clone(),
                    content: Some(echo),
                    tool_calls: vec![],
                    finish_reason: FinishReason::Stop,
                    usage: TokenUsage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                        cache_read_input_tokens: 0,
                        cache_creation_input_tokens: 0,
                    },
                })
            }
            MockBehaviour::Fixed(text) => Ok(CompletionResponse {
                id: format!("mock-{}", self.complete_calls.load(Ordering::Relaxed)),
                model: self.model.clone(),
                content: Some(text.clone()),
                tool_calls: vec![],
                finish_reason: FinishReason::Stop,
                usage: TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                },
            }),
            MockBehaviour::Error(msg) => Err(LlmError::ProviderError {
                provider: self.provider_name.clone(),
                message: msg.clone(),
            }),
        }
    }

    fn model_info_value(&self) -> ModelInfo {
        ModelInfo {
            id: self.model.clone(),
            name: self.model.clone(),
            provider: self.provider_name.clone(),
            context_window: 128_000,
            supports_vision: false,
            supports_function_calling: true,
            supports_streaming: true,
        }
    }
}

impl Default for MockLlmService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmService for MockLlmService {
    async fn complete(&self, request: CompletionRequest) -> LlmResult<CompletionResponse> {
        self.complete_calls.fetch_add(1, Ordering::Relaxed);
        self.make_response(&request)
    }

    async fn complete_stream(&self, request: CompletionRequest) -> LlmResult<CompletionStream> {
        self.stream_calls.fetch_add(1, Ordering::Relaxed);
        let response = self.make_response(&request)?;

        let chunk = crate::StreamChunk {
            id: response.id,
            delta: crate::StreamDelta {
                content: response.content,
                tool_calls: None,
            },
            finish_reason: Some(response.finish_reason),
        };

        Ok(Box::pin(stream::once(async move { Ok(chunk) })))
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        Ok(vec![self.model_info_value()])
    }

    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo> {
        if model == self.model {
            Ok(self.model_info_value())
        } else {
            Err(LlmError::ModelNotFound(model.to_owned()))
        }
    }

    async fn is_model_available(&self, model: &str) -> bool {
        model == self.model
    }

    async fn providers(&self) -> Vec<String> {
        vec![self.provider_name.clone()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Message, MessageContent, Role};

    fn simple_request() -> CompletionRequest {
        CompletionRequest {
            model: "mock-model".to_owned(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Text("hello".to_owned()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
                cache_control: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            tools: None,
            tool_choice: None,
        }
    }

    #[tokio::test]
    async fn echo_behaviour() {
        let mock = MockLlmService::new();
        let resp = mock.complete(simple_request()).await.unwrap();
        assert_eq!(resp.content.as_deref(), Some("hello"));
        assert_eq!(resp.finish_reason, FinishReason::Stop);
    }

    #[tokio::test]
    async fn fixed_behaviour() {
        let mock = MockLlmService::new()
            .with_behaviour(MockBehaviour::Fixed("fixed response".into()));
        let resp = mock.complete(simple_request()).await.unwrap();
        assert_eq!(resp.content.as_deref(), Some("fixed response"));
    }

    #[tokio::test]
    async fn error_behaviour() {
        let mock = MockLlmService::new()
            .with_behaviour(MockBehaviour::Error("test failure".into()));
        let err = mock.complete(simple_request()).await.unwrap_err();
        assert!(err.to_string().contains("test failure"));
    }

    #[tokio::test]
    async fn call_counters() {
        let mock = MockLlmService::new();
        assert_eq!(mock.complete_calls(), 0);
        assert_eq!(mock.stream_calls(), 0);

        mock.complete(simple_request()).await.unwrap();
        mock.complete(simple_request()).await.unwrap();
        assert_eq!(mock.complete_calls(), 2);

        let _stream = mock.complete_stream(simple_request()).await.unwrap();
        assert_eq!(mock.stream_calls(), 1);
    }

    #[tokio::test]
    async fn model_info_methods() {
        let mock = MockLlmService::new().with_model("test-model");
        let models = mock.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "test-model");

        let info = mock.model_info("test-model").await.unwrap();
        assert_eq!(info.id, "test-model");

        assert!(mock.is_model_available("test-model").await);
        assert!(!mock.is_model_available("other").await);

        assert!(mock.model_info("other").await.is_err());
    }

    #[tokio::test]
    async fn providers_method() {
        let mock = MockLlmService::new().with_provider_name("test-provider");
        let providers = mock.providers().await;
        assert_eq!(providers, vec!["test-provider"]);
    }

    #[test]
    fn mock_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockLlmService>();
    }
}
