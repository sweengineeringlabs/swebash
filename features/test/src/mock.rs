/// AI mock infrastructure for tests.
///
/// Extracted from `features/ai/tests/integration.rs` and made public for
/// reuse across all workspace crates.

use std::sync::Arc;

use llm_provider::{LlmService, MockBehaviour, MockLlmService};
use parking_lot::Mutex;
use swebash_ai::api::error::{AiError, AiResult};
use swebash_ai::api::types::{AiMessage, AiResponse, CompletionOptions};
use swebash_ai::core::agents::builtins::create_default_registry;
use swebash_ai::core::DefaultAiService;
use swebash_ai::spi::rag::EmbeddingProvider;
use swebash_ai::{AiConfig, ToolConfig};

use llmrag::RagResult;

// ── MockAiClient ─────────────────────────────────────────────────────

/// Mock `AiClient` that returns fixed "mock" responses.
///
/// Use this when you need a `DefaultAiService` without a real API key.
pub struct MockAiClient;

#[async_trait::async_trait]
impl swebash_ai::spi::AiClient for MockAiClient {
    async fn complete(
        &self,
        _messages: Vec<AiMessage>,
        _options: CompletionOptions,
    ) -> AiResult<AiResponse> {
        Ok(AiResponse {
            content: "mock".into(),
            model: "mock".into(),
        })
    }
    async fn is_ready(&self) -> bool {
        true
    }
    fn description(&self) -> String {
        "mock".into()
    }
    fn provider_name(&self) -> String {
        "mock".into()
    }
    fn model_name(&self) -> String {
        "mock".into()
    }
}

// ── ErrorMockAiClient ────────────────────────────────────────────────

/// Mock `AiClient` that always returns `AiError::Provider` from `complete()`.
///
/// Used for testing error propagation through `translate`/`explain` paths
/// that flow through `AiClient::complete()` rather than the chat engine.
pub struct ErrorMockAiClient {
    pub error_msg: String,
}

#[async_trait::async_trait]
impl swebash_ai::spi::AiClient for ErrorMockAiClient {
    async fn complete(
        &self,
        _messages: Vec<AiMessage>,
        _options: CompletionOptions,
    ) -> AiResult<AiResponse> {
        Err(AiError::Provider(self.error_msg.clone()))
    }
    async fn is_ready(&self) -> bool {
        true
    }
    fn description(&self) -> String {
        "error-mock".into()
    }
    fn provider_name(&self) -> String {
        "error-mock".into()
    }
    fn model_name(&self) -> String {
        "error-mock".into()
    }
}

// ── MockEmbedder ─────────────────────────────────────────────────────

/// Mock embedding provider for RAG integration tests.
///
/// Returns deterministic 8-dimensional vectors based on text content hash,
/// producing unique but reproducible embeddings for search testing.
pub struct MockEmbedder;

#[async_trait::async_trait]
impl EmbeddingProvider for MockEmbedder {
    async fn embed(&self, texts: &[String]) -> RagResult<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let hash = t.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
                let mut v = vec![0.0f32; 8];
                v[(hash as usize) % 8] = 1.0;
                v[((hash >> 4) as usize) % 8] += 0.5;
                v
            })
            .collect())
    }

    fn dimension(&self) -> usize {
        8
    }

    fn model_name(&self) -> &str {
        "mock-embedder"
    }
}

// ── MockRecorder ─────────────────────────────────────────────────────

/// Records calls to a mock for later inspection.
///
/// Wraps any mock to track invocation count, arguments, and ordering.
///
/// # Example
///
/// ```ignore
/// let recorder = MockRecorder::new();
/// recorder.record("complete", "user message");
/// assert_eq!(recorder.call_count(), 1);
/// assert_eq!(recorder.calls()[0], ("complete".into(), "user message".into()));
/// ```
#[derive(Debug, Clone)]
pub struct MockRecorder {
    calls: Arc<Mutex<Vec<(String, String)>>>,
}

impl MockRecorder {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record a method call with its argument summary.
    pub fn record(&self, method: &str, args: &str) {
        self.calls
            .lock()
            .push((method.to_string(), args.to_string()));
    }

    /// Number of recorded calls.
    pub fn call_count(&self) -> usize {
        self.calls.lock().len()
    }

    /// All recorded (method, args) pairs, in order.
    pub fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().clone()
    }

    /// Clear all recorded calls.
    pub fn reset(&self) {
        self.calls.lock().clear();
    }
}

impl Default for MockRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Config ───────────────────────────────────────────────────────────

/// Standard `AiConfig` for mock-backed tests.
///
/// Uses "mock" provider/model with sensible defaults. No API key required.
pub fn mock_config() -> AiConfig {
    AiConfig {
        enabled: true,
        provider: "mock".into(),
        model: "mock".into(),
        history_size: 20,
        default_agent: "shell".into(),
        agent_auto_detect: true,
        tools: ToolConfig::default(),
        log_dir: None,
        docs_base_dir: None,
        rag: swebash_ai::spi::config::RagConfig::default(),
        tool_sandbox: None,
    }
}

// ── Service Builders ─────────────────────────────────────────────────

/// Build a `DefaultAiService` backed by `MockLlmService` (no API key required).
///
/// Uses the echo mock behaviour by default: the LLM echoes back the user message.
pub fn create_mock_service() -> DefaultAiService {
    let config = mock_config();
    let llm: Arc<dyn LlmService> = Arc::new(MockLlmService::new());
    let agents = create_default_registry(llm, config.clone());
    DefaultAiService::new(Box::new(MockAiClient), agents, config)
}

/// Build a mock service with a fixed LLM response.
///
/// Every LLM call returns the given `response` string.
pub fn create_mock_service_fixed(response: &str) -> DefaultAiService {
    let config = mock_config();
    let llm: Arc<dyn LlmService> = Arc::new(
        MockLlmService::new().with_behaviour(MockBehaviour::Fixed(response.to_string())),
    );
    let agents = create_default_registry(llm, config.clone());
    DefaultAiService::new(Box::new(MockAiClient), agents, config)
}

/// Build a mock service where every LLM call returns an error.
///
/// `chat`/`chat_streaming` flow through the LLM path and will fail.
/// `translate`/`explain` flow through `MockAiClient::complete()` and will succeed.
pub fn create_mock_service_error(error_msg: &str) -> DefaultAiService {
    let config = mock_config();
    let llm: Arc<dyn LlmService> = Arc::new(
        MockLlmService::new().with_behaviour(MockBehaviour::Error(error_msg.to_string())),
    );
    let agents = create_default_registry(llm, config.clone());
    DefaultAiService::new(Box::new(MockAiClient), agents, config)
}

/// Build a mock service where both the LLM and `AiClient` return errors.
///
/// `chat`/`chat_streaming` flow through the LLM → `ChatEngine` path,
/// while `translate`/`explain` flow through the `AiClient` → `complete()` path.
/// This ensures both paths fail with the given error.
pub fn create_mock_service_full_error(error_msg: &str) -> DefaultAiService {
    let config = mock_config();
    let llm: Arc<dyn LlmService> = Arc::new(
        MockLlmService::new().with_behaviour(MockBehaviour::Error(error_msg.to_string())),
    );
    let agents = create_default_registry(llm, config.clone());
    DefaultAiService::new(
        Box::new(ErrorMockAiClient {
            error_msg: error_msg.to_string(),
        }),
        agents,
        config,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use swebash_ai::api::AiService;
    use swebash_ai::spi::AiClient;

    #[test]
    fn mock_config_has_sensible_defaults() {
        let config = mock_config();
        assert!(config.enabled);
        assert_eq!(config.provider, "mock");
        assert_eq!(config.model, "mock");
        assert_eq!(config.history_size, 20);
        assert_eq!(config.default_agent, "shell");
        assert!(config.agent_auto_detect);
    }

    #[test]
    fn mock_recorder_tracks_calls() {
        let recorder = MockRecorder::new();
        assert_eq!(recorder.call_count(), 0);

        recorder.record("complete", "hello");
        recorder.record("complete", "world");

        assert_eq!(recorder.call_count(), 2);
        let calls = recorder.calls();
        assert_eq!(calls[0], ("complete".into(), "hello".into()));
        assert_eq!(calls[1], ("complete".into(), "world".into()));
    }

    #[test]
    fn mock_recorder_reset_clears_calls() {
        let recorder = MockRecorder::new();
        recorder.record("foo", "bar");
        assert_eq!(recorder.call_count(), 1);
        recorder.reset();
        assert_eq!(recorder.call_count(), 0);
    }

    #[tokio::test]
    async fn mock_ai_client_returns_mock_response() {
        let client = MockAiClient;
        let resp = client
            .complete(vec![], CompletionOptions::default())
            .await
            .unwrap();
        assert_eq!(resp.content, "mock");
        assert_eq!(resp.model, "mock");
    }

    #[tokio::test]
    async fn error_mock_ai_client_returns_error() {
        let client = ErrorMockAiClient {
            error_msg: "test failure".into(),
        };
        let result = client.complete(vec![], CompletionOptions::default()).await;
        match result {
            Err(AiError::Provider(msg)) => assert_eq!(msg, "test failure"),
            other => panic!("Expected Provider error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn mock_embedder_returns_deterministic_vectors() {
        let embedder = MockEmbedder;
        let v1 = embedder
            .embed(&["hello".into()])
            .await
            .unwrap();
        let v2 = embedder
            .embed(&["hello".into()])
            .await
            .unwrap();
        assert_eq!(v1, v2, "Same input must produce same embedding");
        assert_eq!(v1[0].len(), 8);
    }

    #[tokio::test]
    async fn mock_embedder_dimension_matches() {
        let embedder = MockEmbedder;
        let vectors = embedder
            .embed(&["test".into()])
            .await
            .unwrap();
        assert_eq!(vectors[0].len(), embedder.dimension());
    }

    #[test]
    fn create_mock_service_builds_without_api_key() {
        let _service = create_mock_service();
    }

    #[test]
    fn create_mock_service_fixed_builds_without_api_key() {
        let _service = create_mock_service_fixed("Hello");
    }

    #[test]
    fn create_mock_service_error_builds_without_api_key() {
        let _service = create_mock_service_error("some error");
    }

    #[test]
    fn create_mock_service_full_error_builds_without_api_key() {
        let _service = create_mock_service_full_error("some error");
    }

    #[tokio::test]
    async fn mock_service_is_available() {
        let service = create_mock_service();
        assert!(service.is_available().await);
    }
}
