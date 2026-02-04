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

use crate::config::AiConfig;

use super::config::{AgentsYaml, ConfigAgent};
use super::AgentRegistry;

/// Embedded default agents YAML, compiled into the binary.
const DEFAULT_AGENTS_YAML: &str = include_str!("default_agents.yaml");

/// Create the default agent registry with all built-in agents.
///
/// Loads embedded defaults first, then optionally overlays user-defined
/// agents from a config file (whole-agent replacement by ID).
pub fn create_default_registry(llm: Arc<dyn LlmService>, config: AiConfig) -> AgentRegistry {
    let mut registry = AgentRegistry::new(llm, config);

    // 1. Parse and register embedded default agents
    register_from_yaml(&mut registry, DEFAULT_AGENTS_YAML, "built-in");

    // 2. Look for user config file and overlay
    if let Some(path) = find_user_agents_config() {
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                tracing::info!("Loading user agents from {}", path.display());
                register_from_yaml(&mut registry, &contents, &path.display().to_string());
            }
            Err(e) => {
                tracing::warn!("Failed to read user agents file {}: {e}", path.display());
            }
        }
    }

    registry
}

/// Parse a YAML string and register all agents into the registry.
///
/// On parse failure, logs a warning and continues with whatever agents
/// are already registered.
fn register_from_yaml(registry: &mut AgentRegistry, yaml: &str, source: &str) {
    match AgentsYaml::from_yaml(yaml) {
        Ok(parsed) => {
            let defaults = parsed.defaults;
            for entry in parsed.agents {
                let agent = ConfigAgent::from_entry(entry, &defaults);
                registry.register(Box::new(agent));
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
/// 1. `$SWEBASH_AGENTS_CONFIG` environment variable
/// 2. `~/.config/swebash/agents.yaml`
/// 3. `~/.swebash/agents.yaml`
fn find_user_agents_config() -> Option<PathBuf> {
    // 1. Explicit env var
    if let Ok(path) = std::env::var("SWEBASH_AGENTS_CONFIG") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
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
    use super::super::{Agent, ToolFilter};

    #[test]
    fn test_embedded_yaml_parses() {
        let parsed = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML)
            .expect("Embedded default_agents.yaml should parse");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.agents.len(), 4);

        let ids: Vec<&str> = parsed.agents.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"shell"));
        assert!(ids.contains(&"review"));
        assert!(ids.contains(&"devops"));
        assert!(ids.contains(&"git"));
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
        assert!(agent.trigger_keywords().contains(&"review"));
        assert!(agent.trigger_keywords().contains(&"audit"));
        match agent.tool_filter() {
            ToolFilter::Only { fs, exec, web } => {
                assert!(fs);
                assert!(!exec);
                assert!(!web);
            }
            _ => panic!("Expected ToolFilter::Only"),
        }
    }

    #[test]
    fn test_devops_agent() {
        let parsed = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML).unwrap();
        let defaults = parsed.defaults;
        let entry = parsed.agents.into_iter().find(|a| a.id == "devops").unwrap();
        let agent = ConfigAgent::from_entry(entry, &defaults);

        assert_eq!(agent.id(), "devops");
        assert!(agent.trigger_keywords().contains(&"docker"));
        assert!(agent.trigger_keywords().contains(&"k8s"));
        assert!(matches!(agent.tool_filter(), ToolFilter::All));
    }

    #[test]
    fn test_git_agent() {
        let parsed = AgentsYaml::from_yaml(DEFAULT_AGENTS_YAML).unwrap();
        let defaults = parsed.defaults;
        let entry = parsed.agents.into_iter().find(|a| a.id == "git").unwrap();
        let agent = ConfigAgent::from_entry(entry, &defaults);

        assert_eq!(agent.id(), "git");
        assert!(agent.trigger_keywords().contains(&"git"));
        assert!(agent.trigger_keywords().contains(&"commit"));
        match agent.tool_filter() {
            ToolFilter::Only { fs, exec, web } => {
                assert!(fs);
                assert!(exec);
                assert!(!web);
            }
            _ => panic!("Expected ToolFilter::Only"),
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
            tools: crate::config::ToolConfig::default(),
        };

        let mut registry = AgentRegistry::new(Arc::new(MockLlm), config);
        register_from_yaml(&mut registry, DEFAULT_AGENTS_YAML, "defaults");
        register_from_yaml(&mut registry, user_yaml, "user");

        // User override should replace the default shell agent
        let shell = registry.get("shell").unwrap();
        assert_eq!(shell.display_name(), "Custom Shell");
        assert_eq!(shell.description(), "My custom shell agent");

        // Other agents should still exist
        assert!(registry.get("review").is_some());
        assert!(registry.get("devops").is_some());
        assert!(registry.get("git").is_some());
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
            tools: crate::config::ToolConfig::default(),
        };

        let mut registry = AgentRegistry::new(Arc::new(MockLlm), config);
        register_from_yaml(&mut registry, DEFAULT_AGENTS_YAML, "defaults");
        register_from_yaml(&mut registry, user_yaml, "user");

        // New agent should be added alongside defaults
        assert_eq!(registry.list().len(), 5);
        let security = registry.get("security").unwrap();
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
            tools: crate::config::ToolConfig::default(),
        };

        let mut registry = AgentRegistry::new(Arc::new(MockLlm), config);
        register_from_yaml(&mut registry, DEFAULT_AGENTS_YAML, "defaults");
        register_from_yaml(&mut registry, "not: valid: yaml: [", "bad-user-file");

        // Defaults should still be present
        assert_eq!(registry.list().len(), 4);
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
