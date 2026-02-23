//! Mock AI client for deterministic testing.
//!
//! `MockAiClient` implements `AiClient` using `MockLlmService` from llm-provider.
//! This enables agent behavior testing without requiring API keys.
//!
//! ## Environment Variables
//!
//! | Variable | Purpose |
//! |----------|---------|
//! | `SWEBASH_MOCK_RESPONSE` | Fixed response text |
//! | `SWEBASH_MOCK_RESPONSE_FILE` | Path to file containing response |
//! | `SWEBASH_MOCK_ERROR` | Force error mode (any value) |
//! | `SWEBASH_MOCK_REFLECT` | Reflect mode - echoes system prompt info |
//!
//! When none are set, defaults to echo mode (echoes the last user message).

use std::sync::Arc;

use async_trait::async_trait;

use llm_provider::testing::{MockBehaviour, MockLlmService};
use llm_provider::{
    CompletionBuilder, CompletionRequest, CompletionResponse, CompletionStream, FinishReason,
    LlmResult, LlmService, Message, MessageContent, ModelInfo, Role, TokenUsage,
};

use crate::api::error::{AiError, AiResult};
use crate::api::types::{AiMessage, AiResponse, AiRole, CompletionOptions};
use crate::spi::AiClient;

/// Mock AI client that uses `MockLlmService` for testing.
///
/// Configured via environment variables at construction time.
pub struct MockAiClient {
    llm: Arc<MockLlmService>,
    model: String,
    /// Whether this client was created with env-based configuration.
    /// When true, reflect mode can be activated via SWEBASH_MOCK_REFLECT.
    /// When false (explicit behaviour), reflect mode is not used.
    env_based: bool,
}

impl MockAiClient {
    /// Create a new mock client with behaviour from environment variables.
    pub fn new() -> Self {
        let behaviour = mock_behaviour_from_env();
        let llm = MockLlmService::new()
            .with_behaviour(behaviour)
            .with_model("mock-model")
            .with_provider_name("mock");
        Self {
            llm: Arc::new(llm),
            model: "mock-model".to_string(),
            env_based: true,
        }
    }

    /// Create a mock client with explicit behaviour.
    ///
    /// When created this way, reflect mode (SWEBASH_MOCK_REFLECT) is ignored.
    #[allow(dead_code)]
    pub fn with_behaviour(behaviour: MockBehaviour) -> Self {
        let llm = MockLlmService::new()
            .with_behaviour(behaviour)
            .with_model("mock-model")
            .with_provider_name("mock");
        Self {
            llm: Arc::new(llm),
            model: "mock-model".to_string(),
            env_based: false,
        }
    }

    /// Get the underlying `LlmService` for constructing chat engines.
    ///
    /// When `SWEBASH_MOCK_REFLECT=1` is set and this is an env-based client,
    /// returns a reflecting wrapper that echoes back request structure.
    /// Otherwise returns the inner `MockLlmService` directly.
    pub fn llm_service(&self) -> Arc<dyn LlmService> {
        if self.env_based && std::env::var("SWEBASH_MOCK_REFLECT").is_ok() {
            Arc::new(ReflectingLlmService::new(self.llm.clone()))
        } else {
            self.llm.clone()
        }
    }

    /// Generate a reflect response that echoes back request structure.
    ///
    /// This is useful for testing that system prompts, context, and
    /// conversation history are being passed correctly.
    fn reflect_response(&self, messages: &[AiMessage]) -> AiResponse {
        let mut parts = Vec::new();

        // Extract system prompt info
        let system_msgs: Vec<_> = messages
            .iter()
            .filter(|m| matches!(m.role, AiRole::System))
            .collect();

        if !system_msgs.is_empty() {
            // Extract first 100 chars of system prompt for identification
            let preview: String = system_msgs[0]
                .content
                .chars()
                .take(100)
                .collect();
            parts.push(format!("[SYSTEM_PROMPT:{}...]", preview.replace('\n', " ")));

            // Check for agent identity markers in system prompt
            let sys_content = &system_msgs[0].content;
            if sys_content.contains("shell") || sys_content.contains("Shell") {
                parts.push("[AGENT:shell]".to_string());
            }
            if sys_content.contains("review") || sys_content.contains("Review") || sys_content.contains("code quality") {
                parts.push("[AGENT:review]".to_string());
            }
            if sys_content.contains("git") || sys_content.contains("Git") || sys_content.contains("version control") {
                parts.push("[AGENT:git]".to_string());
            }
            if sys_content.contains("devops") || sys_content.contains("DevOps") || sys_content.contains("Docker") || sys_content.contains("Kubernetes") {
                parts.push("[AGENT:devops]".to_string());
            }

            // Check for docs/RAG context injection
            if sys_content.contains("## Documentation Context") || sys_content.contains("## Context from") {
                parts.push("[DOCS_INJECTED:true]".to_string());
            }
        }

        // Count messages by role
        let user_count = messages.iter().filter(|m| matches!(m.role, AiRole::User)).count();
        let assistant_count = messages.iter().filter(|m| matches!(m.role, AiRole::Assistant)).count();
        parts.push(format!("[HISTORY:user={},assistant={}]", user_count, assistant_count));

        // Echo the last user message
        if let Some(last_user) = messages.iter().rev().find(|m| matches!(m.role, AiRole::User)) {
            parts.push(format!("[USER:{}]", last_user.content));
        }

        AiResponse {
            content: parts.join(" "),
            model: self.model.clone(),
        }
    }
}

impl Default for MockAiClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper that adds reflect mode to an LlmService.
///
/// When reflect mode is active, this generates a response that echoes back
/// the structure of the request (system prompt, agent identity, history counts)
/// for testing purposes.
struct ReflectingLlmService {
    inner: Arc<dyn LlmService>,
}

impl ReflectingLlmService {
    fn new(inner: Arc<dyn LlmService>) -> Self {
        Self { inner }
    }

    /// Extract text from MessageContent
    fn get_text(content: &MessageContent) -> String {
        match content {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    llm_provider::ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    /// Generate reflect response from completion request messages.
    fn reflect_response(&self, messages: &[Message]) -> String {
        let mut parts = Vec::new();

        // Extract system prompt info
        let system_msgs: Vec<_> = messages
            .iter()
            .filter(|m| matches!(m.role, Role::System))
            .collect();

        if !system_msgs.is_empty() {
            let sys_text = Self::get_text(&system_msgs[0].content);

            // Extract first 100 chars of system prompt for identification
            let preview: String = sys_text.chars().take(100).collect();
            parts.push(format!("[SYSTEM_PROMPT:{}...]", preview.replace('\n', " ")));

            // Check for agent identity markers in system prompt
            if sys_text.contains("shell") || sys_text.contains("Shell") {
                parts.push("[AGENT:shell]".to_string());
            }
            if sys_text.contains("review")
                || sys_text.contains("Review")
                || sys_text.contains("code quality")
            {
                parts.push("[AGENT:review]".to_string());
            }
            if sys_text.contains("git")
                || sys_text.contains("Git")
                || sys_text.contains("version control")
            {
                parts.push("[AGENT:git]".to_string());
            }
            if sys_text.contains("devops")
                || sys_text.contains("DevOps")
                || sys_text.contains("Docker")
                || sys_text.contains("Kubernetes")
            {
                parts.push("[AGENT:devops]".to_string());
            }

            // Check for docs/RAG context injection
            if sys_text.contains("## Documentation Context")
                || sys_text.contains("## Context from")
            {
                parts.push("[DOCS_INJECTED:true]".to_string());
            }
        }

        // Count messages by role
        let user_count = messages
            .iter()
            .filter(|m| matches!(m.role, Role::User))
            .count();
        let assistant_count = messages
            .iter()
            .filter(|m| matches!(m.role, Role::Assistant))
            .count();
        parts.push(format!(
            "[HISTORY:user={},assistant={}]",
            user_count, assistant_count
        ));

        // Echo the last user message
        if let Some(last_user) = messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::User))
        {
            let user_text = Self::get_text(&last_user.content);
            parts.push(format!("[USER:{}]", user_text));
        }

        parts.join(" ")
    }
}

#[async_trait]
impl LlmService for ReflectingLlmService {
    async fn complete(&self, request: CompletionRequest) -> LlmResult<CompletionResponse> {
        let content = self.reflect_response(&request.messages);
        Ok(CompletionResponse {
            id: uuid::Uuid::new_v4().to_string(),
            content: Some(content),
            model: "mock-model".to_string(),
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

    async fn complete_stream(&self, request: CompletionRequest) -> LlmResult<CompletionStream> {
        // For streaming, generate the reflect response and stream it as a single chunk
        let content = self.reflect_response(&request.messages);
        let chunk = llm_provider::StreamChunk {
            id: uuid::Uuid::new_v4().to_string(),
            delta: llm_provider::StreamDelta {
                content: Some(content),
                tool_calls: None,
            },
            finish_reason: Some(FinishReason::Stop),
        };
        let stream = futures::stream::once(async move { Ok(chunk) });
        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        self.inner.list_models().await
    }

    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo> {
        self.inner.model_info(model).await
    }

    async fn is_model_available(&self, model: &str) -> bool {
        self.inner.is_model_available(model).await
    }

    async fn providers(&self) -> Vec<String> {
        self.inner.providers().await
    }
}

/// Determine mock behaviour from environment variables.
///
/// Priority:
/// 1. `SWEBASH_MOCK_ERROR` - Force error mode
/// 2. `SWEBASH_MOCK_RESPONSE` - Fixed inline response
/// 3. `SWEBASH_MOCK_RESPONSE_FILE` - Response from file
/// 4. Default: Echo mode (echoes last user message)
pub fn mock_behaviour_from_env() -> MockBehaviour {
    // Error mode takes precedence
    if let Ok(msg) = std::env::var("SWEBASH_MOCK_ERROR") {
        let error_msg = if msg.is_empty() { "mock error".to_string() } else { msg };
        return MockBehaviour::Error(error_msg);
    }

    // Fixed inline response
    if let Ok(response) = std::env::var("SWEBASH_MOCK_RESPONSE") {
        return MockBehaviour::Fixed(response);
    }

    // Response from file
    if let Ok(path) = std::env::var("SWEBASH_MOCK_RESPONSE_FILE") {
        match std::fs::read_to_string(&path) {
            Ok(content) => return MockBehaviour::Fixed(content),
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Failed to read mock response file, using echo mode");
            }
        }
    }

    // Default: echo mode
    MockBehaviour::Echo
}

#[async_trait]
impl AiClient for MockAiClient {
    async fn complete(
        &self,
        messages: Vec<AiMessage>,
        options: CompletionOptions,
    ) -> AiResult<AiResponse> {
        // Check for reflect mode - returns structured info about the request
        // Only use reflect mode for env-based clients (not explicit behaviour)
        // NOTE: This method is only used for direct AiClient calls (translate, explain).
        // Chat messages go through the ChatEngine which uses LlmService directly.
        if self.env_based && std::env::var("SWEBASH_MOCK_REFLECT").is_ok() {
            return Ok(self.reflect_response(&messages));
        }

        let mut builder = CompletionBuilder::new(&self.model);

        for msg in messages {
            builder = match msg.role {
                AiRole::System => builder.system(msg.content),
                AiRole::User => builder.user(msg.content),
                AiRole::Assistant => builder.assistant(msg.content),
            };
        }

        if let Some(temp) = options.temperature {
            builder = builder.temperature(temp);
        }
        if let Some(max) = options.max_tokens {
            builder = builder.max_tokens(max);
        }

        let response = builder
            .execute(self.llm.as_ref())
            .await
            .map_err(|e| AiError::Provider(e.to_string()))?;

        Ok(AiResponse {
            content: response.content.unwrap_or_default(),
            model: response.model,
        })
    }

    async fn is_ready(&self) -> bool {
        true
    }

    fn description(&self) -> String {
        "mock:mock-model".to_string()
    }

    fn provider_name(&self) -> String {
        "mock".to_string()
    }

    fn model_name(&self) -> String {
        self.model.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn mock_behaviour_defaults_to_echo() {
        std::env::remove_var("SWEBASH_MOCK_RESPONSE");
        std::env::remove_var("SWEBASH_MOCK_RESPONSE_FILE");
        std::env::remove_var("SWEBASH_MOCK_ERROR");

        let behaviour = mock_behaviour_from_env();
        assert!(matches!(behaviour, MockBehaviour::Echo));
    }

    #[test]
    #[serial]
    fn mock_behaviour_fixed_response() {
        std::env::set_var("SWEBASH_MOCK_RESPONSE", "test response");
        std::env::remove_var("SWEBASH_MOCK_RESPONSE_FILE");
        std::env::remove_var("SWEBASH_MOCK_ERROR");

        let behaviour = mock_behaviour_from_env();
        match behaviour {
            MockBehaviour::Fixed(s) => assert_eq!(s, "test response"),
            _ => panic!("Expected Fixed behaviour"),
        }

        std::env::remove_var("SWEBASH_MOCK_RESPONSE");
    }

    #[test]
    #[serial]
    fn mock_behaviour_error_mode() {
        std::env::set_var("SWEBASH_MOCK_ERROR", "custom error");
        std::env::remove_var("SWEBASH_MOCK_RESPONSE");
        std::env::remove_var("SWEBASH_MOCK_RESPONSE_FILE");

        let behaviour = mock_behaviour_from_env();
        match behaviour {
            MockBehaviour::Error(msg) => assert_eq!(msg, "custom error"),
            _ => panic!("Expected Error behaviour"),
        }

        std::env::remove_var("SWEBASH_MOCK_ERROR");
    }

    #[test]
    #[serial]
    fn mock_behaviour_error_precedence() {
        // Error takes precedence over response
        std::env::set_var("SWEBASH_MOCK_ERROR", "error");
        std::env::set_var("SWEBASH_MOCK_RESPONSE", "response");

        let behaviour = mock_behaviour_from_env();
        assert!(matches!(behaviour, MockBehaviour::Error(_)));

        std::env::remove_var("SWEBASH_MOCK_ERROR");
        std::env::remove_var("SWEBASH_MOCK_RESPONSE");
    }

    #[tokio::test]
    #[serial]
    async fn mock_client_ready() {
        std::env::remove_var("SWEBASH_MOCK_RESPONSE");
        std::env::remove_var("SWEBASH_MOCK_RESPONSE_FILE");
        std::env::remove_var("SWEBASH_MOCK_ERROR");

        let client = MockAiClient::new();
        assert!(client.is_ready().await);
        assert_eq!(client.provider_name(), "mock");
        assert_eq!(client.model_name(), "mock-model");
    }

    #[tokio::test]
    async fn mock_client_with_fixed_response() {
        let client = MockAiClient::with_behaviour(MockBehaviour::Fixed("Hello!".into()));

        let messages = vec![AiMessage {
            role: AiRole::User,
            content: "Hi".to_string(),
        }];

        let response = client.complete(messages, CompletionOptions::default()).await.unwrap();
        assert_eq!(response.content, "Hello!");
    }

    #[tokio::test]
    async fn mock_client_echo_mode() {
        let client = MockAiClient::with_behaviour(MockBehaviour::Echo);

        let messages = vec![AiMessage {
            role: AiRole::User,
            content: "Echo this".to_string(),
        }];

        let response = client.complete(messages, CompletionOptions::default()).await.unwrap();
        assert_eq!(response.content, "Echo this");
    }

    #[tokio::test]
    #[serial]
    async fn mock_client_reflect_mode() {
        std::env::set_var("SWEBASH_MOCK_REFLECT", "1");
        std::env::remove_var("SWEBASH_MOCK_RESPONSE");
        std::env::remove_var("SWEBASH_MOCK_ERROR");

        let client = MockAiClient::new();

        let messages = vec![
            AiMessage {
                role: AiRole::System,
                content: "You are a helpful shell assistant.".to_string(),
            },
            AiMessage {
                role: AiRole::User,
                content: "hello".to_string(),
            },
        ];

        let response = client.complete(messages, CompletionOptions::default()).await.unwrap();
        assert!(response.content.contains("[SYSTEM_PROMPT:"));
        assert!(response.content.contains("[AGENT:shell]"));
        assert!(response.content.contains("[USER:hello]"));
        assert!(response.content.contains("[HISTORY:user=1,assistant=0]"));

        std::env::remove_var("SWEBASH_MOCK_REFLECT");
    }

    #[tokio::test]
    #[serial]
    async fn mock_client_reflect_detects_agents() {
        std::env::set_var("SWEBASH_MOCK_REFLECT", "1");

        let client = MockAiClient::new();

        // Test review agent detection
        let messages = vec![
            AiMessage {
                role: AiRole::System,
                content: "You are a code review assistant focused on code quality.".to_string(),
            },
            AiMessage {
                role: AiRole::User,
                content: "review this".to_string(),
            },
        ];
        let response = client.complete(messages, CompletionOptions::default()).await.unwrap();
        assert!(response.content.contains("[AGENT:review]"));

        // Test git agent detection
        let messages = vec![
            AiMessage {
                role: AiRole::System,
                content: "You are a Git and version control expert.".to_string(),
            },
            AiMessage {
                role: AiRole::User,
                content: "help with git".to_string(),
            },
        ];
        let response = client.complete(messages, CompletionOptions::default()).await.unwrap();
        assert!(response.content.contains("[AGENT:git]"));

        std::env::remove_var("SWEBASH_MOCK_REFLECT");
    }

    #[tokio::test]
    #[serial]
    async fn mock_client_reflect_shows_history_count() {
        std::env::set_var("SWEBASH_MOCK_REFLECT", "1");

        let client = MockAiClient::new();

        let messages = vec![
            AiMessage { role: AiRole::System, content: "System".to_string() },
            AiMessage { role: AiRole::User, content: "First".to_string() },
            AiMessage { role: AiRole::Assistant, content: "Response 1".to_string() },
            AiMessage { role: AiRole::User, content: "Second".to_string() },
            AiMessage { role: AiRole::Assistant, content: "Response 2".to_string() },
            AiMessage { role: AiRole::User, content: "Third".to_string() },
        ];

        let response = client.complete(messages, CompletionOptions::default()).await.unwrap();
        assert!(response.content.contains("[HISTORY:user=3,assistant=2]"));
        assert!(response.content.contains("[USER:Third]"));

        std::env::remove_var("SWEBASH_MOCK_REFLECT");
    }
}
