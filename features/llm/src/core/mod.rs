/// L4 Core: DefaultAiService orchestration.
///
/// Wires the GatewayClient to the API service trait, delegating
/// all chat and agent operations through llmboot's gateway.
///
/// Stateless features (translate, explain, autocomplete) use the LLM provider directly.
pub mod complete;
pub mod explain;
pub mod prompt;
pub mod rag;
pub mod translate;
pub mod tools;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::api::error::{AiError, AiResult};
use crate::api::types::*;
use crate::api::AiService;
use crate::spi::config::AiConfig;
use crate::spi::{AiClient, GatewayClient};

/// The default implementation of `AiService`.
///
/// Uses llmboot's GatewayClient for agent-based chat operations.
/// The `active_agent` tracks which agent handles chat messages.
pub struct DefaultAiService {
    gateway: Arc<GatewayClient>,
    config: AiConfig,
    active_agent: RwLock<String>,
}

impl DefaultAiService {
    /// Create a new service with the given gateway client and config.
    pub fn new(gateway: GatewayClient, config: AiConfig) -> Self {
        let default_agent = config.default_agent.clone();
        Self {
            gateway: Arc::new(gateway),
            config,
            active_agent: RwLock::new(default_agent),
        }
    }

    /// Get the underlying gateway client.
    pub fn gateway(&self) -> &GatewayClient {
        &self.gateway
    }
}

#[async_trait]
impl AiService for DefaultAiService {
    async fn translate(&self, request: TranslateRequest) -> AiResult<TranslateResponse> {
        self.ensure_ready().await?;
        translate::translate_via_gateway(&self.gateway, request).await
    }

    async fn explain(&self, request: ExplainRequest) -> AiResult<ExplainResponse> {
        self.ensure_ready().await?;
        explain::explain_via_gateway(&self.gateway, request).await
    }

    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        self.ensure_ready().await?;
        let agent_id = self.active_agent.read().await.clone();

        let response = self.gateway.execute(&agent_id, &request.message).await?;

        Ok(ChatResponse {
            reply: response.content.trim().to_string(),
        })
    }

    async fn chat_streaming(
        &self,
        request: ChatRequest,
    ) -> AiResult<tokio::sync::mpsc::Receiver<AiEvent>> {
        self.ensure_ready().await?;
        let agent_id = self.active_agent.read().await.clone();

        // For now, use non-streaming and emit Done event
        // TODO: Implement proper streaming via gateway when available
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let gateway = self.gateway.clone();
        let agent = agent_id.clone();
        let message = request.message.clone();

        tokio::spawn(async move {
            match gateway.execute(&agent, &message).await {
                Ok(response) => {
                    let _ = tx.send(AiEvent::Done(response.content.trim().to_string())).await;
                }
                Err(e) => {
                    let _ = tx.send(AiEvent::Error(e.to_string())).await;
                }
            }
        });

        Ok(rx)
    }

    async fn autocomplete(&self, request: AutocompleteRequest) -> AiResult<AutocompleteResponse> {
        self.ensure_ready().await?;
        complete::autocomplete_via_gateway(&self.gateway, request).await
    }

    async fn is_available(&self) -> bool {
        self.config.enabled && self.gateway.is_ready().await
    }

    async fn status(&self) -> AiStatus {
        AiStatus {
            enabled: self.config.enabled,
            provider: self.gateway.provider_name(),
            model: self.gateway.model_name(),
            ready: self.gateway.is_ready().await,
            description: self.gateway.description(),
        }
    }

    async fn switch_agent(&self, agent_id: &str) -> AiResult<()> {
        if !self.gateway.has_agent(agent_id) {
            return Err(AiError::NotConfigured(format!(
                "Unknown agent '{}'. Use 'agents' to list available agents.",
                agent_id
            )));
        }
        let mut active = self.active_agent.write().await;
        *active = agent_id.to_string();
        self.gateway.set_agent(agent_id);
        Ok(())
    }

    async fn current_agent(&self) -> AgentInfo {
        let agent_id = self.active_agent.read().await.clone();
        AgentInfo {
            id: agent_id.clone(),
            display_name: agent_id.clone(),
            description: format!("Agent: {}", agent_id),
            active: true,
        }
    }

    async fn list_agents(&self) -> Vec<AgentInfo> {
        let active_id = self.active_agent.read().await.clone();
        self.gateway
            .available_agents()
            .iter()
            .map(|id| AgentInfo {
                id: id.clone(),
                display_name: id.clone(),
                description: format!("Agent: {}", id),
                active: id == &active_id,
            })
            .collect()
    }
}

impl DefaultAiService {
    async fn ensure_ready(&self) -> AiResult<()> {
        if !self.config.enabled {
            return Err(AiError::NotConfigured(
                "AI features are disabled. Set SWEBASH_AI_ENABLED=true to enable.".into(),
            ));
        }
        if !self.gateway.is_ready().await {
            return Err(AiError::NotConfigured(
                "AI provider is not ready. Check your API key and provider configuration.".into(),
            ));
        }
        Ok(())
    }

    /// Get the active agent ID.
    pub async fn active_agent_id(&self) -> String {
        self.active_agent.read().await.clone()
    }

    /// Auto-detect the best agent for the given input, if enabled.
    ///
    /// Returns `Some(agent_id)` if a better agent was detected and switched to.
    pub async fn auto_detect_and_switch(&self, _input: &str) -> Option<String> {
        // Agent auto-detection is handled by the gateway
        // For now, return None (no auto-switching at this level)
        None
    }

    /// Update the sandbox's current working directory.
    ///
    /// Call this whenever the shell's virtual_cwd changes so that AI tools
    /// resolve relative paths correctly.
    pub fn set_sandbox_cwd(&self, cwd: std::path::PathBuf) {
        if let Some(sandbox) = &self.config.tool_sandbox {
            sandbox.set_cwd(cwd);
        }
    }

    /// Get a formatted display of conversation history.
    pub async fn format_history(&self) -> String {
        // Gateway handles memory internally; for now return placeholder
        "(history managed by gateway)".to_string()
    }

    /// Clear conversation history.
    pub async fn clear_history(&self) {
        // Gateway handles memory internally
        tracing::info!("clear_history: gateway manages memory");
    }

    /// Clear all conversation history.
    pub async fn clear_all_history(&self) {
        // Gateway handles memory internally
        tracing::info!("clear_all_history: gateway manages memory");
    }
}
