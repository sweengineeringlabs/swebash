/// Built-in agent definitions loaded from YAML.
///
/// Default agents are embedded in the binary via `include_str!()`.
/// Users can add or override agents via a config file at:
///   - `$SWEBASH_AGENTS_CONFIG`
///   - `~/.config/swebash/agents.yaml`
///   - `~/.swebash/agents.yaml`
use std::path::PathBuf;
use std::sync::Arc;

use llm_provider::LlmService;

use crate::spi::config::AiConfig;
use crate::core::rag::index::RagIndexManager;
use crate::core::rag::stores::VectorStoreConfig;

use super::config::{AgentsYaml, ConfigAgent, RagYamlConfig};
use super::AgentManager;

/// Embedded default agents YAML, compiled into the binary.
const DEFAULT_AGENTS_YAML: &str = include_str!("default_agents.yaml");

/// Return the number of agents defined in the embedded default YAML.
///
/// Useful in tests to avoid hardcoding a count that breaks whenever a
/// new agent is added to `default_agents.yaml`.
pub fn builtin_agent_count() -> usize {
    AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML)
        .expect("embedded YAML must parse")
        .agents
        .len()
}

/// Parsed YAML source with its base directory for agent loading.
struct YamlSource {
    parsed: AgentsYaml,
    base_dir: Option<PathBuf>,
}

/// Create the default agent registry with all built-in agents.
///
/// Loads agents in three layers (later layers override earlier ones):
/// 1. Embedded defaults from `default_agents.yaml`
/// 2. Project-local `.swebash/agents.yaml` in `docs_base_dir` (if present)
/// 3. User-level config file (whole-agent replacement by ID)
///
/// RAG configuration from YAML files is merged with env-based config.
/// YAML provides defaults; env vars override when set.
pub fn create_default_registry(llm: Arc<dyn LlmService>, mut config: AiConfig) -> AgentManager {
    let docs_base_dir = config.docs_base_dir.clone();

    // Phase 1: Parse all YAML sources and collect RAG configs.
    let mut yaml_sources = Vec::new();
    let mut yaml_rag_config: Option<RagYamlConfig> = None;

    // 1. Embedded defaults
    if let Ok(parsed) = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML) {
        if let Some(ref rag) = parsed.rag {
            yaml_rag_config = Some(rag.clone());
        }
        yaml_sources.push(YamlSource {
            parsed,
            base_dir: docs_base_dir.clone(),
        });
    }

    // 2. Project-local config
    if let Some(ref base) = docs_base_dir {
        let project_config = base.join(".swebash").join("agents.yaml");
        if project_config.is_file() {
            if let Ok(contents) = std::fs::read_to_string(&project_config) {
                tracing::info!("Loading project agents from {}", project_config.display());
                if let Ok(parsed) = AgentsYaml::from_yaml(&contents) {
                    if let Some(ref rag) = parsed.rag {
                        yaml_rag_config = Some(rag.clone());
                    }
                    yaml_sources.push(YamlSource {
                        parsed,
                        base_dir: Some(base.clone()),
                    });
                } else {
                    tracing::warn!("Failed to parse project agents file {}", project_config.display());
                }
            }
        }
    }

    // 3. User config
    if let Some(path) = find_user_agents_config() {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            tracing::info!("Loading user agents from {}", path.display());
            if let Ok(parsed) = AgentsYaml::from_yaml(&contents) {
                if let Some(ref rag) = parsed.rag {
                    yaml_rag_config = Some(rag.clone());
                }
                yaml_sources.push(YamlSource {
                    parsed,
                    base_dir: path.parent().map(|p| p.to_path_buf()),
                });
            } else {
                tracing::warn!("Failed to parse user agents file {}", path.display());
            }
        }
    }

    // Phase 2: Merge YAML RAG config with env-based config.
    // YAML provides defaults; env vars override.
    if let Some(yaml_rag) = yaml_rag_config {
        // Only apply YAML config if env vars weren't explicitly set.
        // We detect this by checking if config still has default values.
        let env_store_set = std::env::var("SWEBASH_AI_RAG_STORE").is_ok();
        let env_path_set = std::env::var("SWEBASH_AI_RAG_STORE_PATH").is_ok();
        let env_chunk_size_set = std::env::var("SWEBASH_AI_RAG_CHUNK_SIZE").is_ok();
        let env_chunk_overlap_set = std::env::var("SWEBASH_AI_RAG_CHUNK_OVERLAP").is_ok();

        if !env_store_set || !env_path_set {
            let store = VectorStoreConfig::from_yaml(&yaml_rag.store, yaml_rag.path.clone());
            if !env_store_set {
                config.rag.vector_store = store;
            }
        }
        if !env_chunk_size_set {
            config.rag.chunk_size = yaml_rag.chunk_size;
        }
        if !env_chunk_overlap_set {
            config.rag.chunk_overlap = yaml_rag.chunk_overlap;
        }

        tracing::debug!(
            store = ?config.rag.vector_store,
            chunk_size = config.rag.chunk_size,
            chunk_overlap = config.rag.chunk_overlap,
            "RAG config merged from YAML"
        );
    }

    // Phase 3: Create RAG manager with final config.
    let rag_manager = create_rag_manager(&config);
    let rag_available = rag_manager.is_some();
    let mut manager = AgentManager::new(llm, config, rag_manager);

    // Phase 4: Register agents from all YAML sources.
    for source in yaml_sources {
        let defaults = source.parsed.defaults;
        for entry in source.parsed.agents {
            let agent = ConfigAgent::from_entry_with_base_dir(
                entry,
                &defaults,
                source.base_dir.as_deref(),
                rag_available,
            );
            manager.register(agent);
        }
    }

    manager
}

/// Create a RAG index manager with the configured embedding provider and vector store.
///
/// Returns `Some` when a local embedding provider is available (the `rag-local` feature),
/// `None` otherwise. When `None`, agents configured with `strategy: rag` will fall back
/// to preload behavior.
///
/// The vector store backend is selected via `config.rag.vector_store`:
/// - `Memory` (default): ephemeral in-memory store
/// - `File { path }`: JSON file persistence
/// - `Sqlite { path }`: SQLite database (requires `rag-sqlite` feature)
fn create_rag_manager(config: &AiConfig) -> Option<Arc<RagIndexManager>> {
    #[cfg(feature = "rag-local")]
    {
        use crate::core::rag::chunker::ChunkerConfig;
        use crate::core::rag::embeddings::FastEmbedProvider;

        match FastEmbedProvider::new() {
            Ok(embedder) => {
                let store = match config.rag.vector_store.build() {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to build vector store, RAG disabled");
                        return None;
                    }
                };
                let chunker_config = ChunkerConfig {
                    chunk_size: config.rag.chunk_size,
                    overlap: config.rag.chunk_overlap,
                };
                let manager = RagIndexManager::new(
                    Arc::new(embedder),
                    store,
                    chunker_config,
                );
                tracing::info!(
                    store = ?config.rag.vector_store,
                    chunk_size = config.rag.chunk_size,
                    chunk_overlap = config.rag.chunk_overlap,
                    "RAG index manager initialized"
                );
                Some(Arc::new(manager))
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to initialize FastEmbed, RAG disabled");
                None
            }
        }
    }

    #[cfg(not(feature = "rag-local"))]
    {
        let _ = config; // suppress unused warning
        tracing::debug!("rag-local feature not enabled, RAG disabled");
        None
    }
}

/// Parse a YAML string and register all agents into the manager.
///
/// When `base_dir` is `Some`, any agent with a `docs` section will have
/// its file sources loaded relative to that directory and prepended to
/// the system prompt.
///
/// On parse failure, logs a warning and continues with whatever agents
/// are already registered.
///
/// This function is exposed for tests that need to load agents from YAML.
pub(crate) fn register_from_yaml(
    manager: &mut AgentManager,
    yaml: &str,
    source: &str,
    base_dir: Option<&std::path::Path>,
) {
    match AgentsYaml::from_yaml(yaml) {
        Ok(parsed) => {
            let defaults = parsed.defaults;
            for entry in parsed.agents {
                let agent =
                    ConfigAgent::from_entry_with_base_dir(entry, &defaults, base_dir, false);
                manager.register(agent);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to parse agents from {source}: {e}");
        }
    }
}

/// Search for a user agents config file in standard locations.
///
/// Priority order:
/// 1. `$SWEBASH_AGENTS_CONFIG` environment variable (exclusive — never falls through)
/// 2. `~/.config/swebash/agents.yaml`
/// 3. `~/.swebash/agents.yaml`
fn find_user_agents_config() -> Option<PathBuf> {
    // 1. Explicit env var — when set, use it exclusively.
    //    If the path doesn't exist, return None (don't fall through to defaults).
    //    This lets callers disable user config by pointing at a nonexistent path.
    if let Ok(path) = std::env::var("SWEBASH_AGENTS_CONFIG") {
        let p = PathBuf::from(path);
        return if p.is_file() { Some(p) } else { None };
    }

    // 2-3. Standard locations under home directory
    if let Some(home) = home_dir() {
        let candidates = [
            home.join(".config").join("swebash").join("agents.yaml"),
            home.join(".swebash").join("agents.yaml"),
        ];
        for candidate in &candidates {
            if candidate.is_file() {
                return Some(candidate.clone());
            }
        }
    }

    None
}

/// Get the user's home directory.
fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{AgentDescriptor, ToolFilter};
    use llm_provider::MockLlmService;

    #[test]
    fn test_embedded_yaml_parses() {
        let parsed = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML)
            .expect("Embedded default_agents.yaml should parse");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.agents.len(), builtin_agent_count());
    }

    #[test]
    fn test_shell_agent() {
        let parsed = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML).unwrap();
        let defaults = parsed.defaults;
        let entry = parsed.agents.into_iter().find(|a| a.id == "shell").unwrap();
        let agent = ConfigAgent::from_entry(entry, &defaults);

        assert_eq!(agent.id(), "shell");
        assert_eq!(agent.display_name(), "Shell Assistant");
        assert!(agent.trigger_keywords().is_empty());
        assert!(matches!(agent.tool_filter(), ToolFilter::All));
    }

    #[test]
    fn test_review_agent() {
        let parsed = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML).unwrap();
        let defaults = parsed.defaults;
        let entry = parsed.agents.into_iter().find(|a| a.id == "review").unwrap();
        let agent = ConfigAgent::from_entry(entry, &defaults);

        assert_eq!(agent.id(), "review");
        assert!(agent.trigger_keywords().contains(&"review".to_string()));
        match agent.tool_filter() {
            ToolFilter::Categories(cats) => {
                assert!(cats.contains(&"fs".to_string()));
                assert!(!cats.contains(&"exec".to_string()));
                assert!(!cats.contains(&"web".to_string()));
            }
            _ => panic!("Expected ToolFilter::Categories"),
        }
    }

    #[test]
    fn test_devops_agent() {
        let parsed = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML).unwrap();
        let defaults = parsed.defaults;
        let entry = parsed.agents.into_iter().find(|a| a.id == "devops").unwrap();
        let agent = ConfigAgent::from_entry(entry, &defaults);

        assert_eq!(agent.id(), "devops");
        assert!(agent.trigger_keywords().contains(&"docker".to_string()));
        assert!(agent.trigger_keywords().contains(&"k8s".to_string()));
        assert!(matches!(agent.tool_filter(), ToolFilter::All));
    }

    #[test]
    fn test_git_agent() {
        let parsed = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML).unwrap();
        let defaults = parsed.defaults;
        let entry = parsed.agents.into_iter().find(|a| a.id == "git").unwrap();
        let agent = ConfigAgent::from_entry(entry, &defaults);

        assert_eq!(agent.id(), "git");
        assert!(agent.trigger_keywords().contains(&"git".to_string()));
        assert!(agent.trigger_keywords().contains(&"commit".to_string()));
        match agent.tool_filter() {
            ToolFilter::Categories(cats) => {
                assert!(cats.contains(&"fs".to_string()));
                assert!(cats.contains(&"exec".to_string()));
                assert!(!cats.contains(&"web".to_string()));
            }
            _ => panic!("Expected ToolFilter::Categories"),
        }
    }

    #[test]
    fn test_user_override_replaces_agent() {
        let user_yaml = r#"
version: 1
agents:
  - id: shell
    name: Custom Shell
    description: My custom shell agent
    systemPrompt: Custom prompt.
    tools:
      fs: true
      exec: false
      web: false
"#;

        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: crate::spi::config::ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        register_from_yaml(&mut manager, DEFAULT_AGENTS_YAML, "defaults", None);
        register_from_yaml(&mut manager, user_yaml, "user", None);

        // User override should replace the default shell agent
        let shell = manager.get("shell").unwrap();
        assert_eq!(shell.display_name(), "Custom Shell");
        assert_eq!(shell.description(), "My custom shell agent");

        // Other agents should still exist
        assert!(manager.get("review").is_some());
        assert!(manager.get("devops").is_some());
        assert!(manager.get("git").is_some());
    }

    #[test]
    fn test_user_adds_new_agent() {
        let user_yaml = r#"
version: 1
agents:
  - id: security
    name: Security Scanner
    description: Scans for vulnerabilities
    systemPrompt: You are a security scanner.
    triggerKeywords: [security, scan, vuln]
    tools:
      fs: true
      exec: true
      web: true
"#;

        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: crate::spi::config::ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        register_from_yaml(&mut manager, DEFAULT_AGENTS_YAML, "defaults", None);
        register_from_yaml(&mut manager, user_yaml, "user", None);

        // New agent should be added alongside defaults
        assert_eq!(manager.list().len(), builtin_agent_count() + 1);
        let security = manager.get("security").unwrap();
        assert_eq!(security.display_name(), "Security Scanner");
    }

    #[test]
    fn test_invalid_user_yaml_is_ignored() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            model: "gpt-4o".into(),
            history_size: 20,
            default_agent: "shell".into(),
            agent_auto_detect: true,
            tools: crate::spi::config::ToolConfig::default(),
            log_dir: None,
            docs_base_dir: None,
            rag: crate::spi::config::RagConfig::default(),
        };

        let mut manager = AgentManager::new(Arc::new(MockLlmService::new()), config, None);
        register_from_yaml(&mut manager, DEFAULT_AGENTS_YAML, "defaults", None);
        register_from_yaml(&mut manager, "not: valid: yaml: [", "bad-user-file", None);

        // Defaults should still be present
        assert_eq!(manager.list().len(), builtin_agent_count());
    }
}
