use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub git: Option<GitConfig>,
    /// Whether the first-run setup wizard has been completed.
    #[serde(default)]
    pub setup_completed: bool,
}

/// `[ai]` section of the config.
#[derive(Debug, Serialize, Deserialize)]
pub struct AiConfig {
    /// Master switch to enable/disable AI features. Default: `true`.
    /// When `false`, AI mode is completely disabled regardless of other settings.
    /// Can also be controlled via `SWEBASH_AI_ENABLED` env var (env takes precedence).
    #[serde(default = "default_ai_enabled")]
    pub enabled: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: default_ai_enabled(),
        }
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
                eprintln!(
                    "warning: failed to parse {}: {e}",
                    config_path.display()
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
            ai: AiConfig { enabled: false },
            ..Default::default()
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        assert!(serialized.contains("[ai]"));
        assert!(serialized.contains("enabled = false"));

        let deserialized: SwebashConfig = toml::from_str(&serialized).unwrap();
        assert!(!deserialized.ai.enabled);
    }
}
