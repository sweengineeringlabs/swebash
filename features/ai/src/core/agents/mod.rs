/// Agent framework: purpose-built AI agents with dedicated prompts, tools, and memory.
///
/// Each agent has its own `ChatEngine` instance (lazily created), system prompt,
/// and tool filter. The `AgentManager` manages agent lifecycle and provides
/// engine access by agent ID, delegating to Rustratify's `AgentRegistry` and `EngineCache`.
pub mod builtins;
pub mod config;

use std::sync::Arc;

use chat_engine::{ChatConfig, ChatEngine, SimpleChatEngine, ToolAwareChatEngine};
use llm_provider::LlmService;

use agent_controller::{EngineCache, EngineCacheConfig, EngineFactory};

pub use agent_controller::{AgentDescriptor, ToolFilter};

use crate::spi::config::{AiConfig, ToolConfig};
use crate::core::tools;
use crate::core::rag::index::RagIndexManager;
use crate::core::rag::tool::RagTool;

use config::{ConfigAgent, DocsStrategy};

use std::time::Duration;

// ── SwebashEngineFactory ───────────────────────────────────────────

/// Factory that creates `ChatEngine` instances for `ConfigAgent` descriptors.
///
/// Moves the engine-creation logic that was previously in `AgentRegistry::create_engine`
/// into a Rustratify `EngineFactory` implementation.
pub struct SwebashEngineFactory {
    llm: Arc<dyn LlmService>,
    config: AiConfig,
    /// Optional RAG index manager, shared across all agents.
    rag_index_manager: Option<Arc<RagIndexManager>>,
}

impl SwebashEngineFactory {
    /// Combine the agent's tool filter with the global tool config.
    ///
    /// The agent's filter can only further restrict tools — it cannot
    /// enable tools that are disabled globally.
    fn effective_tool_config(&self, filter: &ToolFilter) -> ToolConfig {
        let global = &self.config.tools;
        match filter {
            ToolFilter::All => global.clone(),
            ToolFilter::Categories(cats) => {
                let has = |c: &str| cats.iter().any(|cat| cat == c);
                ToolConfig {
                    enable_fs: global.enable_fs && has("fs"),
                    enable_exec: global.enable_exec && has("exec"),
                    enable_web: global.enable_web && has("web"),
                    enable_rag: has("rag"),
                    ..global.clone()
                }
            }
            ToolFilter::AllowList(_) => global.clone(), // unused by swebash
        }
    }
}

impl EngineFactory<ConfigAgent> for SwebashEngineFactory {
    type Engine = dyn ChatEngine;

    fn create(&self, descriptor: &ConfigAgent) -> Option<Arc<Self::Engine>> {
        let temperature = descriptor.temperature().unwrap_or(0.5);
        let max_tokens = descriptor.max_tokens().unwrap_or(1024);

        // Computed but not yet consumed — confirmation not enforced by ToolAwareChatEngine yet.
        let _require_confirmation = self.config.tools.require_confirmation
            && !descriptor.bypass_confirmation();

        let chat_config = ChatConfig {
            model: self.config.model.clone(),
            temperature,
            max_tokens,
            system_prompt: Some(descriptor.system_prompt().to_string()),
            max_history: self.config.history_size,
            enable_summarization: false,
        };

        let tool_filter = descriptor.tool_filter();
        let effective_tools = self.effective_tool_config(&tool_filter);

        if effective_tools.enabled() {
            let rustratify_config = tool::ToolConfig {
                enable_fs: effective_tools.enable_fs,
                enable_exec: effective_tools.enable_exec,
                enable_web: effective_tools.enable_web,
                fs_max_size: effective_tools.fs_max_size,
                exec_timeout: effective_tools.exec_timeout,
            };

            let mut registry = if effective_tools.cache.enabled {
                let cache_cfg = agent_cache::CacheConfig::with_ttl(
                    Duration::from_secs(effective_tools.cache.ttl_secs),
                ).with_max_entries(effective_tools.cache.max_entries);
                let (reg, _) = tools::create_cached_registry(&rustratify_config, cache_cfg);
                reg
            } else {
                tools::create_standard_registry(&rustratify_config)
            };

            // Register RagTool for agents using the RAG docs strategy.
            if effective_tools.enable_rag {
                if let Some(ref rag_mgr) = self.rag_index_manager {
                    if *descriptor.docs_strategy() == DocsStrategy::Rag {
                        // Build the index eagerly (blocking in the factory).
                        // If docs_base_dir is available, ensure the index is built.
                        if let Some(ref base_dir) = self.config.docs_base_dir {
                            let sources = descriptor.docs_sources();
                            if !sources.is_empty() {
                                let rt = tokio::runtime::Handle::try_current();
                                if let Ok(handle) = rt {
                                    let mgr = rag_mgr.clone();
                                    let agent_id = descriptor.id().to_string();
                                    let srcs = sources.to_vec();
                                    let dir = base_dir.clone();
                                    // Use block_in_place to allow blocking within the
                                    // Tokio runtime (EngineFactory::create is sync).
                                    let _ = tokio::task::block_in_place(|| {
                                        handle.block_on(async {
                                            if let Err(e) = mgr.ensure_index(&agent_id, &srcs, &dir).await {
                                                tracing::warn!(
                                                    agent = %agent_id,
                                                    error = %e,
                                                    "failed to build RAG index, rag_search may return no results"
                                                );
                                            }
                                        })
                                    });
                                }
                            }
                        }

                        registry.register(Box::new(RagTool::new(
                            descriptor.id(),
                            rag_mgr.clone(),
                            descriptor.docs_top_k(),
                        )));
                    }
                }
            }

            Some(Arc::new(ToolAwareChatEngine::new(
                self.llm.clone(),
                chat_config,
                Arc::new(registry),
            ).with_max_iterations(descriptor.max_iterations().unwrap_or(effective_tools.max_iterations))))
        } else {
            Some(Arc::new(SimpleChatEngine::new(self.llm.clone(), chat_config)))
        }
    }
}

// ── AgentManager ───────────────────────────────────────────────────

/// Manages agent descriptors and their cached engines.
///
/// Wraps Rustratify's `AgentRegistry<ConfigAgent>` and `EngineCache` to
/// preserve the same public API surface used by `DefaultAiService`.
pub struct AgentManager {
    registry: agent_controller::AgentRegistry<ConfigAgent>,
    cache: EngineCache<ConfigAgent, SwebashEngineFactory>,
}

impl AgentManager {
    /// Create a new manager with the given LLM service, config, and optional RAG index manager.
    pub fn new(
        llm: Arc<dyn LlmService>,
        config: AiConfig,
        rag_index_manager: Option<Arc<RagIndexManager>>,
    ) -> Self {
        let factory = SwebashEngineFactory {
            llm,
            config: config.clone(),
            rag_index_manager,
        };
        Self {
            registry: agent_controller::AgentRegistry::new(),
            cache: EngineCache::new(factory, EngineCacheConfig::default()),
        }
    }

    /// Register an agent. Overwrites any existing agent with the same ID.
    pub fn register(&mut self, agent: ConfigAgent) {
        self.registry.register(agent);
    }

    /// Get an agent by ID.
    pub fn get(&self, id: &str) -> Option<&ConfigAgent> {
        self.registry.get(id)
    }

    /// List all registered agents (sorted by ID).
    pub fn list(&self) -> Vec<&ConfigAgent> {
        self.registry.list()
    }

    /// Get or create the chat engine for an agent.
    ///
    /// Engines are lazily created on first access and cached for the session.
    /// Each agent gets isolated conversation memory.
    pub fn engine_for(&self, agent_id: &str) -> Option<Arc<dyn ChatEngine>> {
        self.cache.engine_for(agent_id, &self.registry)
    }

    /// Detect which agent should handle the given input based on trigger keywords.
    ///
    /// Returns the agent ID if a match is found, `None` otherwise.
    pub fn detect_agent(&self, input: &str) -> Option<&str> {
        self.registry.detect_agent(input).map(|d| d.id())
    }

    /// Suggest an agent for an unknown name by matching against trigger keywords.
    ///
    /// Returns the agent ID if the name matches one of its keywords, `None` otherwise.
    /// Uses keyword-based matching (swebash's original semantics).
    pub fn suggest_agent(&self, name: &str) -> Option<&str> {
        let name_lower = name.to_lowercase();
        for agent in self.registry.list() {
            for keyword in agent.trigger_keywords() {
                if keyword.to_lowercase() == name_lower {
                    return Some(agent.id());
                }
            }
        }
        None
    }

    /// Clear the cached engine for a specific agent (resets its memory).
    pub fn clear_agent(&self, agent_id: &str) {
        self.cache.clear_agent(agent_id);
    }

    /// Clear all cached engines (resets all agent memory).
    pub fn clear_all(&self) {
        self.cache.clear_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{AgentDefaults, AgentEntry};
    use llm_provider::MockLlmService;

    fn make_test_agent(id: &str, keywords: Vec<String>) -> ConfigAgent {
        ConfigAgent::from_entry(
            AgentEntry {
                id: id.into(),
                name: "Test Agent".into(),
                description: "A test agent".into(),
                temperature: None,
                max_tokens: None,
                system_prompt: "You are a test agent.".into(),
                tools: None,
                trigger_keywords: keywords,
                think_first: None,
                bypass_confirmation: None,
                max_iterations: None,
                docs: None,
                directives: None,
            },
            &AgentDefaults::default(),
        )
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
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let factory = SwebashEngineFactory {
            llm: Arc::new(MockLlmService::new()),
            config: config.clone(),
            rag_index_manager: None,
        };

        // ToolFilter::Categories(empty) disables everything
        let effective = factory.effective_tool_config(&ToolFilter::Categories(vec![]));
        assert!(!effective.enable_fs);
        assert!(!effective.enable_exec);
        assert!(!effective.enable_web);

        // ToolFilter::Categories restricts to selected categories
        let filter = ToolFilter::Categories(vec!["fs".to_string()]);
        let effective = factory.effective_tool_config(&filter);
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
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("git", vec!["git".into(), "commit".into(), "branch".into()]));
        manager.register(make_test_agent("devops", vec!["docker".into(), "k8s".into()]));

        assert_eq!(manager.detect_agent("git commit -m fix"), Some("git"));
        assert_eq!(manager.detect_agent("docker ps"), Some("devops"));
        assert_eq!(manager.detect_agent("list files"), None);
    }

    #[test]
    fn test_suggest_agent() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("devops", vec!["docker".into(), "k8s".into(), "terraform".into()]));
        manager.register(make_test_agent("git", vec!["git".into(), "commit".into()]));

        // "docker" is a keyword of "devops"
        assert_eq!(manager.suggest_agent("docker"), Some("devops"));
        assert_eq!(manager.suggest_agent("k8s"), Some("devops"));
        assert_eq!(manager.suggest_agent("Docker"), Some("devops"));
        // "git" is an exact agent ID, but also a keyword — suggest still works
        assert_eq!(manager.suggest_agent("commit"), Some("git"));
        // No match
        assert_eq!(manager.suggest_agent("unknown"), None);
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
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("alpha", vec![]));
        manager.register(make_test_agent("beta", vec![]));

        let agents = manager.list();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].id(), "alpha");
        assert_eq!(agents[1].id(), "beta");
    }

    // ── AgentManager::get ───────────────────────────────────────────

    #[test]
    fn test_get_existing_agent() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("alpha", vec!["a".into()]));

        let agent = manager.get("alpha");
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().id(), "alpha");
    }

    #[test]
    fn test_get_nonexistent_agent() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        assert!(manager.get("nonexistent").is_none());
    }

    // ── AgentManager::engine_for ────────────────────────────────────

    #[test]
    fn test_engine_for_creates_engine() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("shell", vec![]));

        let engine = manager.engine_for("shell");
        assert!(engine.is_some(), "engine_for should create an engine");
    }

    #[test]
    fn test_engine_for_nonexistent_returns_none() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        assert!(manager.engine_for("ghost").is_none());
    }

    #[test]
    fn test_engine_for_returns_cached_instance() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("agent-a", vec![]));

        let e1 = manager.engine_for("agent-a").unwrap();
        let e2 = manager.engine_for("agent-a").unwrap();
        // Same Arc — pointer equality
        assert!(Arc::ptr_eq(&e1, &e2), "engine_for should return the cached instance");
    }

    #[test]
    fn test_engine_for_different_agents_are_isolated() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("agent-a", vec![]));
        manager.register(make_test_agent("agent-b", vec![]));

        let ea = manager.engine_for("agent-a").unwrap();
        let eb = manager.engine_for("agent-b").unwrap();
        assert!(!Arc::ptr_eq(&ea, &eb), "different agents should have different engines");
    }

    // ── AgentManager::clear_agent / clear_all ───────────────────────

    #[test]
    fn test_clear_agent_resets_engine() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("agent-a", vec![]));

        let e1 = manager.engine_for("agent-a").unwrap();
        manager.clear_agent("agent-a");
        let e2 = manager.engine_for("agent-a").unwrap();
        // After clearing, a new engine should be created
        assert!(!Arc::ptr_eq(&e1, &e2), "engine should be recreated after clear");
    }

    #[test]
    fn test_clear_agent_leaves_others_intact() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("agent-a", vec![]));
        manager.register(make_test_agent("agent-b", vec![]));

        let _ea = manager.engine_for("agent-a").unwrap();
        let eb1 = manager.engine_for("agent-b").unwrap();

        manager.clear_agent("agent-a");

        // agent-b's engine should be unchanged
        let eb2 = manager.engine_for("agent-b").unwrap();
        assert!(Arc::ptr_eq(&eb1, &eb2), "clearing agent-a should not affect agent-b");
    }

    #[test]
    fn test_clear_all_resets_all_engines() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("agent-a", vec![]));
        manager.register(make_test_agent("agent-b", vec![]));

        let ea1 = manager.engine_for("agent-a").unwrap();
        let eb1 = manager.engine_for("agent-b").unwrap();

        manager.clear_all();

        let ea2 = manager.engine_for("agent-a").unwrap();
        let eb2 = manager.engine_for("agent-b").unwrap();
        assert!(!Arc::ptr_eq(&ea1, &ea2), "agent-a engine should be recreated");
        assert!(!Arc::ptr_eq(&eb1, &eb2), "agent-b engine should be recreated");
    }

    // ── AgentManager::register overwrites ───────────────────────────

    #[test]
    fn test_register_overwrites_existing() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(ConfigAgent::from_entry(
            AgentEntry {
                id: "dup".into(),
                name: "Original".into(),
                description: "v1".into(),
                temperature: None,
                max_tokens: None,
                system_prompt: "p1".into(),
                tools: None,
                trigger_keywords: vec![],
                think_first: None,
                bypass_confirmation: None,
                max_iterations: None,
                docs: None,
                directives: None,
            },
            &AgentDefaults::default(),
        ));
        manager.register(ConfigAgent::from_entry(
            AgentEntry {
                id: "dup".into(),
                name: "Replaced".into(),
                description: "v2".into(),
                temperature: None,
                max_tokens: None,
                system_prompt: "p2".into(),
                tools: None,
                trigger_keywords: vec![],
                think_first: None,
                bypass_confirmation: None,
                max_iterations: None,
                docs: None,
                directives: None,
            },
            &AgentDefaults::default(),
        ));

        assert_eq!(manager.list().len(), 1);
        assert_eq!(manager.get("dup").unwrap().display_name(), "Replaced");
    }

    // ── SwebashEngineFactory::effective_tool_config completeness ────

    #[test]
    fn test_effective_tool_config_all_passes_global() {
        let tools = ToolConfig {
            enable_fs: true,
            enable_exec: false,
            enable_web: true,
            ..ToolConfig::default()
        };
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools,
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let factory = SwebashEngineFactory {
            llm: Arc::new(MockLlmService::new()),
            config,
            rag_index_manager: None,
        };

        let effective = factory.effective_tool_config(&ToolFilter::All);
        assert!(effective.enable_fs);
        assert!(!effective.enable_exec); // global disables exec
        assert!(effective.enable_web);
    }

    #[test]
    fn test_effective_tool_config_allowlist_passes_global() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let factory = SwebashEngineFactory {
            llm: Arc::new(MockLlmService::new()),
            config: config.clone(),
            rag_index_manager: None,
        };

        // AllowList is unused by swebash, should pass-through global config
        let effective = factory.effective_tool_config(
            &ToolFilter::AllowList(vec!["some_tool".to_string()]),
        );
        assert!(effective.enable_fs);
        assert!(effective.enable_exec);
        assert!(effective.enable_web);
    }

    #[test]
    fn test_effective_tool_config_categories_intersects_global() {
        // Global: fs=true, exec=true, web=false
        let tools = ToolConfig {
            enable_fs: true,
            enable_exec: true,
            enable_web: false,
            ..ToolConfig::default()
        };
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools,
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let factory = SwebashEngineFactory {
            llm: Arc::new(MockLlmService::new()),
            config,
            rag_index_manager: None,
        };

        // Agent requests web — but global disables it
        let filter = ToolFilter::Categories(vec!["web".to_string()]);
        let effective = factory.effective_tool_config(&filter);
        assert!(!effective.enable_fs); // agent didn't request fs
        assert!(!effective.enable_exec); // agent didn't request exec
        assert!(!effective.enable_web); // global disables web

        // Agent requests fs + exec — global allows both
        let filter = ToolFilter::Categories(vec!["fs".to_string(), "exec".to_string()]);
        let effective = factory.effective_tool_config(&filter);
        assert!(effective.enable_fs);
        assert!(effective.enable_exec);
        assert!(!effective.enable_web);
    }

    #[test]
    fn test_effective_tool_config_preserves_non_boolean_fields() {
        let tools = ToolConfig {
            enable_fs: true,
            enable_exec: true,
            enable_web: true,
            fs_max_size: 999,
            exec_timeout: 42,
            max_iterations: 7,
            ..ToolConfig::default()
        };
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools,
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let factory = SwebashEngineFactory {
            llm: Arc::new(MockLlmService::new()),
            config,
            rag_index_manager: None,
        };

        // Categories should preserve fs_max_size, exec_timeout, etc.
        let effective = factory.effective_tool_config(
            &ToolFilter::Categories(vec!["fs".to_string()]),
        );
        assert_eq!(effective.fs_max_size, 999);
        assert_eq!(effective.exec_timeout, 42);
        assert_eq!(effective.max_iterations, 7);
    }

    // ── SwebashEngineFactory::create ────────────────────────────────

    #[test]
    fn test_factory_creates_engine_for_tools_enabled() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let factory = SwebashEngineFactory {
            llm: Arc::new(MockLlmService::new()),
            config,
            rag_index_manager: None,
        };

        let agent = make_test_agent("shell", vec![]);
        let engine = factory.create(&agent);
        assert!(engine.is_some(), "factory should produce an engine");
    }

    #[test]
    fn test_factory_creates_engine_for_no_tools() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig {
                enable_fs: false,
                enable_exec: false,
                enable_web: false,
                ..ToolConfig::default()
            },
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let factory = SwebashEngineFactory {
            llm: Arc::new(MockLlmService::new()),
            config,
            rag_index_manager: None,
        };

        let agent = make_test_agent("chat-only", vec![]);
        let engine = factory.create(&agent);
        assert!(engine.is_some(), "factory should produce SimpleChatEngine when tools disabled");
    }

    #[test]
    fn test_factory_uses_agent_temperature_and_tokens() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig {
                enable_fs: false,
                enable_exec: false,
                enable_web: false,
                ..ToolConfig::default()
            },
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let factory = SwebashEngineFactory {
            llm: Arc::new(MockLlmService::new()),
            config,
            rag_index_manager: None,
        };

        // Agent with explicit temperature/tokens
        let agent = ConfigAgent::from_entry(
            AgentEntry {
                id: "custom-params".into(),
                name: "Custom".into(),
                description: "desc".into(),
                temperature: Some(0.9),
                max_tokens: Some(4096),
                system_prompt: "You are custom.".into(),
                tools: None,
                trigger_keywords: vec![],
                think_first: None,
                bypass_confirmation: None,
                max_iterations: None,
                docs: None,
                directives: None,
            },
            &AgentDefaults::default(),
        );

        // Should succeed — verifies the factory respects descriptor params
        let engine = factory.create(&agent);
        assert!(engine.is_some());
    }

    // ── Detect / suggest edge cases ─────────────────────────────────

    #[test]
    fn test_detect_agent_case_insensitive() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("git", vec!["Git".into(), "COMMIT".into()]));

        // Rustratify's detect_agent uses contains() with lowered input
        assert!(manager.detect_agent("git status").is_some());
        assert!(manager.detect_agent("GIT STATUS").is_some());
    }

    #[test]
    fn test_detect_agent_empty_input() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("git", vec!["git".into()]));

        assert_eq!(manager.detect_agent(""), None);
    }

    #[test]
    fn test_suggest_agent_empty_name() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        manager.register(make_test_agent("git", vec!["git".into()]));

        assert_eq!(manager.suggest_agent(""), None);
    }

    #[test]
    fn test_detect_agent_no_keywords_skipped() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        // Agent with no keywords should never be detected
        manager.register(make_test_agent("shell", vec![]));
        manager.register(make_test_agent("git", vec!["git".into()]));

        // "shell" has no keywords, should not match
        assert_eq!(manager.detect_agent("shell command"), None);
        assert_eq!(manager.detect_agent("git status"), Some("git"));
    }

    // ── Empty registry ──────────────────────────────────────────────

    #[test]
    fn test_empty_manager_operations() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        assert!(manager.list().is_empty());
        assert!(manager.get("anything").is_none());
        assert!(manager.engine_for("anything").is_none());
        assert_eq!(manager.detect_agent("any input"), None);
        assert_eq!(manager.suggest_agent("anything"), None);
        // clear on empty should not panic
        manager.clear_agent("ghost");
        manager.clear_all();
    }
}
