use std::path::PathBuf;

use serde::Deserialize;

use super::state::{AccessMode, PathRule, SandboxPolicy};

/// Top-level config file structure (`~/.config/swebash/config.toml`).
#[derive(Debug, Deserialize, Default)]
pub struct SwebashConfig {
    #[serde(default)]
    pub workspace: WorkspaceConfig,
}

/// `[workspace]` section of the config.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub struct AllowEntry {
    pub path: String,
    #[serde(default = "default_mode_str")]
    pub mode: String,
}

fn default_workspace_root() -> String {
    "~/workspace".to_string()
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
    /// Convert the deserialized config into a `SandboxPolicy`.
    pub fn into_policy(self) -> SandboxPolicy {
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
