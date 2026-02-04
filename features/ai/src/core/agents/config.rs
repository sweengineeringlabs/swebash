/// YAML-configurable agent definitions.
///
/// Provides serde types for parsing agent YAML files and a `ConfigAgent`
/// that implements the `Agent` trait from parsed configuration.
use serde::Deserialize;

use super::{Agent, ToolFilter};

// ── YAML schema types ──────────────────────────────────────────────

/// Root of an agents YAML file.
#[derive(Debug, Deserialize)]
pub struct AgentsYaml {
    pub version: u32,
    #[serde(default)]
    pub defaults: AgentDefaults,
    pub agents: Vec<AgentEntry>,
}

/// Default values applied to agents that omit optional fields.
#[derive(Debug, Deserialize)]
pub struct AgentDefaults {
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens", rename = "maxTokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub tools: ToolsConfig,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            tools: ToolsConfig::default(),
        }
    }
}

/// Per-category tool toggles.
#[derive(Debug, Deserialize, Clone)]
pub struct ToolsConfig {
    #[serde(default = "bool_true")]
    pub fs: bool,
    #[serde(default = "bool_true")]
    pub exec: bool,
    #[serde(default = "bool_true")]
    pub web: bool,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            fs: true,
            exec: true,
            web: true,
        }
    }
}

/// A single agent entry in the YAML file.
#[derive(Debug, Deserialize)]
pub struct AgentEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub temperature: Option<f32>,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<u32>,
    #[serde(default, rename = "systemPrompt")]
    pub system_prompt: String,
    pub tools: Option<ToolsConfig>,
    #[serde(default, rename = "triggerKeywords")]
    pub trigger_keywords: Vec<String>,
}

// ── Defaults helpers ───────────────────────────────────────────────

fn default_temperature() -> f32 {
    0.5
}

fn default_max_tokens() -> u32 {
    1024
}

fn bool_true() -> bool {
    true
}

// ── Parsing ────────────────────────────────────────────────────────

impl AgentsYaml {
    /// Parse an agents YAML document.
    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| format!("Failed to parse agents YAML: {e}"))
    }
}

// ── ConfigAgent — Agent trait implementation ───────────────────────

/// An agent built from YAML configuration.
pub struct ConfigAgent {
    id: String,
    name: String,
    description: String,
    system_prompt: String,
    tool_filter: ToolFilter,
    temperature: f32,
    max_tokens: u32,
    trigger_keywords: Vec<String>,
}

impl ConfigAgent {
    /// Build a `ConfigAgent` from an `AgentEntry`, filling in defaults.
    pub fn from_entry(entry: AgentEntry, defaults: &AgentDefaults) -> Self {
        let tools = entry.tools.as_ref().unwrap_or(&defaults.tools);
        let tool_filter = if tools.fs && tools.exec && tools.web {
            ToolFilter::All
        } else if !tools.fs && !tools.exec && !tools.web {
            ToolFilter::None
        } else {
            ToolFilter::Only {
                fs: tools.fs,
                exec: tools.exec,
                web: tools.web,
            }
        };

        Self {
            id: entry.id,
            name: entry.name,
            description: entry.description,
            system_prompt: entry.system_prompt,
            tool_filter,
            temperature: entry.temperature.unwrap_or(defaults.temperature),
            max_tokens: entry.max_tokens.unwrap_or(defaults.max_tokens),
            trigger_keywords: entry.trigger_keywords,
        }
    }
}

impl Agent for ConfigAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn system_prompt(&self) -> String {
        self.system_prompt.clone()
    }

    fn tool_filter(&self) -> ToolFilter {
        self.tool_filter.clone()
    }

    fn temperature(&self) -> Option<f32> {
        Some(self.temperature)
    }

    fn max_tokens(&self) -> Option<u32> {
        Some(self.max_tokens)
    }

    fn trigger_keywords(&self) -> Vec<&str> {
        self.trigger_keywords.iter().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_YAML: &str = r#"
version: 1
agents:
  - id: test
    name: Test Agent
    description: A test agent
    systemPrompt: You are a test agent.
"#;

    const FULL_YAML: &str = r#"
version: 1
defaults:
  temperature: 0.7
  maxTokens: 2048
  tools:
    fs: true
    exec: true
    web: false
agents:
  - id: alpha
    name: Alpha Agent
    description: First agent
    systemPrompt: Alpha prompt.
    triggerKeywords: [alpha, first]
  - id: beta
    name: Beta Agent
    description: Second agent
    temperature: 0.3
    maxTokens: 512
    tools:
      fs: true
      exec: false
      web: false
    systemPrompt: Beta prompt.
"#;

    #[test]
    fn test_parse_minimal_yaml() {
        let parsed = AgentsYaml::from_yaml(MINIMAL_YAML).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.agents.len(), 1);
        assert_eq!(parsed.agents[0].id, "test");
        assert_eq!(parsed.agents[0].name, "Test Agent");
    }

    #[test]
    fn test_parse_full_yaml() {
        let parsed = AgentsYaml::from_yaml(FULL_YAML).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.defaults.temperature, 0.7);
        assert_eq!(parsed.defaults.max_tokens, 2048);
        assert!(!parsed.defaults.tools.web);
        assert_eq!(parsed.agents.len(), 2);

        // alpha inherits defaults
        let alpha = &parsed.agents[0];
        assert_eq!(alpha.id, "alpha");
        assert!(alpha.temperature.is_none());
        assert_eq!(alpha.trigger_keywords, vec!["alpha", "first"]);

        // beta overrides
        let beta = &parsed.agents[1];
        assert_eq!(beta.temperature, Some(0.3));
        assert_eq!(beta.max_tokens, Some(512));
    }

    #[test]
    fn test_config_agent_from_entry_defaults() {
        let parsed = AgentsYaml::from_yaml(MINIMAL_YAML).unwrap();
        let entry = parsed.agents.into_iter().next().unwrap();
        let agent = ConfigAgent::from_entry(entry, &parsed.defaults);

        assert_eq!(agent.id(), "test");
        assert_eq!(agent.display_name(), "Test Agent");
        assert_eq!(agent.temperature(), Some(0.5)); // default
        assert_eq!(agent.max_tokens(), Some(1024)); // default
        assert!(matches!(agent.tool_filter(), ToolFilter::All)); // all true by default
        assert!(agent.trigger_keywords().is_empty());
    }

    #[test]
    fn test_config_agent_from_entry_overrides() {
        let parsed = AgentsYaml::from_yaml(FULL_YAML).unwrap();
        let defaults = &parsed.defaults;
        let mut agents = parsed.agents.into_iter();

        let alpha = ConfigAgent::from_entry(agents.next().unwrap(), defaults);
        assert_eq!(alpha.temperature(), Some(0.7)); // inherits default
        assert_eq!(alpha.max_tokens(), Some(2048)); // inherits default
        // alpha has no tools override, so inherits defaults (fs=true, exec=true, web=false)
        match alpha.tool_filter() {
            ToolFilter::Only { fs, exec, web } => {
                assert!(fs);
                assert!(exec);
                assert!(!web);
            }
            _ => panic!("Expected ToolFilter::Only for alpha"),
        }
        assert_eq!(alpha.trigger_keywords(), vec!["alpha", "first"]);

        let beta = ConfigAgent::from_entry(agents.next().unwrap(), defaults);
        assert_eq!(beta.temperature(), Some(0.3)); // overridden
        assert_eq!(beta.max_tokens(), Some(512)); // overridden
        match beta.tool_filter() {
            ToolFilter::Only { fs, exec, web } => {
                assert!(fs);
                assert!(!exec);
                assert!(!web);
            }
            _ => panic!("Expected ToolFilter::Only for beta"),
        }
    }

    #[test]
    fn test_tool_filter_all_when_all_true() {
        let entry = AgentEntry {
            id: "all".into(),
            name: "All".into(),
            description: "All tools".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "prompt".into(),
            tools: Some(ToolsConfig {
                fs: true,
                exec: true,
                web: true,
            }),
            trigger_keywords: vec![],
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        assert!(matches!(agent.tool_filter(), ToolFilter::All));
    }

    #[test]
    fn test_tool_filter_none_when_all_false() {
        let entry = AgentEntry {
            id: "none".into(),
            name: "None".into(),
            description: "No tools".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "prompt".into(),
            tools: Some(ToolsConfig {
                fs: false,
                exec: false,
                web: false,
            }),
            trigger_keywords: vec![],
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        assert!(matches!(agent.tool_filter(), ToolFilter::None));
    }

    #[test]
    fn test_invalid_yaml() {
        let result = AgentsYaml::from_yaml("not: valid: yaml: [");
        assert!(result.is_err());
    }
}
