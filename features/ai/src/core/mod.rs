/// L4 Core: DefaultAiService orchestration.
///
/// Wires the SPI client to the API service trait, delegating
/// to feature-specific modules for each operation.
///
/// Chat is routed through the agent framework: each agent has its own
/// `ChatEngine` instance with isolated memory, system prompt, and tool access.
/// Stateless features (translate, explain, autocomplete) use the `AiClient` directly.
pub mod agents;
pub mod chat;
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
use crate::spi::AiClient;

use agents::{AgentDescriptor, AgentManager};

/// The default implementation of `AiService`.
///
/// Uses an `AgentManager` to manage purpose-built agents, each with
/// its own `ChatEngine`, system prompt, and tool configuration.
/// The `active_agent` tracks which agent handles chat messages.
pub struct DefaultAiService {
    client: Box<dyn AiClient>,
    config: AiConfig,
    agents: AgentManager,
    active_agent: RwLock<String>,
}

impl DefaultAiService {
    /// Create a new service with the given client, agent manager, and config.
    pub fn new(
        client: Box<dyn AiClient>,
        agents: AgentManager,
        config: AiConfig,
    ) -> Self {
        let default_agent = config.default_agent.clone();
        Self {
            client,
            config,
            agents,
            active_agent: RwLock::new(default_agent),
        }
    }

    /// Get the chat engine for the currently active agent.
    async fn active_engine(&self) -> AiResult<Arc<dyn chat_engine::ChatEngine>> {
        let agent_id = self.active_agent.read().await.clone();
        self.agents
            .engine_for(&agent_id)
            .ok_or_else(|| AiError::NotConfigured(format!("Agent '{}' not found", agent_id)))
    }
}

#[async_trait]
impl AiService for DefaultAiService {
    async fn translate(&self, request: TranslateRequest) -> AiResult<TranslateResponse> {
        self.ensure_ready().await?;
        translate::translate(self.client.as_ref(), request).await
    }

    async fn explain(&self, request: ExplainRequest) -> AiResult<ExplainResponse> {
        self.ensure_ready().await?;
        explain::explain(self.client.as_ref(), request).await
    }

    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        self.ensure_ready().await?;
        let engine = self.active_engine().await?;
        chat::chat(engine.as_ref(), request).await
    }

    async fn chat_streaming(
        &self,
        request: ChatRequest,
    ) -> AiResult<tokio::sync::mpsc::Receiver<AiEvent>> {
        self.ensure_ready().await?;
        let engine = self.active_engine().await?;
        chat::chat_streaming(&engine, request).await
    }

    async fn autocomplete(&self, request: AutocompleteRequest) -> AiResult<AutocompleteResponse> {
        self.ensure_ready().await?;
        complete::autocomplete(self.client.as_ref(), request).await
    }

    async fn is_available(&self) -> bool {
        self.config.enabled && self.client.is_ready().await
    }

    async fn status(&self) -> AiStatus {
        AiStatus {
            enabled: self.config.enabled,
            provider: self.client.provider_name(),
            model: self.client.model_name(),
            ready: self.client.is_ready().await,
            description: self.client.description(),
        }
    }

    async fn switch_agent(&self, agent_id: &str) -> AiResult<()> {
        if self.agents.get(agent_id).is_none() {
            let hint = if let Some(suggested) = self.agents.suggest_agent(agent_id) {
                format!(
                    "Unknown agent '{}'. Did you mean '@{}'? Use 'agents' to list available agents.",
                    agent_id, suggested
                )
            } else {
                format!(
                    "Unknown agent '{}'. Use 'agents' to list available agents.",
                    agent_id
                )
            };
            return Err(AiError::NotConfigured(hint));
        }
        let mut active = self.active_agent.write().await;
        *active = agent_id.to_string();
        Ok(())
    }

    async fn current_agent(&self) -> AgentInfo {
        let agent_id = self.active_agent.read().await.clone();
        match self.agents.get(&agent_id) {
            Some(agent) => AgentInfo {
                id: agent.id().to_string(),
                display_name: agent.display_name().to_string(),
                description: agent.description().to_string(),
                active: true,
            },
            None => AgentInfo {
                id: agent_id,
                display_name: "Unknown".to_string(),
                description: "Agent not found".to_string(),
                active: true,
            },
        }
    }

    async fn list_agents(&self) -> Vec<AgentInfo> {
        let active_id = self.active_agent.read().await.clone();
        self.agents
            .list()
            .iter()
            .map(|agent| AgentInfo {
                id: agent.id().to_string(),
                display_name: agent.display_name().to_string(),
                description: agent.description().to_string(),
                active: agent.id() == active_id,
            })
            .collect()
    }
}

impl DefaultAiService {
    async fn ensure_ready(&self) -> AiResult<()> {
        if !self.config.enabled {
            return Err(AiError::NotConfigured("AI features are disabled. Set SWEBASH_AI_ENABLED=true to enable.".into()));
        }
        if !self.client.is_ready().await {
            return Err(AiError::NotConfigured(
                "AI provider is not ready. Check your API key and provider configuration.".into(),
            ));
        }
        Ok(())
    }

    /// Get a formatted display of conversation history for the active agent.
    pub async fn format_history(&self) -> String {
        let engine = match self.active_engine().await {
            Ok(e) => e,
            Err(_) => return "(no active agent)".to_string(),
        };

        let messages = engine
            .memory()
            .get_all_messages()
            .await
            .unwrap_or_default();

        let mut output = String::new();
        for msg in &messages {
            let role_label = match msg.role {
                chat_engine::ChatRole::System => continue,
                chat_engine::ChatRole::User => "You",
                chat_engine::ChatRole::Assistant => "AI",
            };
            output.push_str(&format!("[{}] {}\n", role_label, msg.content));
        }
        if output.is_empty() {
            output.push_str("(no chat history)");
        }
        output
    }

    /// Clear conversation history for the active agent.
    pub async fn clear_history(&self) {
        if let Ok(engine) = self.active_engine().await {
            let _ = engine.new_conversation().await;
        }
    }

    /// Clear conversation history for all agents.
    pub async fn clear_all_history(&self) {
        self.agents.clear_all();
    }

    /// Get the active agent ID.
    pub async fn active_agent_id(&self) -> String {
        self.active_agent.read().await.clone()
    }

    /// Auto-detect the best agent for the given input, if enabled.
    ///
    /// Returns `Some(agent_id)` if a better agent was detected and switched to.
    pub async fn auto_detect_and_switch(&self, input: &str) -> Option<String> {
        if !self.config.agent_auto_detect {
            return None;
        }
        let current = self.active_agent.read().await.clone();
        if let Some(detected) = self.agents.detect_agent(input) {
            if detected != current {
                let mut active = self.active_agent.write().await;
                *active = detected.to_string();
                return Some(detected.to_string());
            }
        }
        None
    }

    /// Update the sandbox's current working directory.
    ///
    /// Call this whenever the shell's virtual_cwd changes so that AI tools
    /// resolve relative paths correctly. Without this, the AI would use
    /// `std::env::current_dir()` which doesn't track the shell's `cd` commands.
    pub fn set_sandbox_cwd(&self, cwd: std::path::PathBuf) {
        if let Some(sandbox) = &self.config.tool_sandbox {
            sandbox.set_cwd(cwd);
        }
    }
}
