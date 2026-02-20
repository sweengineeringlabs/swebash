//! Default implementation of the LlmService trait

use crate::api::{CompletionRequest, CompletionResponse, CompletionStream, LlmService};
use crate::api::{LlmError, LlmResult, ModelInfo};
use crate::core::context::{ContextConfig, ContextValidator, ValidationResult};
use crate::spi::AsyncLlmProvider;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Default implementation of LlmService that routes requests to registered providers
#[derive(Debug)]
pub struct DefaultLlmService {
    /// Map of provider name to provider instance
    providers: RwLock<HashMap<String, Arc<dyn AsyncLlmProvider>>>,
    /// Map of model ID to provider name
    model_to_provider: RwLock<HashMap<String, String>>,
    /// Context validator for pre-flight checks
    context_validator: Option<ContextValidator>,
}

impl DefaultLlmService {
    /// Create a new DefaultLlmService without context validation
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            model_to_provider: RwLock::new(HashMap::new()),
            context_validator: None,
        }
    }

    /// Create a new DefaultLlmService with context validation enabled
    pub fn with_context_validation() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            model_to_provider: RwLock::new(HashMap::new()),
            context_validator: Some(ContextValidator::default()),
        }
    }

    /// Create a new DefaultLlmService with custom context configuration
    pub fn with_context_config(config: ContextConfig) -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            model_to_provider: RwLock::new(HashMap::new()),
            context_validator: Some(ContextValidator::new(config)),
        }
    }

    /// Enable context validation on an existing service
    pub fn enable_context_validation(&mut self, config: Option<ContextConfig>) {
        self.context_validator = Some(match config {
            Some(c) => ContextValidator::new(c),
            None => ContextValidator::default(),
        });
    }

    /// Disable context validation
    pub fn disable_context_validation(&mut self) {
        self.context_validator = None;
    }

    /// Register a provider and map its models
    ///
    /// # Arguments
    ///
    /// * `provider` - The provider to register
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut service = DefaultLlmService::new();
    /// service.register_provider(Arc::new(OpenAiProvider::from_env()?)).await;
    /// ```
    pub async fn register_provider(&self, provider: Arc<dyn AsyncLlmProvider>) {
        let provider_name = provider.name().to_string();
        debug!("Registering provider: {}", provider_name);

        // Map all provider models to this provider
        let mut model_map = self.model_to_provider.write().await;
        for model in provider.models() {
            debug!("  Mapping model {} -> {}", model, provider_name);
            model_map.insert(model.to_string(), provider_name.clone());
        }
        drop(model_map);

        // Store the provider
        let mut providers = self.providers.write().await;
        providers.insert(provider_name, provider);
    }

    /// Find the provider for a given model
    async fn find_provider(&self, model: &str) -> LlmResult<Arc<dyn AsyncLlmProvider>> {
        // First, try exact match in our model map
        let provider_name = {
            let model_map = self.model_to_provider.read().await;
            model_map.get(model).cloned()
        };

        if let Some(provider_name) = provider_name {
            let providers = self.providers.read().await;
            if let Some(provider) = providers.get(&provider_name) {
                return Ok(Arc::clone(provider));
            }
        }

        // Try prefix matching on all providers
        let providers = self.providers.read().await;
        for provider in providers.values() {
            if provider.supports(model) {
                return Ok(Arc::clone(provider));
            }
        }

        Err(LlmError::ModelNotFound(format!(
            "No provider found for model: {}",
            model
        )))
    }
}

impl Default for DefaultLlmService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LlmService for DefaultLlmService {
    async fn complete(&self, request: CompletionRequest) -> LlmResult<CompletionResponse> {
        debug!("Completing request for model: {}", request.model);
        let provider = self.find_provider(&request.model).await?;

        // Apply context validation if enabled
        let request = if let Some(ref validator) = self.context_validator {
            // Get model info for context window size
            if let Some(model_info) = provider.model_info(&request.model) {
                let validation_result = validator.validate_or_error(&request, &model_info)?;

                match validation_result {
                    ValidationResult::Ok { utilization, .. } => {
                        debug!(
                            "Context validation passed: {:.1}% utilization",
                            utilization * 100.0
                        );
                        request
                    }
                    ValidationResult::Warning { utilization, .. } => {
                        info!(
                            "Context window at {:.1}% - approaching limit",
                            utilization * 100.0
                        );
                        request
                    }
                    ValidationResult::Truncated { messages, removed_message_count, .. } => {
                        info!(
                            "Auto-truncated {} messages to fit context window",
                            removed_message_count
                        );
                        // Create new request with truncated messages
                        CompletionRequest {
                            messages,
                            ..request
                        }
                    }
                    ValidationResult::Exceeded { .. } => {
                        // Should never reach here - validate_or_error returns Err for Exceeded
                        unreachable!("Exceeded case handled by validate_or_error")
                    }
                }
            } else {
                warn!(
                    "Model info not available for {}, skipping context validation",
                    request.model
                );
                request
            }
        } else {
            request
        };

        provider.complete(&request).await
    }

    async fn complete_stream(&self, request: CompletionRequest) -> LlmResult<CompletionStream> {
        debug!("Streaming completion for model: {}", request.model);
        let provider = self.find_provider(&request.model).await?;

        // Apply context validation if enabled
        let request = if let Some(ref validator) = self.context_validator {
            // Get model info for context window size
            if let Some(model_info) = provider.model_info(&request.model) {
                let validation_result = validator.validate_or_error(&request, &model_info)?;

                match validation_result {
                    ValidationResult::Ok { utilization, .. } => {
                        debug!(
                            "Context validation passed: {:.1}% utilization",
                            utilization * 100.0
                        );
                        request
                    }
                    ValidationResult::Warning { utilization, .. } => {
                        info!(
                            "Context window at {:.1}% - approaching limit",
                            utilization * 100.0
                        );
                        request
                    }
                    ValidationResult::Truncated { messages, removed_message_count, .. } => {
                        info!(
                            "Auto-truncated {} messages to fit context window",
                            removed_message_count
                        );
                        CompletionRequest {
                            messages,
                            ..request
                        }
                    }
                    ValidationResult::Exceeded { .. } => {
                        unreachable!("Exceeded case handled by validate_or_error")
                    }
                }
            } else {
                warn!(
                    "Model info not available for {}, skipping context validation",
                    request.model
                );
                request
            }
        } else {
            request
        };

        Ok(provider.complete_stream(&request))
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        debug!("Listing all models");
        let providers = self.providers.read().await;
        let mut models = Vec::new();

        for provider in providers.values() {
            for model_id in provider.models() {
                if let Some(info) = provider.model_info(model_id) {
                    models.push(info);
                } else {
                    // Create basic info if provider doesn't provide it
                    warn!(
                        "Provider {} doesn't provide info for model {}",
                        provider.name(),
                        model_id
                    );
                }
            }
        }

        Ok(models)
    }

    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo> {
        debug!("Getting info for model: {}", model);
        let provider = self.find_provider(model).await?;
        provider
            .model_info(model)
            .ok_or_else(|| LlmError::ModelNotFound(format!("Model info not available: {}", model)))
    }

    async fn is_model_available(&self, model: &str) -> bool {
        self.find_provider(model).await.is_ok()
    }

    async fn providers(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
}
