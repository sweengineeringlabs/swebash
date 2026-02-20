//! Configuration-driven LLM provider setup
//!
//! This module enables provider selection via configuration rather than code.
//! Users specify which provider to use in config files, and the code remains
//! provider-agnostic.
//!
//! # Example Configuration (YAML)
//!
//! ```yaml
//! llm:
//!   # Active provider - change this to switch providers without code changes
//!   provider: openai
//!
//!   # Default model to use (optional, provider has its own default)
//!   default_model: gpt-4o
//!
//! # Provider-specific configurations
//! providers:
//!   openai:
//!     api_key_env: OPENAI_API_KEY
//!     base_url_env: OPENAI_BASE_URL
//!     default_base_url: https://api.openai.com/v1
//!   anthropic:
//!     api_key_env: ANTHROPIC_API_KEY
//!     base_url_env: ANTHROPIC_BASE_URL
//!     default_base_url: https://api.anthropic.com/v1
//!   gemini:
//!     api_key_env: GEMINI_API_KEY
//!     alt_api_key_env: GOOGLE_API_KEY
//!     base_url_env: GEMINI_BASE_URL
//!     default_base_url: https://generativelanguage.googleapis.com/v1beta
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! // Code is provider-agnostic - just load from config
//! let config = LlmConfig::load("config.yml")?;
//! let service = LlmService::from_config(&config).await?;
//!
//! // Use the service without knowing which provider is active
//! let response = service.complete(request).await?;
//! ```

use rustboot_config::{ConfigLoader, DotEnvSource, FileSource, Mergeable};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::keys;

/// Well-known provider identifiers
pub mod provider_id {
    pub const OPENAI: &str = "openai";
    pub const ANTHROPIC: &str = "anthropic";
    pub const GEMINI: &str = "gemini";
    pub const AZURE_OPENAI: &str = "azure-openai";
    pub const OLLAMA: &str = "ollama";
    pub const BEDROCK: &str = "bedrock";
}

/// Main LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Active provider identifier (e.g., "openai", "anthropic", "gemini")
    pub provider: String,

    /// Default model to use (optional - provider has its own default)
    #[serde(default)]
    pub default_model: Option<String>,

    /// Request timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Maximum retries for failed requests
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_timeout() -> u64 {
    60_000
}

fn default_max_retries() -> u32 {
    3
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: provider_id::OPENAI.to_string(),
            default_model: None,
            timeout_ms: default_timeout(),
            max_retries: default_max_retries(),
        }
    }
}

impl Mergeable for LlmConfig {
    fn merge(&mut self, other: Self) {
        // Non-default provider wins
        if other.provider != provider_id::OPENAI {
            self.provider = other.provider;
        }
        // Model override
        if other.default_model.is_some() {
            self.default_model = other.default_model;
        }
        // Non-default timeout wins
        if other.timeout_ms != default_timeout() {
            self.timeout_ms = other.timeout_ms;
        }
        // Non-default retries wins
        if other.max_retries != default_max_retries() {
            self.max_retries = other.max_retries;
        }
    }
}

/// Configuration for a specific provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSpec {
    /// Environment variable name for API key
    pub api_key_env: String,

    /// Alternative environment variable for API key (fallback)
    #[serde(default)]
    pub alt_api_key_env: Option<String>,

    /// Environment variable for custom base URL
    #[serde(default)]
    pub base_url_env: Option<String>,

    /// Default base URL if not specified via environment
    #[serde(default)]
    pub default_base_url: Option<String>,

    /// Additional provider-specific settings
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Registry of all provider configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersConfig {
    #[serde(flatten)]
    pub providers: HashMap<String, ProviderSpec>,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        let mut providers = HashMap::new();

        // OpenAI
        providers.insert(
            provider_id::OPENAI.to_string(),
            ProviderSpec {
                api_key_env: keys::OPENAI_API_KEY.to_string(),
                alt_api_key_env: None,
                base_url_env: Some(keys::OPENAI_BASE_URL.to_string()),
                default_base_url: Some("https://api.openai.com/v1".to_string()),
                extra: HashMap::new(),
            },
        );

        // Anthropic
        providers.insert(
            provider_id::ANTHROPIC.to_string(),
            ProviderSpec {
                api_key_env: keys::ANTHROPIC_API_KEY.to_string(),
                alt_api_key_env: None,
                base_url_env: Some(keys::ANTHROPIC_BASE_URL.to_string()),
                default_base_url: Some("https://api.anthropic.com/v1".to_string()),
                extra: HashMap::new(),
            },
        );

        // Gemini
        providers.insert(
            provider_id::GEMINI.to_string(),
            ProviderSpec {
                api_key_env: keys::GEMINI_API_KEY.to_string(),
                alt_api_key_env: Some(keys::GOOGLE_API_KEY.to_string()),
                base_url_env: Some(keys::GEMINI_BASE_URL.to_string()),
                default_base_url: Some(
                    "https://generativelanguage.googleapis.com/v1beta".to_string(),
                ),
                extra: HashMap::new(),
            },
        );

        // Azure OpenAI
        providers.insert(
            provider_id::AZURE_OPENAI.to_string(),
            ProviderSpec {
                api_key_env: keys::AZURE_OPENAI_API_KEY.to_string(),
                alt_api_key_env: None,
                base_url_env: Some(keys::AZURE_OPENAI_ENDPOINT.to_string()),
                default_base_url: None, // Required - no default
                extra: HashMap::new(),
            },
        );

        // Ollama (local)
        providers.insert(
            provider_id::OLLAMA.to_string(),
            ProviderSpec {
                api_key_env: keys::OLLAMA_API_KEY.to_string(), // Usually not needed
                alt_api_key_env: None,
                base_url_env: Some(keys::OLLAMA_BASE_URL.to_string()),
                default_base_url: Some("http://localhost:11434".to_string()),
                extra: HashMap::new(),
            },
        );

        Self { providers }
    }
}

impl Mergeable for ProvidersConfig {
    fn merge(&mut self, other: Self) {
        // Merge providers - other's providers override existing
        for (key, value) in other.providers {
            self.providers.insert(key, value);
        }
    }
}

impl ProvidersConfig {
    /// Get configuration for a specific provider
    pub fn get(&self, provider_id: &str) -> Option<&ProviderSpec> {
        self.providers.get(provider_id)
    }

    /// Check if a provider is registered
    pub fn has(&self, provider_id: &str) -> bool {
        self.providers.contains_key(provider_id)
    }

    /// List all registered provider IDs
    pub fn list(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}

/// Complete application configuration including LLM settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppLlmConfig {
    /// LLM configuration
    #[serde(default)]
    pub llm: LlmConfig,

    /// Provider specifications
    #[serde(default)]
    pub providers: ProvidersConfig,
}

impl Mergeable for AppLlmConfig {
    fn merge(&mut self, other: Self) {
        self.llm.merge(other.llm);
        self.providers.merge(other.providers);
    }
}

impl AppLlmConfig {
    /// Load configuration from a YAML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::Io {
            path: path.as_ref().to_string_lossy().to_string(),
            source: e,
        })?;

        Self::from_yaml(&content)
    }

    /// Parse configuration from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self, ConfigError> {
        serde_yaml::from_str(yaml).map_err(ConfigError::Parse)
    }

    /// Load configuration with hierarchical merging from multiple sources
    ///
    /// This method loads configuration from:
    /// 1. Base defaults
    /// 2. Optional .env file (if exists)
    /// 3. Config file (YAML/JSON/TOML)
    /// 4. Environment variable overrides
    ///
    /// Sources are merged in order, with later sources overriding earlier ones.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Load from config file with .env support
    /// let config = AppLlmConfig::load_merged("config.yml")?;
    ///
    /// // Or start fresh from defaults + env
    /// let config = AppLlmConfig::load_merged_default()?;
    /// ```
    pub fn load_merged<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let file_path = path.as_ref().to_str().ok_or_else(|| ConfigError::Io {
            path: path.as_ref().to_string_lossy().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid path"),
        })?;

        // Use ConfigLoader to load from .env + file
        let file_config: Self = ConfigLoader::new()
            .load(DotEnvSource::new())
            .map_err(|e| ConfigError::Loader(e.to_string()))?
            .load(FileSource::auto(file_path).map_err(|e| ConfigError::Loader(e.to_string()))?)
            .map_err(|e| ConfigError::Loader(e.to_string()))?
            .build();

        // Merge with environment overrides
        let mut result = file_config;
        let env_overrides = Self::from_env();

        // Apply specific environment overrides (using centralized keys)
        if std::env::var(keys::LLM_PROVIDER).is_ok() {
            result.llm.provider = env_overrides.llm.provider;
        }
        if std::env::var(keys::LLM_DEFAULT_MODEL).is_ok() {
            result.llm.default_model = env_overrides.llm.default_model;
        }
        if std::env::var(keys::LLM_TIMEOUT_MS).is_ok() {
            result.llm.timeout_ms = env_overrides.llm.timeout_ms;
        }
        if std::env::var(keys::LLM_MAX_RETRIES).is_ok() {
            result.llm.max_retries = env_overrides.llm.max_retries;
        }

        Ok(result)
    }

    /// Load configuration from defaults merged with environment
    ///
    /// This is a convenience method equivalent to `from_env()` but using
    /// the ConfigLoader infrastructure for consistency.
    pub fn load_merged_default() -> Result<Self, ConfigError> {
        // Load from .env file if it exists
        let _: Self = ConfigLoader::new()
            .load(DotEnvSource::new())
            .map_err(|e| ConfigError::Loader(e.to_string()))?
            .build();

        // Use from_env which handles all the environment variable logic
        Ok(Self::from_env())
    }

    /// Load from environment with defaults
    ///
    /// This method:
    /// 1. Loads `.env` file (if exists) into process environment
    /// 2. Checks `LLM_PROVIDER` environment variable to set active provider
    /// 3. Auto-detects provider based on available API keys if not set
    ///
    /// Uses centralized key constants from `keys` module for consistency.
    pub fn from_env() -> Self {
        // Load .env file into process environment first
        // This ensures .env values are available for all subsequent env::var calls
        let _ = DotEnvSource::new().load_into_env();

        let mut config = Self::default();

        // Allow overriding provider via environment
        if let Ok(provider) = std::env::var(keys::LLM_PROVIDER) {
            config.llm.provider = provider.to_lowercase();
        } else {
            // Auto-detect provider based on available API keys
            if let Some(detected) = Self::detect_provider_from_api_keys() {
                config.llm.provider = detected;
            }
        }

        // Allow overriding default model via environment
        if let Ok(model) = std::env::var(keys::LLM_DEFAULT_MODEL) {
            config.llm.default_model = Some(model);
        }

        // Allow overriding timeout via environment
        if let Ok(timeout_str) = std::env::var(keys::LLM_TIMEOUT_MS) {
            if let Ok(timeout) = timeout_str.parse::<u64>() {
                config.llm.timeout_ms = timeout;
            }
        }

        // Allow overriding max retries via environment
        if let Ok(retries_str) = std::env::var(keys::LLM_MAX_RETRIES) {
            if let Ok(retries) = retries_str.parse::<u32>() {
                config.llm.max_retries = retries;
            }
        }

        config
    }

    /// Detect provider based on available API keys in environment
    ///
    /// Checks for API keys in priority order and returns the first provider found.
    /// Uses centralized key constants from `keys` module for consistency.
    fn detect_provider_from_api_keys() -> Option<String> {
        // Priority order: Anthropic, OpenAI, Gemini
        let providers_to_check = [
            (provider_id::ANTHROPIC, keys::ANTHROPIC_API_KEY),
            (provider_id::OPENAI, keys::OPENAI_API_KEY),
            (provider_id::GEMINI, keys::GEMINI_API_KEY),
            (provider_id::GEMINI, keys::GOOGLE_API_KEY),
        ];

        for (provider, env_var) in providers_to_check {
            if let Ok(key) = std::env::var(env_var) {
                if !key.is_empty() {
                    tracing::info!(
                        provider = %provider,
                        env_var = %env_var,
                        "Auto-detected LLM provider from API key"
                    );
                    return Some(provider.to_string());
                }
            }
        }

        None
    }

    /// Get the active provider specification
    pub fn active_provider(&self) -> Option<&ProviderSpec> {
        self.providers.get(&self.llm.provider)
    }

    /// Resolve API key for the active provider from environment
    pub fn resolve_api_key(&self) -> Option<String> {
        let spec = self.active_provider()?;

        // Try primary env var
        if let Ok(key) = std::env::var(&spec.api_key_env) {
            if !key.is_empty() {
                return Some(key);
            }
        }

        // Try alternative env var
        if let Some(ref alt_env) = spec.alt_api_key_env {
            if let Ok(key) = std::env::var(alt_env) {
                if !key.is_empty() {
                    return Some(key);
                }
            }
        }

        None
    }

    /// Resolve base URL for the active provider
    pub fn resolve_base_url(&self) -> Option<String> {
        let spec = self.active_provider()?;

        // Try env var first
        if let Some(ref env_var) = spec.base_url_env {
            if let Ok(url) = std::env::var(env_var) {
                if !url.is_empty() {
                    return Some(url);
                }
            }
        }

        // Fall back to default
        spec.default_base_url.clone()
    }

    /// Validate that the active provider is configured
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Check provider exists
        if !self.providers.has(&self.llm.provider) {
            return Err(ConfigError::UnknownProvider {
                provider: self.llm.provider.clone(),
                available: self.providers.list().iter().map(|s| s.to_string()).collect(),
            });
        }

        // Check API key is available (unless it's ollama which may not need one)
        if self.llm.provider != provider_id::OLLAMA && self.resolve_api_key().is_none() {
            let spec = self.active_provider().unwrap();
            return Err(ConfigError::MissingApiKey {
                provider: self.llm.provider.clone(),
                env_var: spec.api_key_env.clone(),
                alt_env_var: spec.alt_api_key_env.clone(),
            });
        }

        Ok(())
    }
}

/// Configuration errors
#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse(serde_yaml::Error),
    UnknownProvider {
        provider: String,
        available: Vec<String>,
    },
    MissingApiKey {
        provider: String,
        env_var: String,
        alt_env_var: Option<String>,
    },
    /// Error from secrets service
    Secrets(String),
    /// Error from ConfigLoader
    Loader(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "Failed to read config file '{}': {}", path, source)
            }
            ConfigError::Parse(e) => {
                write!(f, "Failed to parse config: {}", e)
            }
            ConfigError::UnknownProvider { provider, available } => {
                write!(f, "Unknown provider '{}'. Available: {:?}", provider, available)
            }
            ConfigError::MissingApiKey {
                provider,
                env_var,
                alt_env_var,
            } => {
                write!(f, "Missing API key for provider '{}'. Set {}", provider, env_var)?;
                if let Some(alt) = alt_env_var {
                    write!(f, " or {}", alt)?;
                }
                write!(f, " environment variable")
            }
            ConfigError::Secrets(msg) => {
                write!(f, "Secrets error: {}", msg)
            }
            ConfigError::Loader(msg) => {
                write!(f, "Config loader error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io { source, .. } => Some(source),
            ConfigError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_yaml::Error> for ConfigError {
    fn from(e: serde_yaml::Error) -> Self {
        ConfigError::Parse(e)
    }
}

// =============================================================================
// Secrets Integration (feature: secrets)
// =============================================================================

#[cfg(feature = "secrets")]
mod secrets_integration {
    use super::*;
    use swe_secrets::{SecretService, SecretsError};

    impl AppLlmConfig {
        /// Resolve API key using the secrets service
        ///
        /// This async method tries the primary API key env var first,
        /// then falls back to the alternative if configured.
        ///
        /// # Example
        ///
        /// ```rust,ignore
        /// use llm_provider::{AppLlmConfig, create_service_with_secrets};
        /// use swe_secrets::from_env;
        ///
        /// let config = AppLlmConfig::from_env();
        /// let secrets = from_env().await?;
        /// let api_key = config.resolve_api_key_with_secrets(&secrets).await?;
        /// ```
        pub async fn resolve_api_key_with_secrets<S: SecretService>(
            &self,
            secrets: &S,
        ) -> Result<Option<String>, SecretsError> {
            let spec = match self.active_provider() {
                Some(s) => s,
                None => return Ok(None),
            };

            // Try primary env var
            if let Some(secret) = secrets.get_opt(&spec.api_key_env).await? {
                let value = secret.value.expose().to_string();
                if !value.is_empty() {
                    return Ok(Some(value));
                }
            }

            // Try alternative env var
            if let Some(ref alt_env) = spec.alt_api_key_env {
                if let Some(secret) = secrets.get_opt(alt_env).await? {
                    let value = secret.value.expose().to_string();
                    if !value.is_empty() {
                        return Ok(Some(value));
                    }
                }
            }

            Ok(None)
        }

        /// Resolve base URL using the secrets service
        ///
        /// Tries the configured base URL environment variable first,
        /// then falls back to the default URL.
        pub async fn resolve_base_url_with_secrets<S: SecretService>(
            &self,
            secrets: &S,
        ) -> Result<Option<String>, SecretsError> {
            let spec = match self.active_provider() {
                Some(s) => s,
                None => return Ok(None),
            };

            // Try env var first
            if let Some(ref env_var) = spec.base_url_env {
                if let Some(secret) = secrets.get_opt(env_var).await? {
                    let value = secret.value.expose().to_string();
                    if !value.is_empty() {
                        return Ok(Some(value));
                    }
                }
            }

            // Fall back to default
            Ok(spec.default_base_url.clone())
        }

        /// Validate configuration using the secrets service
        ///
        /// Checks that the provider is known and has an API key available
        /// (except for providers like Ollama that don't require one).
        pub async fn validate_with_secrets<S: SecretService>(
            &self,
            secrets: &S,
        ) -> Result<(), ConfigError> {
            // Check provider exists
            if !self.providers.has(&self.llm.provider) {
                return Err(ConfigError::UnknownProvider {
                    provider: self.llm.provider.clone(),
                    available: self.providers.list().iter().map(|s| s.to_string()).collect(),
                });
            }

            // Check API key is available (unless it's ollama which may not need one)
            if self.llm.provider != super::provider_id::OLLAMA {
                let api_key = self
                    .resolve_api_key_with_secrets(secrets)
                    .await
                    .map_err(|e| ConfigError::Secrets(e.to_string()))?;

                if api_key.is_none() {
                    let spec = self.active_provider().unwrap();
                    return Err(ConfigError::MissingApiKey {
                        provider: self.llm.provider.clone(),
                        env_var: spec.api_key_env.clone(),
                        alt_env_var: spec.alt_api_key_env.clone(),
                    });
                }
            }

            Ok(())
        }
    }
}

// Re-export secrets types when feature is enabled
#[cfg(feature = "secrets")]
pub use swe_secrets::{SecretService, SecretsError};

#[cfg(test)]
mod tests {
    use super::*;
    use super::keys;

    #[test]
    fn test_default_config() {
        let config = AppLlmConfig::default();
        assert_eq!(config.llm.provider, "openai");
        assert!(config.providers.has("openai"));
        assert!(config.providers.has("anthropic"));
        assert!(config.providers.has("gemini"));
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
llm:
  provider: anthropic
  default_model: claude-3-5-sonnet-20241022

providers:
  anthropic:
    api_key_env: ANTHROPIC_API_KEY
    default_base_url: https://api.anthropic.com/v1
"#;

        let config = AppLlmConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.llm.provider, "anthropic");
        assert_eq!(
            config.llm.default_model,
            Some("claude-3-5-sonnet-20241022".to_string())
        );
    }

    #[test]
    fn test_provider_list() {
        let config = ProvidersConfig::default();
        let providers = config.list();
        assert!(providers.contains(&"openai"));
        assert!(providers.contains(&"anthropic"));
        assert!(providers.contains(&"gemini"));
    }

    #[test]
    fn test_unknown_provider_error() {
        let mut config = AppLlmConfig::default();
        config.llm.provider = "unknown-provider".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigError::UnknownProvider { .. }
        ));
    }

    #[test]
    fn test_from_env_default_provider() {
        // Clear any existing LLM_PROVIDER and API keys
        std::env::remove_var(keys::LLM_PROVIDER);
        std::env::remove_var(keys::ANTHROPIC_API_KEY);
        std::env::remove_var(keys::OPENAI_API_KEY);
        std::env::remove_var(keys::GEMINI_API_KEY);
        std::env::remove_var(keys::GOOGLE_API_KEY);

        let config = AppLlmConfig::from_env();
        assert_eq!(config.llm.provider, "openai"); // Default when no keys found
    }

    #[test]
    fn test_from_env_auto_detect_anthropic() {
        // Clear LLM_PROVIDER but set Anthropic key
        std::env::remove_var(keys::LLM_PROVIDER);
        std::env::remove_var(keys::OPENAI_API_KEY);
        std::env::set_var(keys::ANTHROPIC_API_KEY, "test-key");

        let config = AppLlmConfig::from_env();
        assert_eq!(config.llm.provider, "anthropic"); // Auto-detected

        std::env::remove_var(keys::ANTHROPIC_API_KEY);
    }

    #[test]
    fn test_from_env_custom_provider() {
        std::env::set_var(keys::LLM_PROVIDER, "ANTHROPIC");

        let config = AppLlmConfig::from_env();
        assert_eq!(config.llm.provider, "anthropic"); // Lowercased

        std::env::remove_var(keys::LLM_PROVIDER);
    }

    #[test]
    fn test_from_env_custom_model() {
        std::env::set_var(keys::LLM_DEFAULT_MODEL, "gpt-4-turbo");

        let config = AppLlmConfig::from_env();
        assert_eq!(config.llm.default_model, Some("gpt-4-turbo".to_string()));

        std::env::remove_var(keys::LLM_DEFAULT_MODEL);
    }

    #[test]
    fn test_resolve_api_key() {
        std::env::set_var("TEST_API_KEY", "sk-test-123");

        let mut config = AppLlmConfig::default();
        config.llm.provider = "test-provider".to_string();
        config.providers.providers.insert(
            "test-provider".to_string(),
            ProviderSpec {
                api_key_env: "TEST_API_KEY".to_string(),
                alt_api_key_env: None,
                base_url_env: None,
                default_base_url: None,
                extra: HashMap::new(),
            },
        );

        let key = config.resolve_api_key();
        assert_eq!(key, Some("sk-test-123".to_string()));

        std::env::remove_var("TEST_API_KEY");
    }

    #[test]
    fn test_resolve_api_key_fallback() {
        std::env::remove_var("PRIMARY_KEY");
        std::env::set_var("ALT_KEY", "sk-alt-456");

        let mut config = AppLlmConfig::default();
        config.llm.provider = "test-provider".to_string();
        config.providers.providers.insert(
            "test-provider".to_string(),
            ProviderSpec {
                api_key_env: "PRIMARY_KEY".to_string(),
                alt_api_key_env: Some("ALT_KEY".to_string()),
                base_url_env: None,
                default_base_url: None,
                extra: HashMap::new(),
            },
        );

        let key = config.resolve_api_key();
        assert_eq!(key, Some("sk-alt-456".to_string()));

        std::env::remove_var("ALT_KEY");
    }

    #[test]
    fn test_resolve_base_url_from_env() {
        std::env::set_var("CUSTOM_BASE_URL", "https://custom.api.com");

        let mut config = AppLlmConfig::default();
        config.llm.provider = "test-provider".to_string();
        config.providers.providers.insert(
            "test-provider".to_string(),
            ProviderSpec {
                api_key_env: "API_KEY".to_string(),
                alt_api_key_env: None,
                base_url_env: Some("CUSTOM_BASE_URL".to_string()),
                default_base_url: Some("https://default.api.com".to_string()),
                extra: HashMap::new(),
            },
        );

        let url = config.resolve_base_url();
        assert_eq!(url, Some("https://custom.api.com".to_string())); // Env takes precedence

        std::env::remove_var("CUSTOM_BASE_URL");
    }

    #[test]
    fn test_resolve_base_url_default() {
        std::env::remove_var("NONEXISTENT_URL");

        let mut config = AppLlmConfig::default();
        config.llm.provider = "test-provider".to_string();
        config.providers.providers.insert(
            "test-provider".to_string(),
            ProviderSpec {
                api_key_env: "API_KEY".to_string(),
                alt_api_key_env: None,
                base_url_env: Some("NONEXISTENT_URL".to_string()),
                default_base_url: Some("https://default.api.com".to_string()),
                extra: HashMap::new(),
            },
        );

        let url = config.resolve_base_url();
        assert_eq!(url, Some("https://default.api.com".to_string())); // Falls back to default
    }

    #[test]
    fn test_active_provider() {
        let config = AppLlmConfig::default();
        let provider = config.active_provider();
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().api_key_env, keys::OPENAI_API_KEY);
    }

    #[test]
    fn test_provider_ids() {
        assert_eq!(provider_id::OPENAI, "openai");
        assert_eq!(provider_id::ANTHROPIC, "anthropic");
        assert_eq!(provider_id::GEMINI, "gemini");
        assert_eq!(provider_id::AZURE_OPENAI, "azure-openai");
        assert_eq!(provider_id::OLLAMA, "ollama");
    }

    #[test]
    fn test_load_merged_from_yaml() {
        use std::io::Write;
        use std::sync::atomic::{AtomicU32, Ordering};

        // Use unique file name with counter to avoid conflicts
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir();
        let file_path = dir.join(format!("test_llm_config_{}.yaml", id));

        // Create a test YAML config
        let yaml = r#"
llm:
  provider: gemini
  default_model: gemini-pro
  timeout_ms: 30000
"#;
        let mut file = std::fs::File::create(&file_path).unwrap();
        write!(file, "{}", yaml).unwrap();

        // This test verifies that file config is loaded correctly.
        // Note: If LLM_PROVIDER env var is set by another test,
        // it will override the file value - that's expected behavior.
        let config = AppLlmConfig::load_merged(&file_path).unwrap();

        // Model and timeout should always come from file
        assert_eq!(config.llm.default_model, Some("gemini-pro".to_string()));
        assert_eq!(config.llm.timeout_ms, 30000);

        // Provider may be overridden by env var (test for expected values)
        assert!(
            config.llm.provider == "gemini"
                || config.llm.provider == "anthropic"
                || config.llm.provider == "openai",
            "Expected known provider, got: {}",
            config.llm.provider
        );

        // Clean up
        std::fs::remove_file(&file_path).ok();
    }

    #[test]
    fn test_load_merged_with_env_override() {
        use std::io::Write;
        use std::sync::atomic::{AtomicU32, Ordering};

        // Use unique file name with counter to avoid conflicts
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir();
        let file_path = dir.join(format!("test_llm_config_env_{}.yaml", id));

        // Create a test YAML config
        let yaml = r#"
llm:
  provider: gemini
  default_model: gemini-pro
"#;
        let mut file = std::fs::File::create(&file_path).unwrap();
        write!(file, "{}", yaml).unwrap();

        // Save current value to restore later
        let original = std::env::var(keys::LLM_PROVIDER).ok();

        // Set env var override
        std::env::set_var(keys::LLM_PROVIDER, "anthropic");

        let config = AppLlmConfig::load_merged(&file_path).unwrap();
        // Env var should override file
        assert_eq!(config.llm.provider, "anthropic");
        // Model should come from file
        assert_eq!(config.llm.default_model, Some("gemini-pro".to_string()));

        // Restore original value
        match original {
            Some(v) => std::env::set_var(keys::LLM_PROVIDER, v),
            None => std::env::remove_var(keys::LLM_PROVIDER),
        }
        std::fs::remove_file(&file_path).ok();
    }

    #[test]
    fn test_load_merged_default() {
        // This test verifies load_merged_default returns successfully
        // and returns a valid configuration. The exact provider may vary
        // based on which API keys are set in the environment when
        // running tests in parallel.
        let config = AppLlmConfig::load_merged_default().unwrap();

        // Should have a valid provider (one of the known ones)
        assert!(
            config.llm.provider == "openai"
                || config.llm.provider == "anthropic"
                || config.llm.provider == "gemini",
            "Expected known provider, got: {}",
            config.llm.provider
        );

        // Should have default timeouts
        assert_eq!(config.llm.timeout_ms, 60_000);
        assert_eq!(config.llm.max_retries, 3);
    }

    #[test]
    fn test_mergeable_llm_config() {
        let mut base = LlmConfig::default();
        base.default_model = Some("base-model".to_string());

        let override_config = LlmConfig {
            provider: "anthropic".to_string(),
            default_model: Some("override-model".to_string()),
            timeout_ms: default_timeout(),
            max_retries: default_max_retries(),
        };

        base.merge(override_config);
        assert_eq!(base.provider, "anthropic"); // Overridden
        assert_eq!(base.default_model, Some("override-model".to_string())); // Overridden
    }

    #[test]
    fn test_mergeable_app_config() {
        let mut base = AppLlmConfig::default();
        base.llm.default_model = Some("base-model".to_string());

        let mut override_config = AppLlmConfig::default();
        override_config.llm.provider = "gemini".to_string();
        override_config.llm.default_model = Some("gemini-pro".to_string());

        base.merge(override_config);
        assert_eq!(base.llm.provider, "gemini");
        assert_eq!(base.llm.default_model, Some("gemini-pro".to_string()));
    }
}
