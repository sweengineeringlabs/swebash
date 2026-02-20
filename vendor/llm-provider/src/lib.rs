//! LLM Provider - Unified LLM client
//!
//! This crate provides a unified interface to multiple LLM providers
//! (OpenAI, Anthropic, Gemini).
//!
//! # Configuration-Driven Design
//!
//! Provider selection is driven by configuration, not code:
//!
//! ```bash
//! export LLM_PROVIDER=openai
//! export LLM_DEFAULT_MODEL=gpt-4o
//! ```
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use llm_provider::{create_service, CompletionBuilder};
//!
//! let service = create_service().await?;
//! let response = CompletionBuilder::new("gpt-4o")
//!     .system("You are a helpful assistant.")
//!     .user("Hello!")
//!     .execute(&service)
//!     .await?;
//! ```

use std::sync::Arc;

// =============================================================================
// Internal Modules
// =============================================================================

mod config;
mod spi;
mod api;
mod core;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

// =============================================================================
// Public API - Types & Errors (from api/)
// =============================================================================

pub use api::{
    // Types
    CompletionRequest, CompletionResponse, Message, MessageContent, Role,
    ModelInfo, TokenUsage, FinishReason, ToolCall, ToolChoice, ToolDefinition, ContentPart,
    ImageUrl, StreamChunk, StreamDelta, ToolCallDelta, CacheControl, CacheableMessage,
    // Errors
    LlmError, LlmResult,
    // Service
    LlmService, CompletionStream, CompletionBuilder,
};

// =============================================================================
// Public API - Configuration
// =============================================================================

pub use config::{ProviderConfig, AppLlmConfig, LlmConfig, ProviderSpec, ProvidersConfig};
pub use config::app::provider_id;
pub use config::keys;

// =============================================================================
// Public API - Provider Traits & Implementations (from spi/)
// =============================================================================

pub use spi::{LlmProvider, AsyncLlmProvider};

#[cfg(feature = "openai")]
pub use spi::OpenAiProvider;

#[cfg(feature = "anthropic")]
pub use spi::AnthropicProvider;

#[cfg(feature = "gemini")]
pub use spi::GeminiProvider;

// =============================================================================
// Public API - Service Implementation (from core/)
// =============================================================================

pub use core::DefaultLlmService;

// =============================================================================
// Public API - Resilience Patterns (from core/)
// =============================================================================

pub use core::{with_retry, with_retry_config};
pub use core::{
    acquire_rate_limit, global_limiter, try_acquire_rate_limit,
    RateLimitConfig, RateLimiter,
};
pub use core::{
    global_metrics, init_global_metrics, LlmMetrics, MetricsTimer,
};
pub use core::{
    ContextConfig, ContextConfigBuilder, ContextValidator, ValidationResult,
};

// =============================================================================
// Factory Functions
// =============================================================================

/// Create a default LLM service with provider selected from configuration
///
/// The active provider is determined by the `LLM_PROVIDER` environment variable.
/// If not set, defaults to "openai". The code remains provider-agnostic.
///
/// Supported providers:
/// - `openai` - Uses `OPENAI_API_KEY`
/// - `anthropic` - Uses `ANTHROPIC_API_KEY`
/// - `gemini` - Uses `GEMINI_API_KEY` or `GOOGLE_API_KEY`
///
/// # Example
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Set LLM_PROVIDER=anthropic to use Anthropic without code changes
/// let service = llm_provider::create_service().await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_service() -> LlmResult<DefaultLlmService> {
    let config = AppLlmConfig::from_env();
    create_service_from_config(&config).await
}

/// Create a service from explicit configuration
///
/// Use this for programmatic configuration or when loading from a config file.
///
/// # Example
/// ```no_run
/// use llm_provider::{create_service_from_config, AppLlmConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = AppLlmConfig::load("config.yml")?;
/// let service = create_service_from_config(&config).await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_service_from_config(config: &AppLlmConfig) -> LlmResult<DefaultLlmService> {
    let service = DefaultLlmService::new();
    let provider_name = &config.llm.provider;

    tracing::debug!("Creating LLM service with provider: {}", provider_name);

    // Register only the configured provider
    match provider_name.as_str() {
        #[cfg(feature = "openai")]
        provider_id::OPENAI => {
            if let Some(api_key) = config.resolve_api_key() {
                let mut provider_config = ProviderConfig::default();
                provider_config.api_key = Some(api_key);
                if let Some(base_url) = config.resolve_base_url() {
                    provider_config.base_url = Some(base_url);
                }
                tracing::info!("Registered OpenAI provider (config-driven)");
                service.register_provider(Arc::new(OpenAiProvider::new(provider_config))).await;
            } else {
                return Err(LlmError::Configuration(format!(
                    "OpenAI API key not found. Set {} environment variable",
                    keys::OPENAI_API_KEY
                )));
            }
        }

        #[cfg(feature = "anthropic")]
        provider_id::ANTHROPIC => {
            if let Some(api_key) = config.resolve_api_key() {
                let mut provider_config = ProviderConfig::default();
                provider_config.api_key = Some(api_key);
                if let Some(base_url) = config.resolve_base_url() {
                    provider_config.base_url = Some(base_url);
                }
                tracing::info!("Registered Anthropic provider (config-driven)");
                service.register_provider(Arc::new(AnthropicProvider::new(provider_config))).await;
            } else {
                return Err(LlmError::Configuration(format!(
                    "Anthropic API key not found. Set {} environment variable",
                    keys::ANTHROPIC_API_KEY
                )));
            }
        }

        #[cfg(feature = "gemini")]
        provider_id::GEMINI => {
            if let Some(api_key) = config.resolve_api_key() {
                let mut provider_config = ProviderConfig::default();
                provider_config.api_key = Some(api_key);
                if let Some(base_url) = config.resolve_base_url() {
                    provider_config.base_url = Some(base_url);
                }
                tracing::info!("Registered Gemini provider (config-driven)");
                service.register_provider(Arc::new(GeminiProvider::new(provider_config))).await;
            } else {
                return Err(LlmError::Configuration(format!(
                    "Gemini API key not found. Set {} or {} environment variable",
                    keys::GEMINI_API_KEY,
                    keys::GOOGLE_API_KEY
                )));
            }
        }

        other => {
            return Err(LlmError::Configuration(format!(
                "Unknown provider '{}'. Supported: openai, anthropic, gemini",
                other
            )));
        }
    }

    Ok(service)
}

/// Create a service with all available providers (legacy behavior)
///
/// This registers all providers that have API keys configured, regardless
/// of the `LLM_PROVIDER` setting. Use `create_service()` for config-driven
/// single-provider mode.
pub async fn create_service_all_providers() -> LlmResult<DefaultLlmService> {
    let service = DefaultLlmService::new();

    // Register OpenAI if configured
    #[cfg(feature = "openai")]
    if let Ok(provider) = OpenAiProvider::from_env() {
        tracing::info!("Registered OpenAI provider");
        service.register_provider(Arc::new(provider)).await;
    }

    // Register Anthropic if configured
    #[cfg(feature = "anthropic")]
    if let Ok(provider) = AnthropicProvider::from_env() {
        tracing::info!("Registered Anthropic provider");
        service.register_provider(Arc::new(provider)).await;
    }

    // Register Gemini if configured
    #[cfg(feature = "gemini")]
    if let Ok(provider) = GeminiProvider::from_env() {
        tracing::info!("Registered Gemini provider");
        service.register_provider(Arc::new(provider)).await;
    }

    Ok(service)
}

/// Create a service builder for custom provider configuration
///
/// # Example
/// ```no_run
/// use llm_provider::{service_builder, ProviderConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let service = service_builder()
///     .with_openai(ProviderConfig {
///         api_key: Some("sk-...".to_string()),
///         ..Default::default()
///     })
///     .build()
///     .await;
/// # Ok(())
/// # }
/// ```
pub fn service_builder() -> LlmServiceBuilder {
    LlmServiceBuilder::new()
}

/// Builder for creating custom LLM services
pub struct LlmServiceBuilder {
    providers: Vec<Arc<dyn AsyncLlmProvider>>,
}

impl LlmServiceBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    /// Add an OpenAI provider with custom config
    #[cfg(feature = "openai")]
    pub fn with_openai(mut self, config: ProviderConfig) -> Self {
        self.providers.push(Arc::new(OpenAiProvider::new(config)));
        self
    }

    /// Add an Anthropic provider with custom config
    #[cfg(feature = "anthropic")]
    pub fn with_anthropic(mut self, config: ProviderConfig) -> Self {
        self.providers.push(Arc::new(AnthropicProvider::new(config)));
        self
    }

    /// Add an Anthropic provider authenticated via OAuth bearer token.
    ///
    /// The OAuth service is called on every request to obtain a fresh token.
    #[cfg(feature = "oauth")]
    pub fn with_anthropic_oauth(
        mut self,
        oauth: Arc<dyn llm_oauth::OAuthService>,
        base_url: Option<String>,
    ) -> Self {
        self.providers
            .push(Arc::new(AnthropicProvider::with_oauth(oauth, base_url)));
        self
    }

    /// Add a Gemini provider with custom config
    #[cfg(feature = "gemini")]
    pub fn with_gemini(mut self, config: ProviderConfig) -> Self {
        self.providers.push(Arc::new(GeminiProvider::new(config)));
        self
    }

    /// Add a custom provider
    pub fn with_provider(mut self, provider: Arc<dyn AsyncLlmProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Build the service with all registered providers
    pub async fn build(self) -> DefaultLlmService {
        let service = DefaultLlmService::new();
        for provider in self.providers {
            service.register_provider(provider).await;
        }
        service
    }
}

impl Default for LlmServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Secrets Integration (feature: secrets)
// =============================================================================

#[cfg(feature = "secrets")]
mod secrets_integration {
    use super::*;
    use swe_secrets::SecretService;

    /// Create a service using the secrets service for API key retrieval
    ///
    /// This is the recommended way to create a service when using swe-secrets
    /// for centralized secret management.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use llm_provider::create_service_with_secrets;
    /// use swe_secrets::from_env;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let secrets = from_env().await?;
    ///     let service = create_service_with_secrets(&secrets).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_service_with_secrets<S: SecretService>(
        secrets: &S,
    ) -> LlmResult<DefaultLlmService> {
        let config = AppLlmConfig::from_env();
        create_service_from_config_with_secrets(&config, secrets).await
    }

    /// Create a service from config using the secrets service
    ///
    /// Combines explicit configuration with secrets-based API key retrieval.
    pub async fn create_service_from_config_with_secrets<S: SecretService>(
        config: &AppLlmConfig,
        secrets: &S,
    ) -> LlmResult<DefaultLlmService> {
        let service = DefaultLlmService::new();
        let provider_name = &config.llm.provider;

        tracing::debug!(
            "Creating LLM service with provider: {} (using secrets)",
            provider_name
        );

        // Resolve API key and base URL using secrets service
        let api_key = config
            .resolve_api_key_with_secrets(secrets)
            .await
            .map_err(|e| LlmError::Configuration(format!("Secrets error: {}", e)))?;

        let base_url = config
            .resolve_base_url_with_secrets(secrets)
            .await
            .map_err(|e| LlmError::Configuration(format!("Secrets error: {}", e)))?;

        match provider_name.as_str() {
            #[cfg(feature = "openai")]
            provider_id::OPENAI => {
                if let Some(key) = api_key {
                    let mut provider_config = ProviderConfig::default();
                    provider_config.api_key = Some(key);
                    if let Some(url) = base_url {
                        provider_config.base_url = Some(url);
                    }
                    tracing::info!("Registered OpenAI provider (secrets-driven)");
                    service
                        .register_provider(Arc::new(OpenAiProvider::new(provider_config)))
                        .await;
                } else {
                    return Err(LlmError::Configuration(format!(
                        "OpenAI API key not found. Set {} in your secrets",
                        keys::OPENAI_API_KEY
                    )));
                }
            }

            #[cfg(feature = "anthropic")]
            provider_id::ANTHROPIC => {
                if let Some(key) = api_key {
                    let mut provider_config = ProviderConfig::default();
                    provider_config.api_key = Some(key);
                    if let Some(url) = base_url {
                        provider_config.base_url = Some(url);
                    }
                    tracing::info!("Registered Anthropic provider (secrets-driven)");
                    service
                        .register_provider(Arc::new(AnthropicProvider::new(provider_config)))
                        .await;
                } else {
                    return Err(LlmError::Configuration(format!(
                        "Anthropic API key not found. Set {} in your secrets",
                        keys::ANTHROPIC_API_KEY
                    )));
                }
            }

            #[cfg(feature = "gemini")]
            provider_id::GEMINI => {
                if let Some(key) = api_key {
                    let mut provider_config = ProviderConfig::default();
                    provider_config.api_key = Some(key);
                    if let Some(url) = base_url {
                        provider_config.base_url = Some(url);
                    }
                    tracing::info!("Registered Gemini provider (secrets-driven)");
                    service
                        .register_provider(Arc::new(GeminiProvider::new(provider_config)))
                        .await;
                } else {
                    return Err(LlmError::Configuration(format!(
                        "Gemini API key not found. Set {} or {} in your secrets",
                        keys::GEMINI_API_KEY,
                        keys::GOOGLE_API_KEY
                    )));
                }
            }

            other => {
                return Err(LlmError::Configuration(format!(
                    "Unknown provider '{}'. Supported: openai, anthropic, gemini",
                    other
                )));
            }
        }

        Ok(service)
    }
}

#[cfg(feature = "secrets")]
pub use secrets_integration::{create_service_from_config_with_secrets, create_service_with_secrets};

#[cfg(feature = "secrets")]
pub use swe_secrets::{SecretService, SecretsError};

#[cfg(any(test, feature = "testing"))]
pub use testing::{MockBehaviour, MockLlmService};

// =============================================================================
// OAuth Integration (feature: oauth)
// =============================================================================

/// Re-exports from `llm-oauth` for consumers that use OAuth-based authentication.
#[cfg(feature = "oauth")]
pub mod oauth {
    pub use llm_oauth::{
        from_claude_credentials, from_credentials_file, from_credentials,
        OAuthService,
        ClaudeCredentialsStore, DefaultOAuthService, InMemoryTokenStore,
        OAuthCredentials, OAuthConfig, OAuthError, OAuthResult,
    };
}
