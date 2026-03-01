/// AI mock infrastructure for tests.
///
/// Provides mock implementations of `AiService` and related types for testing
/// without requiring real API keys or network access.

use std::sync::Arc;

use parking_lot::Mutex;
use swebash_llm::api::error::{AiError, AiResult};
use swebash_llm::api::types::{
    AiEvent, AiMessage, AiResponse, AgentInfo, AiStatus, AutocompleteRequest, AutocompleteResponse,
    ChatRequest, ChatResponse, CompletionOptions, ExplainRequest, ExplainResponse,
    TranslateRequest, TranslateResponse,
};
use swebash_llm::api::AiService;
use swebash_llm::spi::rag::EmbeddingProvider;
use swebash_llm::{AiConfig, ToolConfig};

use llmrag::RagResult;

// ── MockAiClient ─────────────────────────────────────────────────────

/// Mock `AiClient` that returns fixed "mock" responses.
///
/// Use this when you need an `AiClient` without a real API key.
pub struct MockAiClient;

#[async_trait::async_trait]
impl swebash_llm::spi::AiClient for MockAiClient {
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
impl swebash_llm::spi::AiClient for ErrorMockAiClient {
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

// ── MockAiService ────────────────────────────────────────────────────

/// Mock implementation of `AiService` for testing.
///
/// Returns configurable responses without requiring a gateway or API key.
pub struct MockAiService {
    config: AiConfig,
    behaviour: MockServiceBehaviour,
    current_agent: Mutex<String>,
}

/// Behaviour configuration for MockAiService.
#[derive(Clone)]
pub enum MockServiceBehaviour {
    /// Return a fixed response for all operations.
    Fixed(String),
    /// Return an error for all operations.
    Error(String),
    /// Echo back the input.
    Echo,
}

impl Default for MockServiceBehaviour {
    fn default() -> Self {
        Self::Echo
    }
}

impl MockAiService {
    /// Create a new mock service with echo behaviour.
    pub fn new(config: AiConfig) -> Self {
        Self {
            config,
            behaviour: MockServiceBehaviour::Echo,
            current_agent: Mutex::new("shell".to_string()),
        }
    }

    /// Set the response behaviour.
    pub fn with_behaviour(mut self, behaviour: MockServiceBehaviour) -> Self {
        self.behaviour = behaviour;
        self
    }

    fn response(&self, input: &str) -> AiResult<String> {
        match &self.behaviour {
            MockServiceBehaviour::Fixed(s) => Ok(s.clone()),
            MockServiceBehaviour::Error(e) => Err(AiError::Provider(e.clone())),
            MockServiceBehaviour::Echo => Ok(input.to_string()),
        }
    }
}

#[async_trait::async_trait]
impl AiService for MockAiService {
    async fn translate(&self, request: TranslateRequest) -> AiResult<TranslateResponse> {
        let command = self.response(&request.natural_language)?;
        Ok(TranslateResponse {
            command,
            explanation: "Mock explanation".to_string(),
        })
    }

    async fn explain(&self, request: ExplainRequest) -> AiResult<ExplainResponse> {
        let explanation = self.response(&request.command)?;
        Ok(ExplainResponse { explanation })
    }

    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        let reply = self.response(&request.message)?;
        Ok(ChatResponse { reply })
    }

    async fn chat_streaming(
        &self,
        request: ChatRequest,
    ) -> AiResult<tokio::sync::mpsc::Receiver<AiEvent>> {
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let result = self.response(&request.message);

        tokio::spawn(async move {
            match result {
                Ok(content) => {
                    let _ = tx.send(AiEvent::Done(content)).await;
                }
                Err(e) => {
                    let _ = tx.send(AiEvent::Error(e.to_string())).await;
                }
            }
        });

        Ok(rx)
    }

    async fn autocomplete(&self, request: AutocompleteRequest) -> AiResult<AutocompleteResponse> {
        let suggestion = self.response(&request.partial_input)?;
        Ok(AutocompleteResponse {
            suggestions: vec![suggestion],
        })
    }

    async fn is_available(&self) -> bool {
        self.config.enabled && !matches!(self.behaviour, MockServiceBehaviour::Error(_))
    }

    async fn status(&self) -> AiStatus {
        AiStatus {
            enabled: self.config.enabled,
            provider: "mock".to_string(),
            model: "mock".to_string(),
            ready: self.is_available().await,
            description: "Mock AI service for testing".to_string(),
        }
    }

    async fn switch_agent(&self, agent_id: &str) -> AiResult<()> {
        // Only allow known mock agents
        if agent_id != "shell" && agent_id != "git" {
            return Err(AiError::NotConfigured(format!(
                "Unknown agent '{}'. Use 'agents' to list available agents.",
                agent_id
            )));
        }
        *self.current_agent.lock() = agent_id.to_string();
        Ok(())
    }

    async fn current_agent(&self) -> AgentInfo {
        let id = self.current_agent.lock().clone();
        AgentInfo {
            id: id.clone(),
            display_name: id.clone(),
            description: format!("Mock agent: {}", id),
            active: true,
        }
    }

    async fn list_agents(&self) -> Vec<AgentInfo> {
        let active = self.current_agent.lock().clone();
        vec![
            AgentInfo {
                id: "shell".to_string(),
                display_name: "Shell".to_string(),
                description: "Mock shell agent".to_string(),
                active: active == "shell",
            },
            AgentInfo {
                id: "git".to_string(),
                display_name: "Git".to_string(),
                description: "Mock git agent".to_string(),
                active: active == "git",
            },
        ]
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
        rag: swebash_llm::spi::config::RagConfig::default(),
        tool_sandbox: None,
    }
}

// ── Service Builders ─────────────────────────────────────────────────

/// Build a mock `AiService` (no API key required).
///
/// Uses the echo behaviour by default: operations echo back the input.
pub fn create_mock_service() -> MockAiService {
    MockAiService::new(mock_config())
}

/// Build a mock service with a fixed response.
///
/// Every operation returns the given `response` string.
pub fn create_mock_service_fixed(response: &str) -> MockAiService {
    MockAiService::new(mock_config()).with_behaviour(MockServiceBehaviour::Fixed(response.into()))
}

/// Build a mock service where every operation returns an error.
pub fn create_mock_service_error(error_msg: &str) -> MockAiService {
    MockAiService::new(mock_config()).with_behaviour(MockServiceBehaviour::Error(error_msg.into()))
}

/// Build a mock service where all operations fail with the given error.
///
/// This is an alias for `create_mock_service_error` for compatibility.
pub fn create_mock_service_full_error(error_msg: &str) -> MockAiService {
    create_mock_service_error(error_msg)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        use swebash_llm::spi::AiClient;
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
        use swebash_llm::spi::AiClient;
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

    #[tokio::test]
    async fn mock_service_echo_behaviour() {
        let service = create_mock_service();
        let response = service
            .chat(ChatRequest {
                message: "Hello world".into(),
            })
            .await
            .unwrap();
        assert_eq!(response.reply, "Hello world");
    }

    #[tokio::test]
    async fn mock_service_fixed_behaviour() {
        let service = create_mock_service_fixed("Fixed response");
        let response = service
            .chat(ChatRequest {
                message: "Any message".into(),
            })
            .await
            .unwrap();
        assert_eq!(response.reply, "Fixed response");
    }

    #[tokio::test]
    async fn mock_service_error_behaviour() {
        let service = create_mock_service_error("Test error");
        let result = service
            .chat(ChatRequest {
                message: "Hello".into(),
            })
            .await;
        match result {
            Err(AiError::Provider(msg)) => assert_eq!(msg, "Test error"),
            other => panic!("Expected Provider error, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn mock_service_switch_agent() {
        let service = create_mock_service();
        let initial = service.current_agent().await;
        assert_eq!(initial.id, "shell");

        service.switch_agent("git").await.unwrap();
        let updated = service.current_agent().await;
        assert_eq!(updated.id, "git");
    }

    #[tokio::test]
    async fn mock_service_list_agents() {
        let service = create_mock_service();
        let agents = service.list_agents().await;
        assert_eq!(agents.len(), 2);
        assert!(agents.iter().any(|a| a.id == "shell"));
        assert!(agents.iter().any(|a| a.id == "git"));
    }
}
