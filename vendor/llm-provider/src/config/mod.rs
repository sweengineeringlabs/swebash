//! Configuration types for LLM providers

pub mod app;
pub mod keys;
pub mod provider;

pub use app::{AppLlmConfig, LlmConfig, ProviderSpec, ProvidersConfig};
pub use provider::ProviderConfig;

// Provider IDs are accessed as `config::app::provider_id::OPENAI`
