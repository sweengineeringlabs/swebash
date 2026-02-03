use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadlineConfig {
    #[serde(default = "default_edit_mode")]
    pub edit_mode: EditMode,

    #[serde(default = "default_max_history")]
    pub max_history_size: usize,

    #[serde(default = "default_true")]
    pub history_ignore_space: bool,

    #[serde(default = "default_true")]
    pub enable_completion: bool,

    #[serde(default = "default_true")]
    pub enable_highlighting: bool,

    #[serde(default = "default_true")]
    pub enable_hints: bool,

    #[serde(default)]
    pub colors: ColorConfig,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EditMode {
    Emacs,
    Vi,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColorConfig {
    #[serde(default = "default_green")]
    pub builtin_command: String,

    #[serde(default = "default_blue")]
    pub external_command: String,

    #[serde(default = "default_red")]
    pub invalid_command: String,

    #[serde(default = "default_yellow")]
    pub string: String,

    #[serde(default = "default_cyan")]
    pub path: String,

    #[serde(default = "default_magenta")]
    pub operator: String,

    #[serde(default = "default_gray")]
    pub hint: String,
}

impl Default for ReadlineConfig {
    fn default() -> Self {
        Self {
            edit_mode: EditMode::Emacs,
            max_history_size: 1000,
            history_ignore_space: true,
            enable_completion: true,
            enable_highlighting: true,
            enable_hints: true,
            colors: ColorConfig::default(),
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            builtin_command: "green".to_string(),
            external_command: "blue".to_string(),
            invalid_command: "red".to_string(),
            string: "yellow".to_string(),
            path: "cyan".to_string(),
            operator: "magenta".to_string(),
            hint: "gray".to_string(),
        }
    }
}

impl ReadlineConfig {
    /// Load configuration from file
    pub fn load() -> Self {
        let config_path = std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(dirs::home_dir)
            .map(|h| h.join(".swebashrc"))
            .unwrap_or_else(|| PathBuf::from(".swebashrc"));

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str::<SwebashRcFile>(&content) {
                return config.readline;
            }
        }

        Self::default()
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct SwebashRcFile {
    #[serde(default)]
    readline: ReadlineConfig,
}

// Default functions for serde
fn default_edit_mode() -> EditMode {
    EditMode::Emacs
}

fn default_max_history() -> usize {
    1000
}

fn default_true() -> bool {
    true
}

fn default_green() -> String {
    "green".to_string()
}

fn default_blue() -> String {
    "blue".to_string()
}

fn default_red() -> String {
    "red".to_string()
}

fn default_yellow() -> String {
    "yellow".to_string()
}

fn default_cyan() -> String {
    "cyan".to_string()
}

fn default_magenta() -> String {
    "magenta".to_string()
}

fn default_gray() -> String {
    "gray".to_string()
}

impl ColorConfig {
    /// Convert color name to ANSI code
    pub fn to_ansi(&self, color_name: &str) -> &'static str {
        match color_name {
            "black" => "\x1b[30m",
            "red" => "\x1b[31m",
            "green" => "\x1b[32m",
            "yellow" => "\x1b[33m",
            "blue" => "\x1b[34m",
            "magenta" => "\x1b[35m",
            "cyan" => "\x1b[36m",
            "white" => "\x1b[37m",
            "gray" | "grey" => "\x1b[90m",
            _ => "\x1b[0m", // Reset
        }
    }

    pub fn builtin_ansi(&self) -> &'static str {
        self.to_ansi(&self.builtin_command)
    }

    pub fn external_ansi(&self) -> &'static str {
        self.to_ansi(&self.external_command)
    }

    pub fn invalid_ansi(&self) -> &'static str {
        self.to_ansi(&self.invalid_command)
    }

    pub fn string_ansi(&self) -> &'static str {
        self.to_ansi(&self.string)
    }

    pub fn path_ansi(&self) -> &'static str {
        self.to_ansi(&self.path)
    }

    pub fn operator_ansi(&self) -> &'static str {
        self.to_ansi(&self.operator)
    }

    pub fn hint_ansi(&self) -> &'static str {
        self.to_ansi(&self.hint)
    }
}
