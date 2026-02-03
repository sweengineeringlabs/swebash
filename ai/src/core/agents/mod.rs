/// Agent framework: purpose-built AI agents with dedicated prompts, tools, and memory.
///
/// Each agent has its own `ChatEngine` instance (lazily created), system prompt,
/// and tool filter. The `AgentRegistry` manages agent lifecycle and provides
/// engine access by agent ID.
pub mod builtins;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use chat_engine::{ChatConfig, ChatEngine, SimpleChatEngine, ToolAwareChatEngine};
use llm_provider::LlmService;

use crate::config::{AiConfig, ToolConfig};
use crate::core::tools;

/// Controls which tool categories an agent can access.
#[derive(Debug, Clone)]
pub enum ToolFilter {
    /// All configured tools are available.
    All,
    /// No tools — pure conversational agent.
    None,
    /// Selective tool access by category.
    Only {
        fs: bool,
        exec: bool,
        web: bool,
    },
}

/// A purpose-built AI agent with its own prompt, tools, and behavior.
pub trait Agent: Send + Sync {
    /// Unique identifier (e.g. "shell", "review").
    fn id(&self) -> &str;

    /// Human-readable name (e.g. "Shell Assistant").
    fn display_name(&self) -> &str;

    /// Short description of what this agent does.
    fn description(&self) -> &str;

    /// The system prompt that defines this agent's behavior.
    fn system_prompt(&self) -> String;

    /// Which tool categories this agent can use.
    fn tool_filter(&self) -> ToolFilter {
        ToolFilter::All
    }

    /// Optional temperature override.
    fn temperature(&self) -> Option<f32> {
        None
    }

    /// Optional max_tokens override.
    fn max_tokens(&self) -> Option<u32> {
        None
    }

    /// Keywords that trigger auto-detection of this agent.
    fn trigger_keywords(&self) -> Vec<&str> {
        vec![]
    }
}

/// Registry of agents with lazily-cached chat engines.
///
/// Each agent gets its own `ChatEngine` instance with isolated memory,
/// created on first use and cached for the session.
pub struct AgentRegistry {
    agents: HashMap<String, Box<dyn Agent>>,
    engines: RwLock<HashMap<String, Arc<dyn ChatEngine>>>,
    llm: Arc<dyn LlmService>,
    config: AiConfig,
}

impl AgentRegistry {
    /// Create a new registry with the given LLM service and config.
    pub fn new(llm: Arc<dyn LlmService>, config: AiConfig) -> Self {
        Self {
            agents: HashMap::new(),
            engines: RwLock::new(HashMap::new()),
            llm,
            config,
        }
    }

    /// Register an agent. Overwrites any existing agent with the same ID.
    pub fn register(&mut self, agent: Box<dyn Agent>) {
        self.agents.insert(agent.id().to_string(), agent);
    }

    /// Get an agent by ID.
    pub fn get(&self, id: &str) -> Option<&dyn Agent> {
        self.agents.get(id).map(|a| a.as_ref())
    }

    /// List all registered agents.
    pub fn list(&self) -> Vec<&dyn Agent> {
        let mut agents: Vec<&dyn Agent> = self.agents.values().map(|a| a.as_ref()).collect();
        agents.sort_by_key(|a| a.id());
        agents
    }

    /// Get or create the chat engine for an agent.
    ///
    /// Engines are lazily created on first access and cached for the session.
    /// Each agent gets isolated conversation memory.
    pub async fn engine_for(&self, agent_id: &str) -> Option<Arc<dyn ChatEngine>> {
        // Fast path: engine already cached
        {
            let engines = self.engines.read().await;
            if let Some(engine) = engines.get(agent_id) {
                return Some(engine.clone());
            }
        }

        // Slow path: create engine
        let agent = self.agents.get(agent_id)?;
        let engine = self.create_engine(agent.as_ref());

        let mut engines = self.engines.write().await;
        engines.insert(agent_id.to_string(), engine.clone());
        Some(engine)
    }

    /// Detect which agent should handle the given input based on trigger keywords.
    ///
    /// Returns the agent ID if a match is found, `None` otherwise.
    pub fn detect_agent(&self, input: &str) -> Option<&str> {
        let lower = input.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();

        for agent in self.agents.values() {
            let keywords = agent.trigger_keywords();
            if keywords.is_empty() {
                continue;
            }
            for keyword in &keywords {
                let kw_lower = keyword.to_lowercase();
                if words.iter().any(|w| *w == kw_lower) {
                    return Some(agent.id());
                }
            }
        }
        None
    }

    /// Clear the cached engine for a specific agent (resets its memory).
    pub async fn clear_agent(&self, agent_id: &str) {
        let mut engines = self.engines.write().await;
        engines.remove(agent_id);
    }

    /// Clear all cached engines (resets all agent memory).
    pub async fn clear_all(&self) {
        let mut engines = self.engines.write().await;
        engines.clear();
    }

    /// Build a ChatEngine for the given agent based on its tool filter and config.
    fn create_engine(&self, agent: &dyn Agent) -> Arc<dyn ChatEngine> {
        let temperature = agent.temperature().unwrap_or(0.5);
        let max_tokens = agent.max_tokens().unwrap_or(1024);

        let chat_config = ChatConfig {
            model: self.config.model.clone(),
            temperature,
            max_tokens,
            system_prompt: Some(agent.system_prompt()),
            max_history: self.config.history_size,
            enable_summarization: false,
        };

        let tool_filter = agent.tool_filter();
        let effective_tools = self.effective_tool_config(&tool_filter);

        if effective_tools.enabled() {
            let rustratify_config = tool::ToolConfig {
                enable_fs: effective_tools.enable_fs,
                enable_exec: effective_tools.enable_exec,
                enable_web: effective_tools.enable_web,
                fs_max_size: effective_tools.fs_max_size,
                exec_timeout: effective_tools.exec_timeout,
            };

            let registry = tools::create_standard_registry(&rustratify_config);
            Arc::new(ToolAwareChatEngine::new(
                self.llm.clone(),
                chat_config,
                Arc::new(registry),
            ))
        } else {
            Arc::new(SimpleChatEngine::new(self.llm.clone(), chat_config))
        }
    }

    /// Combine the agent's tool filter with the global tool config.
    ///
    /// The agent's filter can only further restrict tools — it cannot
    /// enable tools that are disabled globally.
    fn effective_tool_config(&self, filter: &ToolFilter) -> ToolConfig {
        let global = &self.config.tools;
        match filter {
            ToolFilter::All => global.clone(),
            ToolFilter::None => ToolConfig {
                enable_fs: false,
                enable_exec: false,
                enable_web: false,
                ..global.clone()
            },
            ToolFilter::Only { fs, exec, web } => ToolConfig {
                enable_fs: global.enable_fs && *fs,
                enable_exec: global.enable_exec && *exec,
                enable_web: global.enable_web && *web,
                ..global.clone()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAgent {
        id: String,
        keywords: Vec<String>,
    }

    impl Agent for TestAgent {
        fn id(&self) -> &str {
            &self.id
        }
        fn display_name(&self) -> &str {
            "Test Agent"
        }
        fn description(&self) -> &str {
            "A test agent"
        }
        fn system_prompt(&self) -> String {
            "You are a test agent.".to_string()
        }
        fn trigger_keywords(&self) -> Vec<&str> {
            self.keywords.iter().map(|s| s.as_str()).collect()
        }
    }

    #[test]
    fn test_tool_filter_effective_config() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
        };

        // ToolFilter::None disables everything
        let none_config = AgentRegistry::new(
            Arc::new(MockLlm),
            config.clone(),
        );
        let effective = none_config.effective_tool_config(&ToolFilter::None);
        assert!(!effective.enable_fs);
        assert!(!effective.enable_exec);
        assert!(!effective.enable_web);

        // ToolFilter::Only restricts to selected categories
        let filter = ToolFilter::Only {
            fs: true,
            exec: false,
            web: false,
        };
        let effective = none_config.effective_tool_config(&filter);
        assert!(effective.enable_fs);
        assert!(!effective.enable_exec);
        assert!(!effective.enable_web);
    }

    #[test]
    fn test_detect_agent() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
        };

        let mut registry = AgentRegistry::new(Arc::new(MockLlm), config);
        registry.register(Box::new(TestAgent {
            id: "git".into(),
            keywords: vec!["git".into(), "commit".into(), "branch".into()],
        }));
        registry.register(Box::new(TestAgent {
            id: "devops".into(),
            keywords: vec!["docker".into(), "k8s".into()],
        }));

        assert_eq!(registry.detect_agent("git commit -m fix"), Some("git"));
        assert_eq!(registry.detect_agent("docker ps"), Some("devops"));
        assert_eq!(registry.detect_agent("list files"), None);
    }

    #[test]
    fn test_register_and_list() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
        };

        let mut registry = AgentRegistry::new(Arc::new(MockLlm), config);
        registry.register(Box::new(TestAgent {
            id: "alpha".into(),
            keywords: vec![],
        }));
        registry.register(Box::new(TestAgent {
            id: "beta".into(),
            keywords: vec![],
        }));

        let agents = registry.list();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].id(), "alpha");
        assert_eq!(agents[1].id(), "beta");
    }

    /// Minimal mock LLM for unit tests (never actually called).
    struct MockLlm;

    #[async_trait::async_trait]
    impl LlmService for MockLlm {
        async fn providers(&self) -> Vec<String> {
            vec!["mock".into()]
        }

        async fn complete(
            &self,
            _request: llm_provider::CompletionRequest,
        ) -> Result<llm_provider::CompletionResponse, llm_provider::LlmError> {
            Ok(llm_provider::CompletionResponse {
                id: "mock-id".into(),
                content: Some("mock".into()),
                model: "mock".into(),
                tool_calls: vec![],
                usage: llm_provider::TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                },
                finish_reason: llm_provider::FinishReason::Stop,
            })
        }

        async fn complete_stream(
            &self,
            _request: llm_provider::CompletionRequest,
        ) -> Result<
            std::pin::Pin<
                Box<dyn futures::Stream<Item = Result<llm_provider::StreamChunk, llm_provider::LlmError>> + Send>,
            >,
            llm_provider::LlmError,
        > {
            Ok(Box::pin(futures::stream::empty()))
        }

        async fn list_models(&self) -> Result<Vec<llm_provider::ModelInfo>, llm_provider::LlmError> {
            Ok(vec![])
        }

        async fn model_info(&self, _model: &str) -> Result<llm_provider::ModelInfo, llm_provider::LlmError> {
            Err(llm_provider::LlmError::Configuration("mock".into()))
        }

        async fn is_model_available(&self, _model: &str) -> bool {
            false
        }
    }
}
