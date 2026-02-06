/// YAML-configurable agent definitions.
///
/// Provides serde types for parsing agent YAML files and a `ConfigAgent`
/// that implements the `AgentDescriptor` trait from parsed configuration.
use std::path::Path;

use serde::Deserialize;

use agent_controller::{AgentDescriptor, ToolFilter};

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
    #[serde(default, rename = "thinkFirst")]
    pub think_first: bool,
    #[serde(default, rename = "bypassConfirmation")]
    pub bypass_confirmation: bool,
    #[serde(default, rename = "maxIterations")]
    pub max_iterations: Option<usize>,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            tools: ToolsConfig::default(),
            think_first: false,
            bypass_confirmation: false,
            max_iterations: None,
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

/// Document context configuration for loading reference material.
#[derive(Debug, Deserialize)]
pub struct DocsConfig {
    /// Token budget for document context (heuristic: 1 token ≈ 4 chars).
    pub budget: usize,
    /// File paths or glob patterns to load as documentation.
    pub sources: Vec<String>,
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
    #[serde(default, rename = "thinkFirst")]
    pub think_first: Option<bool>,
    #[serde(default, rename = "bypassConfirmation")]
    pub bypass_confirmation: Option<bool>,
    #[serde(default, rename = "maxIterations")]
    pub max_iterations: Option<usize>,
    /// Document context configuration.
    #[serde(default)]
    pub docs: Option<DocsConfig>,
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

// ── Document loading ────────────────────────────────────────────────

/// Load document context from file sources, respecting a token budget.
///
/// Expands globs, reads files, prepends `--- path ---` headers, and
/// truncates to `budget * 4` chars (heuristic: 1 token ≈ 4 chars).
/// Missing files are skipped with a warning (fail-open).
/// Returns `None` if no content was loaded.
pub fn load_docs_context(docs: &DocsConfig, base_dir: &Path) -> Option<String> {
    if docs.sources.is_empty() {
        return None;
    }

    let char_budget = docs.budget * 4;
    let mut result = String::new();

    for pattern in &docs.sources {
        let full_pattern = base_dir.join(pattern).to_string_lossy().to_string();
        let paths = match glob::glob(&full_pattern) {
            Ok(paths) => paths,
            Err(e) => {
                tracing::warn!(pattern = %pattern, error = %e, "invalid glob pattern, skipping");
                continue;
            }
        };

        for entry in paths {
            let path = match entry {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "glob entry error, skipping");
                    continue;
                }
            };

            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let display_path = path.strip_prefix(base_dir)
                        .unwrap_or(&path)
                        .display();
                    let header = format!("--- {} ---\n", display_path);
                    result.push_str(&header);
                    result.push_str(&content);
                    result.push('\n');
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "failed to read doc file, skipping");
                }
            }

            if result.len() >= char_budget {
                break;
            }
        }

        if result.len() >= char_budget {
            break;
        }
    }

    if result.is_empty() {
        return None;
    }

    // Truncate to budget
    if result.len() > char_budget {
        result.truncate(char_budget);
    }

    Some(result)
}

// ── Parsing ────────────────────────────────────────────────────────

impl AgentsYaml {
    /// Parse an agents YAML document.
    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| format!("Failed to parse agents YAML: {e}"))
    }
}

// ── ConfigAgent — AgentDescriptor implementation ────────────────────

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
    bypass_confirmation: bool,
    max_iterations: Option<usize>,
}

impl ConfigAgent {
    /// Build a `ConfigAgent` from an `AgentEntry`, filling in defaults.
    ///
    /// If `base_dir` is provided and the entry has a `docs` section,
    /// document content is loaded and prepended to the system prompt.
    pub fn from_entry(entry: AgentEntry, defaults: &AgentDefaults) -> Self {
        Self::from_entry_with_base_dir(entry, defaults, None)
    }

    /// Build a `ConfigAgent` with an optional base directory for doc loading.
    pub fn from_entry_with_base_dir(
        entry: AgentEntry,
        defaults: &AgentDefaults,
        base_dir: Option<&Path>,
    ) -> Self {
        let tools = entry.tools.as_ref().unwrap_or(&defaults.tools);
        let tool_filter = if tools.fs && tools.exec && tools.web {
            ToolFilter::All
        } else {
            let mut cats = Vec::new();
            if tools.fs {
                cats.push("fs".to_string());
            }
            if tools.exec {
                cats.push("exec".to_string());
            }
            if tools.web {
                cats.push("web".to_string());
            }
            ToolFilter::Categories(cats)
        };

        let think_first = entry.think_first.unwrap_or(defaults.think_first);
        let mut system_prompt = if think_first && !entry.system_prompt.is_empty() {
            format!(
                "{}\nAlways explain your reasoning and what you plan to do before calling any tools.\n\
                 Provide a brief explanation of your approach first, then use tools to execute it.",
                entry.system_prompt.trim_end()
            )
        } else {
            entry.system_prompt
        };

        // Load document context and prepend to system prompt.
        // TODO: migrate to ChatConfig.docs_context when ChatEngine supports it
        if let (Some(docs), Some(dir)) = (&entry.docs, base_dir) {
            if let Some(docs_content) = load_docs_context(docs, dir) {
                system_prompt = format!(
                    "<documentation>\n{}\n</documentation>\n\n{}",
                    docs_content, system_prompt
                );
            }
        }

        Self {
            id: entry.id,
            name: entry.name,
            description: entry.description,
            system_prompt,
            tool_filter,
            temperature: entry.temperature.unwrap_or(defaults.temperature),
            max_tokens: entry.max_tokens.unwrap_or(defaults.max_tokens),
            trigger_keywords: entry.trigger_keywords,
            bypass_confirmation: entry
                .bypass_confirmation
                .unwrap_or(defaults.bypass_confirmation),
            max_iterations: entry.max_iterations.or(defaults.max_iterations),
        }
    }

    /// Whether this agent bypasses tool confirmation prompts.
    pub fn bypass_confirmation(&self) -> bool {
        self.bypass_confirmation
    }

    /// Per-agent tool iteration limit override, if set.
    pub fn max_iterations(&self) -> Option<usize> {
        self.max_iterations
    }
}

impl AgentDescriptor for ConfigAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn system_prompt(&self) -> &str {
        &self.system_prompt
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

    fn trigger_keywords(&self) -> &[String] {
        &self.trigger_keywords
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
            ToolFilter::Categories(cats) => {
                assert!(cats.contains(&"fs".to_string()));
                assert!(cats.contains(&"exec".to_string()));
                assert!(!cats.contains(&"web".to_string()));
            }
            _ => panic!("Expected ToolFilter::Categories for alpha"),
        }
        assert_eq!(alpha.trigger_keywords(), &["alpha".to_string(), "first".to_string()]);

        let beta = ConfigAgent::from_entry(agents.next().unwrap(), defaults);
        assert_eq!(beta.temperature(), Some(0.3)); // overridden
        assert_eq!(beta.max_tokens(), Some(512)); // overridden
        match beta.tool_filter() {
            ToolFilter::Categories(cats) => {
                assert!(cats.contains(&"fs".to_string()));
                assert!(!cats.contains(&"exec".to_string()));
                assert!(!cats.contains(&"web".to_string()));
            }
            _ => panic!("Expected ToolFilter::Categories for beta"),
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
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
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
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        match agent.tool_filter() {
            ToolFilter::Categories(cats) => assert!(cats.is_empty()),
            _ => panic!("Expected ToolFilter::Categories(empty) for none"),
        }
    }

    #[test]
    fn test_invalid_yaml() {
        let result = AgentsYaml::from_yaml("not: valid: yaml: [");
        assert!(result.is_err());
    }

    // ── AgentDescriptor return-type tests ────────────────────────────

    #[test]
    fn test_descriptor_system_prompt_returns_borrowed_str() {
        let entry = AgentEntry {
            id: "sp".into(),
            name: "SP".into(),
            description: "desc".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "Multiline\nprompt\nhere.".into(),
            tools: None,
            trigger_keywords: vec![],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        // system_prompt() returns &str — borrow from owned field
        let prompt: &str = agent.system_prompt();
        assert_eq!(prompt, "Multiline\nprompt\nhere.");
    }

    #[test]
    fn test_descriptor_trigger_keywords_returns_borrowed_slice() {
        let entry = AgentEntry {
            id: "kw".into(),
            name: "KW".into(),
            description: "desc".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "prompt".into(),
            tools: None,
            trigger_keywords: vec!["a".into(), "b".into(), "c".into()],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        // trigger_keywords() returns &[String] — borrow from owned Vec
        let kw: &[String] = agent.trigger_keywords();
        assert_eq!(kw.len(), 3);
        assert_eq!(kw[0], "a");
        assert_eq!(kw[2], "c");
    }

    // ── ToolFilter::Categories mapping completeness ─────────────────

    #[test]
    fn test_categories_single_exec() {
        let entry = AgentEntry {
            id: "exec-only".into(),
            name: "Exec".into(),
            description: "Exec only".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "prompt".into(),
            tools: Some(ToolsConfig { fs: false, exec: true, web: false }),
            trigger_keywords: vec![],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        match agent.tool_filter() {
            ToolFilter::Categories(cats) => {
                assert_eq!(cats, vec!["exec".to_string()]);
            }
            _ => panic!("Expected Categories"),
        }
    }

    #[test]
    fn test_categories_single_web() {
        let entry = AgentEntry {
            id: "web-only".into(),
            name: "Web".into(),
            description: "Web only".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "prompt".into(),
            tools: Some(ToolsConfig { fs: false, exec: false, web: true }),
            trigger_keywords: vec![],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        match agent.tool_filter() {
            ToolFilter::Categories(cats) => {
                assert_eq!(cats, vec!["web".to_string()]);
            }
            _ => panic!("Expected Categories"),
        }
    }

    #[test]
    fn test_categories_fs_and_web() {
        let entry = AgentEntry {
            id: "fw".into(),
            name: "FW".into(),
            description: "FS + Web".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "prompt".into(),
            tools: Some(ToolsConfig { fs: true, exec: false, web: true }),
            trigger_keywords: vec![],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        match agent.tool_filter() {
            ToolFilter::Categories(cats) => {
                assert_eq!(cats.len(), 2);
                assert_eq!(cats[0], "fs");
                assert_eq!(cats[1], "web");
            }
            _ => panic!("Expected Categories"),
        }
    }

    #[test]
    fn test_categories_exec_and_web() {
        let entry = AgentEntry {
            id: "ew".into(),
            name: "EW".into(),
            description: "Exec + Web".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "prompt".into(),
            tools: Some(ToolsConfig { fs: false, exec: true, web: true }),
            trigger_keywords: vec![],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        match agent.tool_filter() {
            ToolFilter::Categories(cats) => {
                assert_eq!(cats.len(), 2);
                assert_eq!(cats[0], "exec");
                assert_eq!(cats[1], "web");
            }
            _ => panic!("Expected Categories"),
        }
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn test_empty_system_prompt() {
        let entry = AgentEntry {
            id: "empty-prompt".into(),
            name: "Empty".into(),
            description: "desc".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: String::new(),
            tools: None,
            trigger_keywords: vec![],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        assert_eq!(agent.system_prompt(), "");
    }

    #[test]
    fn test_many_trigger_keywords() {
        let kws: Vec<String> = (0..20).map(|i| format!("kw{i}")).collect();
        let entry = AgentEntry {
            id: "many-kw".into(),
            name: "Many".into(),
            description: "desc".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "prompt".into(),
            tools: None,
            trigger_keywords: kws.clone(),
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let agent = ConfigAgent::from_entry(entry, &AgentDefaults::default());
        assert_eq!(agent.trigger_keywords().len(), 20);
        assert_eq!(agent.trigger_keywords(), kws.as_slice());
    }

    #[test]
    fn test_register_overwrites_same_id() {
        let defaults = AgentDefaults::default();
        let entry1 = AgentEntry {
            id: "dup".into(),
            name: "Original".into(),
            description: "First version".into(),
            temperature: None,
            max_tokens: None,
            system_prompt: "p1".into(),
            tools: None,
            trigger_keywords: vec![],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let entry2 = AgentEntry {
            id: "dup".into(),
            name: "Replacement".into(),
            description: "Second version".into(),
            temperature: Some(0.9),
            max_tokens: None,
            system_prompt: "p2".into(),
            tools: None,
            trigger_keywords: vec!["new".into()],
            think_first: None,
            bypass_confirmation: None,
            max_iterations: None,
            docs: None,
        };
        let a1 = ConfigAgent::from_entry(entry1, &defaults);
        let a2 = ConfigAgent::from_entry(entry2, &defaults);
        assert_eq!(a1.display_name(), "Original");
        assert_eq!(a2.display_name(), "Replacement");
        assert_eq!(a2.temperature(), Some(0.9));
        assert_eq!(a2.trigger_keywords(), &["new".to_string()]);
    }

    // ── Document Context Tests ──────────────────────────────────────

    const DOCS_YAML: &str = r#"
version: 1
agents:
  - id: with-docs
    name: Docs Agent
    description: Agent with docs
    systemPrompt: You are helpful.
    docs:
      budget: 8000
      sources:
        - "docs/*.md"
"#;

    const NO_DOCS_YAML: &str = r#"
version: 1
agents:
  - id: no-docs
    name: No Docs Agent
    description: Agent without docs
    systemPrompt: You are helpful.
"#;

    #[test]
    fn test_yaml_with_docs_field_parses() {
        let parsed = AgentsYaml::from_yaml(DOCS_YAML).unwrap();
        assert_eq!(parsed.agents.len(), 1);
        let docs = parsed.agents[0].docs.as_ref().unwrap();
        assert_eq!(docs.budget, 8000);
        assert_eq!(docs.sources, vec!["docs/*.md"]);
    }

    #[test]
    fn test_yaml_without_docs_field_still_works() {
        let parsed = AgentsYaml::from_yaml(NO_DOCS_YAML).unwrap();
        assert_eq!(parsed.agents.len(), 1);
        assert!(parsed.agents[0].docs.is_none());
    }

    #[test]
    fn test_load_docs_context_with_files() {
        let dir = tempfile::tempdir().unwrap();
        let docs_dir = dir.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("a.md"), "# File A\nContent A").unwrap();
        std::fs::write(docs_dir.join("b.md"), "# File B\nContent B").unwrap();

        let config = DocsConfig {
            budget: 8000,
            sources: vec!["docs/*.md".to_string()],
        };

        let result = load_docs_context(&config, dir.path());
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("--- docs"));
        assert!(text.contains("Content A"));
        assert!(text.contains("Content B"));
    }

    #[test]
    fn test_load_docs_context_missing_files_skipped() {
        let dir = tempfile::tempdir().unwrap();

        let config = DocsConfig {
            budget: 8000,
            sources: vec!["nonexistent/*.md".to_string()],
        };

        let result = load_docs_context(&config, dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_load_docs_context_budget_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let docs_dir = dir.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        // Write a file larger than budget
        let content = "x".repeat(5000);
        std::fs::write(docs_dir.join("big.md"), &content).unwrap();

        let config = DocsConfig {
            budget: 100, // 100 tokens = 400 chars
            sources: vec!["docs/*.md".to_string()],
        };

        let result = load_docs_context(&config, dir.path());
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.len() <= 400);
    }

    #[test]
    fn test_load_docs_context_empty_sources_returns_none() {
        let dir = tempfile::tempdir().unwrap();

        let config = DocsConfig {
            budget: 8000,
            sources: vec![],
        };

        let result = load_docs_context(&config, dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_load_docs_context_glob_expansion() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("crates").join("compiler");
        std::fs::create_dir_all(sub.join("lexer")).unwrap();
        std::fs::create_dir_all(sub.join("parser")).unwrap();
        std::fs::write(sub.join("lexer").join("README.md"), "Lexer docs").unwrap();
        std::fs::write(sub.join("parser").join("README.md"), "Parser docs").unwrap();

        let config = DocsConfig {
            budget: 8000,
            sources: vec!["crates/compiler/*/README.md".to_string()],
        };

        let result = load_docs_context(&config, dir.path());
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Lexer docs"));
        assert!(text.contains("Parser docs"));
    }
}
