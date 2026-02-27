use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

use super::git_config::GitConfig;
use super::state::{AccessMode, PathRule, SandboxPolicy};

/// Top-level config file structure (`~/.config/swebash/config.toml`).
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SwebashConfig {
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    /// AI assistant configuration.
    #[serde(default)]
    pub ai: AiConfig,
    /// Git branch pipeline and safety gate configuration.
    /// Deprecated: Use `workspaces` instead for repo-bound configs.
    #[serde(default)]
    pub git: Option<GitConfig>,
    /// Whether the first-run setup wizard has been completed.
    #[serde(default)]
    pub setup_completed: bool,
    /// Workspace-to-repository bindings.
    /// Each workspace is permanently bound to a specific git repository.
    #[serde(default)]
    pub bound_workspaces: Vec<BoundWorkspace>,
}

/// A workspace permanently bound to a specific git repository.
/// Once created, the binding cannot be changed - only deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundWorkspace {
    /// Workspace root path (absolute).
    pub workspace_path: String,
    /// Git remote URL this workspace is bound to (for verification).
    pub repo_remote: String,
    /// Local repository path at time of binding.
    pub repo_local: String,
    /// ISO 8601 timestamp when this binding was created.
    pub bound_at: String,
    /// Git branch pipeline and safety gates for this workspace.
    #[serde(default)]
    pub git: Option<GitConfig>,
}

impl BoundWorkspace {
    /// Check if the given path matches this workspace.
    pub fn matches_workspace(&self, path: &str) -> bool {
        let workspace = normalize_path(&self.workspace_path);
        let check = normalize_path(path);
        check.starts_with(&workspace)
    }

    /// Check if the given repo remote matches this binding.
    pub fn matches_remote(&self, remote: &str) -> bool {
        normalize_remote(&self.repo_remote) == normalize_remote(remote)
    }
}

/// Normalize a path for comparison (lowercase on Windows, forward slashes).
fn normalize_path(path: &str) -> String {
    let p = path.replace('\\', "/");
    #[cfg(windows)]
    {
        p.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        p
    }
}

/// Normalize a git remote URL for comparison.
/// Handles: https://github.com/user/repo.git vs git@github.com:user/repo.git
fn normalize_remote(remote: &str) -> String {
    let r = remote.trim();
    // Remove trailing .git
    let r = r.strip_suffix(".git").unwrap_or(r);
    // Convert SSH to HTTPS format for comparison
    if r.starts_with("git@") {
        // git@github.com:user/repo -> github.com/user/repo
        r.strip_prefix("git@")
            .unwrap_or(r)
            .replace(':', "/")
            .to_lowercase()
    } else if r.starts_with("https://") {
        // https://github.com/user/repo -> github.com/user/repo
        r.strip_prefix("https://").unwrap_or(r).to_lowercase()
    } else if r.starts_with("http://") {
        r.strip_prefix("http://").unwrap_or(r).to_lowercase()
    } else {
        r.to_lowercase()
    }
}

impl SwebashConfig {
    /// Find a bound workspace that matches the given path.
    pub fn find_workspace_for_path(&self, path: &str) -> Option<&BoundWorkspace> {
        self.bound_workspaces
            .iter()
            .find(|ws| ws.matches_workspace(path))
    }

    /// Find a bound workspace that matches the given repo remote.
    pub fn find_workspace_for_remote(&self, remote: &str) -> Option<&BoundWorkspace> {
        self.bound_workspaces
            .iter()
            .find(|ws| ws.matches_remote(remote))
    }

    /// Check if a workspace is already bound to any repo.
    pub fn is_workspace_bound(&self, path: &str) -> bool {
        self.find_workspace_for_path(path).is_some()
    }

    /// Verify that the current directory's repo matches the bound workspace.
    /// Returns Err with message if there's a mismatch.
    pub fn verify_repo_binding(&self, workspace_path: &str, current_remote: &str) -> Result<(), String> {
        if let Some(bound) = self.find_workspace_for_path(workspace_path) {
            if !bound.matches_remote(current_remote) {
                return Err(format!(
                    "Workspace mismatch: This workspace is bound to '{}' but current repo is '{}'.\n\
                     Commits are blocked to prevent accidental changes to the wrong repository.",
                    bound.repo_remote, current_remote
                ));
            }
        }
        Ok(())
    }
}

/// `[ai]` section of the config.
///
/// API keys can be stored here instead of environment variables.
/// Environment variables always take precedence over config file values.
///
/// Example config:
/// ```toml
/// [ai]
/// enabled = true
/// provider = "anthropic"
/// anthropic_api_key = "sk-ant-..."
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct AiConfig {
    /// Master switch to enable/disable AI features. Default: `true`.
    /// When `false`, AI mode is completely disabled regardless of other settings.
    /// Can also be controlled via `SWEBASH_AI_ENABLED` env var (env takes precedence).
    #[serde(default = "default_ai_enabled")]
    pub enabled: bool,
    /// LLM provider: "openai", "anthropic", or "gemini". Default: "openai".
    /// Can be overridden via `LLM_PROVIDER` env var.
    #[serde(default)]
    pub provider: Option<String>,
    /// Model to use (e.g. "gpt-4o", "claude-sonnet-4-20250514").
    /// Can be overridden via `LLM_DEFAULT_MODEL` env var.
    #[serde(default)]
    pub model: Option<String>,
    /// OpenAI API key. Can be overridden via `OPENAI_API_KEY` env var.
    #[serde(default)]
    pub openai_api_key: Option<String>,
    /// Anthropic API key. Can be overridden via `ANTHROPIC_API_KEY` env var.
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
    /// Google Gemini API key. Can be overridden via `GEMINI_API_KEY` env var.
    #[serde(default)]
    pub gemini_api_key: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: default_ai_enabled(),
            provider: None,
            model: None,
            openai_api_key: None,
            anthropic_api_key: None,
            gemini_api_key: None,
        }
    }
}

impl AiConfig {
    /// Get the API key for the given provider, checking env var first, then config file.
    pub fn api_key_for_provider(&self, provider: &str) -> Option<String> {
        match provider {
            "openai" => std::env::var("OPENAI_API_KEY")
                .ok()
                .or_else(|| self.openai_api_key.clone()),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .or_else(|| self.anthropic_api_key.clone()),
            "gemini" => std::env::var("GEMINI_API_KEY")
                .ok()
                .or_else(|| self.gemini_api_key.clone()),
            _ => None,
        }
    }

    /// Get the effective provider (env var overrides config file).
    pub fn effective_provider(&self) -> String {
        std::env::var("LLM_PROVIDER")
            .ok()
            .or_else(|| self.provider.clone())
            .unwrap_or_else(|| "openai".to_string())
    }

    /// Get the effective model (env var overrides config file).
    pub fn effective_model(&self) -> Option<String> {
        std::env::var("LLM_DEFAULT_MODEL")
            .ok()
            .or_else(|| self.model.clone())
    }

    /// Check if an API key is available for the effective provider.
    pub fn has_api_key(&self) -> bool {
        let provider = self.effective_provider();
        self.api_key_for_provider(&provider).is_some()
    }
}

fn default_ai_enabled() -> bool {
    true
}

/// `[workspace]` section of the config.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Workspace root path (supports `~` expansion). Default: `~/workspace`.
    #[serde(default = "default_workspace_root")]
    pub root: String,
    /// Default access mode for the workspace root: `"ro"` or `"rw"`.
    #[serde(default = "default_mode_str")]
    pub mode: String,
    /// Whether the sandbox is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Additional allowed paths.
    #[serde(default)]
    pub allow: Vec<AllowEntry>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: default_workspace_root(),
            mode: default_mode_str(),
            enabled: default_enabled(),
            allow: Vec::new(),
        }
    }
}

/// A single `[[workspace.allow]]` entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct AllowEntry {
    pub path: String,
    #[serde(default = "default_mode_str")]
    pub mode: String,
}

fn default_workspace_root() -> String {
    std::env::var("SWEBASH_WORKSPACE")
        .unwrap_or_else(|_| "~/.config/swebash/workspace".to_string())
}

fn default_mode_str() -> String {
    "ro".to_string()
}

fn default_enabled() -> bool {
    true
}

/// Parse an access-mode string (`"ro"` / `"rw"`). Defaults to `ReadOnly`.
fn parse_mode(s: &str) -> AccessMode {
    match s.to_ascii_lowercase().as_str() {
        "rw" | "readwrite" | "read-write" => AccessMode::ReadWrite,
        _ => AccessMode::ReadOnly,
    }
}

/// Expand a leading `~` or `~/` in a path string to the user's home directory.
fn expand_tilde(raw: &str) -> PathBuf {
    if raw == "~" {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(raw))
    } else if let Some(rest) = raw.strip_prefix("~/") {
        dirs::home_dir()
            .map(|h| h.join(rest))
            .unwrap_or_else(|| PathBuf::from(raw))
    } else {
        PathBuf::from(raw)
    }
}

impl SwebashConfig {
    /// Build a `SandboxPolicy` from this config.
    pub fn to_policy(&self) -> SandboxPolicy {
        let ws = &self.workspace;
        let workspace_root = expand_tilde(&ws.root);
        let workspace_root = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.clone());

        let mut allowed_paths = vec![PathRule {
            root: workspace_root.clone(),
            mode: parse_mode(&ws.mode),
        }];

        for entry in &ws.allow {
            let path = expand_tilde(&entry.path);
            let path = path.canonicalize().unwrap_or(path);
            allowed_paths.push(PathRule {
                root: path,
                mode: parse_mode(&entry.mode),
            });
        }

        SandboxPolicy {
            workspace_root,
            allowed_paths,
            enabled: ws.enabled,
        }
    }
}

/// Load the config file from `~/.config/swebash/config.toml`.
/// Returns the default config if the file is missing or malformed.
pub fn load_config() -> SwebashConfig {
    let config_path = dirs::home_dir()
        .map(|h| h.join(".config").join("swebash").join("config.toml"))
        .unwrap_or_else(|| PathBuf::from(".config/swebash/config.toml"));

    match std::fs::read_to_string(&config_path) {
        Ok(contents) => match toml::from_str::<SwebashConfig>(&contents) {
            Ok(cfg) => cfg,
            Err(e) => {
                warn!(
                    path = %config_path.display(),
                    error = %e,
                    "failed to parse config file, using defaults"
                );
                SwebashConfig::default()
            }
        },
        Err(_) => SwebashConfig::default(),
    }
}

/// Save the config to `~/.config/swebash/config.toml`.
/// Creates the parent directory if it does not exist.
pub fn save_config(config: &SwebashConfig) -> Result<(), String> {
    let config_dir = dirs::home_dir()
        .map(|h| h.join(".config").join("swebash"))
        .ok_or_else(|| "could not determine home directory".to_string())?;

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("could not create config directory {}: {e}", config_dir.display()))?;

    let config_path = config_dir.join("config.toml");
    let content = toml::to_string_pretty(config)
        .map_err(|e| format!("failed to serialize config: {e}"))?;

    std::fs::write(&config_path, content)
        .map_err(|e| format!("failed to write {}: {e}", config_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::git_config::{
        BranchPipeline, GateAction, GitConfig,
    };

    #[test]
    fn default_config_setup_completed_false() {
        let config = SwebashConfig::default();
        assert!(!config.setup_completed);
        assert!(config.git.is_none());
    }

    #[test]
    fn default_config_workspace_defaults() {
        // Clear env var to test the true default
        std::env::remove_var("SWEBASH_WORKSPACE");
        let config = SwebashConfig::default();
        assert_eq!(config.workspace.root, "~/.config/swebash/workspace");
        assert_eq!(config.workspace.mode, "ro");
        assert!(config.workspace.enabled);
        assert!(config.workspace.allow.is_empty());
    }

    #[test]
    fn default_workspace_follows_xdg_spec() {
        // Clear env var to test the true default
        std::env::remove_var("SWEBASH_WORKSPACE");
        // XDG Base Directory Specification: config goes in ~/.config/<app>
        let root = default_workspace_root();
        assert!(root.starts_with("~/.config/swebash"));
        assert!(root.contains("workspace"));
    }

    #[test]
    fn env_var_overrides_default_workspace() {
        let test_path = "/custom/workspace/path";
        std::env::set_var("SWEBASH_WORKSPACE", test_path);
        let root = default_workspace_root();
        assert_eq!(root, test_path);
        std::env::remove_var("SWEBASH_WORKSPACE");
    }

    #[test]
    fn serde_roundtrip_default_config() {
        let config = SwebashConfig::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: SwebashConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.workspace.root, config.workspace.root);
        assert_eq!(deserialized.setup_completed, config.setup_completed);
        assert!(deserialized.git.is_none());
    }

    #[test]
    fn serde_roundtrip_with_git_config() {
        let pipeline = BranchPipeline::default_for_user("tester");
        let gates = GitConfig::default_gates(&pipeline);
        let git_config = GitConfig {
            user_id: "tester".to_string(),
            pipeline,
            gates,
        };
        let config = SwebashConfig {
            workspace: WorkspaceConfig::default(),
            ai: AiConfig::default(),
            git: Some(git_config),
            setup_completed: true,
            bound_workspaces: vec![],
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: SwebashConfig = toml::from_str(&serialized).unwrap();

        assert!(deserialized.setup_completed);
        let git = deserialized.git.unwrap();
        assert_eq!(git.user_id, "tester");
        assert_eq!(git.pipeline.branches.len(), 6);
        assert_eq!(git.gates.len(), 6);
        assert_eq!(git.gates[0].can_force_push, GateAction::Deny);
    }

    #[test]
    fn deserialize_legacy_config_without_git_fields() {
        // Simulate a config.toml from before git gates were added
        let toml_str = r#"
[workspace]
root = "~/myproject"
mode = "rw"
enabled = true
"#;
        let config: SwebashConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.workspace.root, "~/myproject");
        assert_eq!(config.workspace.mode, "rw");
        assert!(!config.setup_completed); // defaults to false
        assert!(config.git.is_none()); // defaults to None
    }

    #[test]
    fn deserialize_with_setup_completed_true() {
        let toml_str = r#"
setup_completed = true

[workspace]
root = "~/workspace"
"#;
        let config: SwebashConfig = toml::from_str(toml_str).unwrap();
        assert!(config.setup_completed);
    }

    #[test]
    fn to_policy_preserves_enabled_flag() {
        let mut config = SwebashConfig::default();
        config.workspace.enabled = false;
        let policy = config.to_policy();
        assert!(!policy.enabled);
    }

    #[test]
    fn to_policy_sets_rw_mode() {
        let mut config = SwebashConfig::default();
        config.workspace.mode = "rw".to_string();
        let policy = config.to_policy();
        assert_eq!(policy.allowed_paths[0].mode, AccessMode::ReadWrite);
    }

    #[test]
    fn parse_mode_variants() {
        assert_eq!(parse_mode("ro"), AccessMode::ReadOnly);
        assert_eq!(parse_mode("rw"), AccessMode::ReadWrite);
        assert_eq!(parse_mode("readwrite"), AccessMode::ReadWrite);
        assert_eq!(parse_mode("read-write"), AccessMode::ReadWrite);
        assert_eq!(parse_mode("RW"), AccessMode::ReadWrite);
        assert_eq!(parse_mode("anything_else"), AccessMode::ReadOnly);
    }

    #[test]
    fn save_config_serializes_git_section() {
        let pipeline = BranchPipeline::default_for_user("u");
        let gates = GitConfig::default_gates(&pipeline);
        let config = SwebashConfig {
            workspace: WorkspaceConfig::default(),
            ai: AiConfig::default(),
            git: Some(GitConfig {
                user_id: "u".to_string(),
                pipeline,
                gates,
            }),
            setup_completed: true,
            bound_workspaces: vec![],
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        assert!(serialized.contains("setup_completed = true"));
        assert!(serialized.contains("[git]"));
        assert!(serialized.contains("user_id = \"u\""));
        assert!(serialized.contains("[[git.gates]]"));
        assert!(serialized.contains("can_force_push = \"deny\""));
    }

    // ── AI config tests ─────────────────────────────────────────────────

    #[test]
    fn default_ai_config_enabled() {
        let config = AiConfig::default();
        assert!(config.enabled);
    }

    #[test]
    fn default_config_ai_enabled() {
        let config = SwebashConfig::default();
        assert!(config.ai.enabled);
    }

    #[test]
    fn deserialize_config_with_ai_disabled() {
        let toml_str = r#"
[ai]
enabled = false

[workspace]
root = "~/workspace"
"#;
        let config: SwebashConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.ai.enabled);
    }

    #[test]
    fn deserialize_legacy_config_without_ai_section() {
        // Old configs without [ai] section should default to enabled
        let toml_str = r#"
[workspace]
root = "~/workspace"
"#;
        let config: SwebashConfig = toml::from_str(toml_str).unwrap();
        assert!(config.ai.enabled);
    }

    #[test]
    fn ai_config_serde_roundtrip() {
        let config = SwebashConfig {
            ai: AiConfig { enabled: false, ..Default::default() },
            ..Default::default()
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        assert!(serialized.contains("[ai]"));
        assert!(serialized.contains("enabled = false"));

        let deserialized: SwebashConfig = toml::from_str(&serialized).unwrap();
        assert!(!deserialized.ai.enabled);
    }

    // ── Workspace binding tests ────────────────────────────────────────────

    #[test]
    fn normalize_path_converts_backslashes() {
        let result = normalize_path("C:\\Users\\test\\project");
        assert_eq!(result, "c:/users/test/project");
    }

    #[test]
    fn normalize_path_preserves_forward_slashes() {
        let result = normalize_path("/home/user/project");
        #[cfg(windows)]
        assert_eq!(result, "/home/user/project");
        #[cfg(not(windows))]
        assert_eq!(result, "/home/user/project");
    }

    #[test]
    fn normalize_remote_https_url() {
        let result = normalize_remote("https://github.com/user/repo.git");
        assert_eq!(result, "github.com/user/repo");
    }

    #[test]
    fn normalize_remote_ssh_url() {
        let result = normalize_remote("git@github.com:user/repo.git");
        assert_eq!(result, "github.com/user/repo");
    }

    #[test]
    fn normalize_remote_removes_trailing_git() {
        let result = normalize_remote("https://github.com/user/repo");
        assert_eq!(result, "github.com/user/repo");
    }

    #[test]
    fn normalize_remote_case_insensitive() {
        let result1 = normalize_remote("https://GitHub.com/User/Repo.git");
        let result2 = normalize_remote("https://github.com/user/repo.git");
        assert_eq!(result1, result2);
    }

    #[test]
    fn bound_workspace_matches_workspace_exact() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        assert!(ws.matches_workspace("/home/user/project"));
    }

    #[test]
    fn bound_workspace_matches_workspace_subdirectory() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        assert!(ws.matches_workspace("/home/user/project/src/main.rs"));
    }

    #[test]
    fn bound_workspace_no_match_different_path() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        assert!(!ws.matches_workspace("/home/user/other"));
    }

    #[test]
    fn bound_workspace_matches_remote_same_url() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        assert!(ws.matches_remote("https://github.com/user/repo.git"));
    }

    #[test]
    fn bound_workspace_matches_remote_ssh_vs_https() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        // SSH format should match HTTPS format
        assert!(ws.matches_remote("git@github.com:user/repo.git"));
    }

    #[test]
    fn bound_workspace_no_match_different_remote() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        assert!(!ws.matches_remote("https://github.com/other/repo.git"));
    }

    #[test]
    fn config_find_workspace_for_path() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        let config = SwebashConfig {
            bound_workspaces: vec![ws],
            ..Default::default()
        };
        assert!(config.find_workspace_for_path("/home/user/project").is_some());
        assert!(config.find_workspace_for_path("/home/user/other").is_none());
    }

    #[test]
    fn config_verify_repo_binding_success() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        let config = SwebashConfig {
            bound_workspaces: vec![ws],
            ..Default::default()
        };
        let result = config.verify_repo_binding(
            "/home/user/project",
            "https://github.com/user/repo.git",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn config_verify_repo_binding_mismatch() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        let config = SwebashConfig {
            bound_workspaces: vec![ws],
            ..Default::default()
        };
        let result = config.verify_repo_binding(
            "/home/user/project",
            "https://github.com/other/repo.git",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mismatch"));
    }

    #[test]
    fn config_multiple_workspaces() {
        let ws1 = BoundWorkspace {
            workspace_path: "/home/user/project1".to_string(),
            repo_remote: "https://github.com/user/repo1.git".to_string(),
            repo_local: "/home/user/project1".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        let ws2 = BoundWorkspace {
            workspace_path: "/home/user/project2".to_string(),
            repo_remote: "https://github.com/user/repo2.git".to_string(),
            repo_local: "/home/user/project2".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        let config = SwebashConfig {
            bound_workspaces: vec![ws1, ws2],
            ..Default::default()
        };

        // Each workspace should find its own binding
        let found1 = config.find_workspace_for_path("/home/user/project1");
        let found2 = config.find_workspace_for_path("/home/user/project2");

        assert!(found1.is_some());
        assert!(found2.is_some());
        assert!(found1.unwrap().matches_remote("https://github.com/user/repo1.git"));
        assert!(found2.unwrap().matches_remote("https://github.com/user/repo2.git"));
    }

    #[test]
    fn bound_workspace_serde_roundtrip() {
        let ws = BoundWorkspace {
            workspace_path: "/home/user/project".to_string(),
            repo_remote: "https://github.com/user/repo.git".to_string(),
            repo_local: "/home/user/project".to_string(),
            bound_at: "2026-01-01T00:00:00Z".to_string(),
            git: None,
        };
        let config = SwebashConfig {
            bound_workspaces: vec![ws],
            ..Default::default()
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        assert!(serialized.contains("[[bound_workspaces]]"));
        assert!(serialized.contains("workspace_path"));
        assert!(serialized.contains("repo_remote"));

        let deserialized: SwebashConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.bound_workspaces.len(), 1);
        assert_eq!(
            deserialized.bound_workspaces[0].repo_remote,
            "https://github.com/user/repo.git"
        );
    }
}
