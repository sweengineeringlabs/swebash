/// L2 SPI implementation: delegates to llmboot's L1 Gateway API.
///
/// This is the NEW implementation that routes all AI operations through
/// llmboot's GatewayService, replacing the rustratify-based ChatProviderClient.
///
/// The Gateway provides:
/// - Input validation and sanitization
/// - Guardrails (injection detection, PII masking)
/// - Agent runtime with pattern execution (ReAct, CoT, etc.)
/// - Tool orchestration
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use crate::api::error::{AiError, AiResult};
use crate::api::types::{AiMessage, AiResponse, AiRole, CompletionOptions};
use crate::spi::AiClient;

// llmboot L1 Gateway API
use llmboot_input::{
    create_gateway, GatewayService, AgentResponse, GatewayError, GatewayErrorCode,
};
use llmboot_orchestration::{
    load_agents_from_file, ToolPool, LlmProvider,
    create_session, SessionContext, SessionError,
};

/// Client that wraps llmboot's GatewayService.
///
/// This implements the `AiClient` trait using llmboot's L1 entry point,
/// which provides:
/// - Input validation and guardrails
/// - Agent management and execution
/// - Tool orchestration
/// - Pattern execution (ReAct, CoT, etc.)
pub struct GatewayClient {
    gateway: Arc<dyn GatewayService>,
    session: SessionContext,
    provider: String,
    model: String,
    /// Current agent ID for completions
    current_agent: RwLock<String>,
}

impl GatewayClient {
    /// Create a new gateway client from an agents YAML file.
    ///
    /// # Arguments
    /// * `agents_path` - Path to the agents YAML configuration file
    /// * `provider` - LLM provider name (e.g., "openai", "anthropic")
    /// * `model` - Model to use for completions
    ///
    /// # Errors
    /// Returns an error if:
    /// - LLM provider cannot be initialized (missing API key, etc.)
    /// - Agents file cannot be loaded or parsed
    /// - Gateway creation fails
    pub async fn new(
        agents_path: impl AsRef<Path>,
        provider: &str,
        model: &str,
    ) -> AiResult<Self> {
        // Create session with LLM provider
        let session = create_session().await.map_err(map_session_error)?;

        // Create an empty tool pool - swebash uses its own tool system
        // TODO: Register swebash tools in the pool when migrating fully
        let tool_pool = ToolPool::new();

        // Load agents from YAML
        let loaded = load_agents_from_file(agents_path.as_ref(), session.clone(), tool_pool)
            .map_err(|e| AiError::NotConfigured(format!("Failed to load agents: {}", e)))?;

        // Create the gateway
        let gateway = create_gateway(loaded)
            .map_err(|e| AiError::NotConfigured(format!("Failed to create gateway: {}", e)))?;

        tracing::info!(
            provider = %provider,
            model = %model,
            "Gateway client initialized via llmboot L1 API"
        );

        Ok(Self {
            gateway,
            session,
            provider: provider.to_string(),
            model: model.to_string(),
            current_agent: RwLock::new("shell".to_string()),
        })
    }

    /// Create a gateway client with a pre-configured gateway and session.
    ///
    /// This is useful for testing or when you want to customize
    /// the gateway creation process.
    pub fn with_gateway(
        gateway: Arc<dyn GatewayService>,
        session: SessionContext,
        provider: &str,
        model: &str,
    ) -> Self {
        Self {
            gateway,
            session,
            provider: provider.to_string(),
            model: model.to_string(),
            current_agent: RwLock::new("shell".to_string()),
        }
    }

    /// Set the current agent for completions.
    pub fn set_agent(&self, agent_id: &str) {
        let mut current = self.current_agent.write();
        *current = agent_id.to_string();
    }

    /// Get the current agent ID.
    pub fn current_agent(&self) -> String {
        self.current_agent.read().clone()
    }

    /// Get available agent IDs.
    pub fn available_agents(&self) -> Vec<String> {
        self.gateway.available_agents().unwrap_or_default()
    }

    /// Check if an agent exists.
    pub fn has_agent(&self, agent_id: &str) -> bool {
        self.gateway.has_agent(agent_id)
    }

    /// Execute a request through the gateway.
    ///
    /// This is the primary method for interacting with agents through
    /// the llmboot gateway.
    pub async fn execute(&self, agent_id: &str, input: &str) -> AiResult<AgentResponse> {
        self.gateway
            .execute(agent_id, input, &self.model)
            .await
            .map_err(map_gateway_error)
    }

    /// Get the underlying LLM provider for direct access.
    ///
    /// This is useful for stateless completions that don't need
    /// agent context or tool access.
    pub fn llm_provider(&self) -> Arc<dyn LlmProvider> {
        self.session.llm()
    }

    /// Get the session context.
    ///
    /// The session context provides access to provider info and auth details.
    pub fn session(&self) -> &SessionContext {
        &self.session
    }
}

#[async_trait]
impl AiClient for GatewayClient {
    async fn complete(
        &self,
        messages: Vec<AiMessage>,
        _options: CompletionOptions,
    ) -> AiResult<AiResponse> {
        // Extract the last user message as input
        let input = messages
            .iter()
            .filter(|m| matches!(m.role, AiRole::User))
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("");

        if input.is_empty() {
            return Err(AiError::ParseError("No user message provided".into()));
        }

        // Execute through the gateway with the current agent
        let agent_id = self.current_agent.read().clone();
        let response = self.execute(&agent_id, input).await?;

        Ok(AiResponse {
            content: response.content,
            model: self.model.clone(),
        })
    }

    async fn is_ready(&self) -> bool {
        // Check if we have at least one agent available
        !self.available_agents().is_empty()
    }

    fn description(&self) -> String {
        format!("{}:{} (llmboot gateway)", self.provider, self.model)
    }

    fn provider_name(&self) -> String {
        self.provider.clone()
    }

    fn model_name(&self) -> String {
        self.model.clone()
    }
}

/// Convert llmboot SessionError to swebash AiError.
fn map_session_error(err: SessionError) -> AiError {
    AiError::NotConfigured(err.to_string())
}

/// Convert llmboot GatewayError to swebash AiError.
fn map_gateway_error(err: GatewayError) -> AiError {
    let details = err.details.as_ref().unwrap_or(&err.message);
    match err.code {
        GatewayErrorCode::InvalidInput => AiError::ParseError(details.clone()),
        GatewayErrorCode::NotFound => AiError::NotConfigured(details.clone()),
        GatewayErrorCode::Timeout => AiError::Timeout,
        GatewayErrorCode::Unavailable => AiError::RateLimited,
        GatewayErrorCode::Internal | GatewayErrorCode::Configuration => {
            AiError::Provider(details.clone())
        }
        _ => AiError::Provider(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_gateway_error_invalid_input() {
        let err = GatewayError::new(GatewayErrorCode::InvalidInput, "test input error");
        let ai_err = map_gateway_error(err);
        assert!(matches!(ai_err, AiError::ParseError(_)));
    }

    #[test]
    fn test_map_gateway_error_not_found() {
        let err = GatewayError::new(GatewayErrorCode::NotFound, "agent not found");
        let ai_err = map_gateway_error(err);
        assert!(matches!(ai_err, AiError::NotConfigured(_)));
    }
}
