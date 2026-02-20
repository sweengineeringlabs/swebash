//! LLM SPI - Provider traits for LLM backends
//!
//! This crate defines the contracts that LLM provider implementations must satisfy.
//!
//! # Overview
//!
//! The SPI (Service Provider Interface) layer defines two main traits:
//! - [`LlmProvider`]: For synchronous/blocking LLM providers (e.g., local llama.cpp bindings)
//! - [`AsyncLlmProvider`]: For asynchronous LLM providers (e.g., OpenAI, Anthropic HTTP APIs)
//!
//! # Examples
//!
//! Implementing a synchronous provider:
//!
//! ```rust,ignore
//! use llm_spi::*;
//!
//! #[derive(Debug)]
//! struct MyProvider;
//!
//! impl LlmProvider for MyProvider {
//!     fn name(&self) -> &str { "my-provider" }
//!     fn models(&self) -> &[&str] { &["model-1", "model-2"] }
//!     fn model_info(&self, model: &str) -> Option<ModelInfo> { None }
//!     fn is_configured(&self) -> bool { true }
//!     fn complete(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse> {
//!         // Implementation here
//!     }
//!     fn complete_stream(&self, request: &CompletionRequest)
//!         -> LlmResult<Box<dyn Iterator<Item = LlmResult<StreamChunk>> + Send>> {
//!         // Implementation here
//!     }
//!     fn as_any(&self) -> &dyn Any { self }
//! }
//! ```

mod openai;
mod anthropic;
mod gemini;

pub use openai::OpenAiProvider;
pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;

use std::any::Any;
use futures::stream::BoxStream;

use crate::api::{LlmResult, CompletionRequest, CompletionResponse, ModelInfo, StreamChunk};

/// Synchronous LLM provider trait
///
/// Use this for providers that have blocking APIs (e.g., local llama.cpp bindings).
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow usage across thread boundaries.
///
/// # Model Support
///
/// Providers should implement [`models()`](Self::models) to return a list of supported
/// model identifiers. The [`supports()`](Self::supports) method provides a default
/// implementation that checks if a model ID matches or starts with any supported model.
pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    /// Get the provider name (e.g., "openai", "anthropic")
    ///
    /// This should be a stable identifier that doesn't change across versions.
    fn name(&self) -> &str;

    /// Get list of supported model IDs
    ///
    /// Returns a slice of model identifiers that this provider can handle.
    /// Model IDs can be exact matches or prefixes (e.g., "gpt-4" matches "gpt-4-turbo").
    fn models(&self) -> &[&str];

    /// Check if provider supports the given model
    ///
    /// Default implementation checks if the model ID exactly matches or starts with
    /// any of the supported models returned by [`models()`](Self::models).
    ///
    /// # Arguments
    ///
    /// * `model` - The model identifier to check
    ///
    /// # Returns
    ///
    /// `true` if the provider supports this model, `false` otherwise
    fn supports(&self, model: &str) -> bool {
        self.models().iter().any(|m| *m == model || model.starts_with(m))
    }

    /// Get model information
    ///
    /// Returns detailed information about a specific model if available.
    ///
    /// # Arguments
    ///
    /// * `model` - The model identifier to query
    ///
    /// # Returns
    ///
    /// `Some(ModelInfo)` if information is available, `None` otherwise
    fn model_info(&self, model: &str) -> Option<ModelInfo>;

    /// Check if provider is configured and ready
    ///
    /// This should verify that all necessary configuration (API keys, endpoints, etc.)
    /// is present and valid. It should not make network calls or perform expensive checks.
    ///
    /// # Returns
    ///
    /// `true` if the provider is ready to handle requests, `false` otherwise
    fn is_configured(&self) -> bool;

    /// Complete a request (blocking)
    ///
    /// Sends a completion request to the LLM and waits for the full response.
    /// This method blocks the current thread until the response is received.
    ///
    /// # Arguments
    ///
    /// * `request` - The completion request containing the prompt and parameters
    ///
    /// # Returns
    ///
    /// `Ok(CompletionResponse)` on success, or an error if the request fails
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The provider is not configured
    /// - The model is not supported
    /// - Network or API errors occur
    /// - The request is invalid or rejected
    fn complete(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse>;

    /// Complete with streaming (returns sync iterator)
    ///
    /// Sends a completion request and returns an iterator that yields response chunks
    /// as they arrive. This is useful for displaying progressive output to users.
    ///
    /// # Arguments
    ///
    /// * `request` - The completion request containing the prompt and parameters
    ///
    /// # Returns
    ///
    /// An iterator that yields `StreamChunk` results as they arrive
    ///
    /// # Errors
    ///
    /// The iterator items may contain errors if streaming fails partway through.
    fn complete_stream(&self, request: &CompletionRequest)
        -> LlmResult<Box<dyn Iterator<Item = LlmResult<StreamChunk>> + Send>>;

    /// Downcast support for type-specific operations
    ///
    /// Allows downcasting to concrete provider types for accessing
    /// provider-specific functionality.
    ///
    /// # Returns
    ///
    /// A reference to `self` as `&dyn Any`
    fn as_any(&self) -> &dyn Any;
}

/// Asynchronous LLM provider trait
///
/// Use this for providers with async/HTTP-based APIs (e.g., OpenAI, Anthropic).
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow usage across async runtime boundaries.
///
/// # Model Support
///
/// Providers should implement [`models()`](Self::models) to return a list of supported
/// model identifiers. The [`supports()`](Self::supports) method provides a default
/// implementation that checks if a model ID matches or starts with any supported model.
///
/// # Examples
///
/// ```rust,ignore
/// use llm_spi::*;
///
/// #[derive(Debug)]
/// struct MyAsyncProvider;
///
/// #[async_trait::async_trait]
/// impl AsyncLlmProvider for MyAsyncProvider {
///     fn name(&self) -> &str { "my-async-provider" }
///     fn models(&self) -> &[&str] { &["model-1"] }
///     fn model_info(&self, model: &str) -> Option<ModelInfo> { None }
///     fn is_configured(&self) -> bool { true }
///
///     async fn complete(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse> {
///         // Async implementation
///     }
///
///     fn complete_stream(&self, request: &CompletionRequest)
///         -> BoxStream<'static, LlmResult<StreamChunk>> {
///         // Return async stream
///     }
///
///     fn as_any(&self) -> &dyn Any { self }
/// }
/// ```
#[async_trait::async_trait]
pub trait AsyncLlmProvider: Send + Sync + std::fmt::Debug {
    /// Get the provider name
    ///
    /// This should be a stable identifier that doesn't change across versions.
    fn name(&self) -> &str;

    /// Get list of supported model IDs
    ///
    /// Returns a slice of model identifiers that this provider can handle.
    /// Model IDs can be exact matches or prefixes (e.g., "gpt-4" matches "gpt-4-turbo").
    fn models(&self) -> &[&str];

    /// Check if provider supports the given model
    ///
    /// Default implementation checks if the model ID exactly matches or starts with
    /// any of the supported models returned by [`models()`](Self::models).
    ///
    /// # Arguments
    ///
    /// * `model` - The model identifier to check
    ///
    /// # Returns
    ///
    /// `true` if the provider supports this model, `false` otherwise
    fn supports(&self, model: &str) -> bool {
        self.models().iter().any(|m| *m == model || model.starts_with(m))
    }

    /// Get model information
    ///
    /// Returns detailed information about a specific model if available.
    ///
    /// # Arguments
    ///
    /// * `model` - The model identifier to query
    ///
    /// # Returns
    ///
    /// `Some(ModelInfo)` if information is available, `None` otherwise
    fn model_info(&self, model: &str) -> Option<ModelInfo>;

    /// Check if provider is configured and ready
    ///
    /// This should verify that all necessary configuration (API keys, endpoints, etc.)
    /// is present and valid. It should not make network calls or perform expensive checks.
    ///
    /// # Returns
    ///
    /// `true` if the provider is ready to handle requests, `false` otherwise
    fn is_configured(&self) -> bool;

    /// Complete a request asynchronously
    ///
    /// Sends a completion request to the LLM and awaits the full response.
    ///
    /// # Arguments
    ///
    /// * `request` - The completion request containing the prompt and parameters
    ///
    /// # Returns
    ///
    /// `Ok(CompletionResponse)` on success, or an error if the request fails
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The provider is not configured
    /// - The model is not supported
    /// - Network or API errors occur
    /// - The request is invalid or rejected
    async fn complete(&self, request: &CompletionRequest) -> LlmResult<CompletionResponse>;

    /// Complete with streaming (returns async stream)
    ///
    /// Sends a completion request and returns an async stream that yields response chunks
    /// as they arrive. This is useful for displaying progressive output to users.
    ///
    /// # Arguments
    ///
    /// * `request` - The completion request containing the prompt and parameters
    ///
    /// # Returns
    ///
    /// A boxed async stream that yields `StreamChunk` results as they arrive
    ///
    /// # Note
    ///
    /// The stream has a `'static` lifetime, meaning it owns all its data.
    /// Stream items may contain errors if streaming fails partway through.
    fn complete_stream(&self, request: &CompletionRequest)
        -> BoxStream<'static, LlmResult<StreamChunk>>;

    /// Downcast support
    ///
    /// Allows downcasting to concrete provider types for accessing
    /// provider-specific functionality.
    ///
    /// # Returns
    ///
    /// A reference to `self` as `&dyn Any`
    fn as_any(&self) -> &dyn Any;
}
