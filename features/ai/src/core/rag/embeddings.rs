/// EmbeddingProvider implementations for RAG.
///
/// - `FastEmbedProvider` — local ONNX inference via the `fastembed` crate
///   (feature-gated behind `rag-local`)
/// - `ApiEmbeddingProvider` — delegates to an external embedding API via
///   the existing `AiClient` / LLM provider infrastructure

use async_trait::async_trait;

use crate::api::error::{AiError, AiResult};
use crate::spi::rag::EmbeddingProvider;

// ── FastEmbedProvider ───────────────────────────────────────────────

/// Local ONNX-based embedding using the `fastembed` crate.
///
/// Uses `BAAI/bge-small-en-v1.5` (384 dimensions) by default.
/// The model is downloaded and cached on first use.
///
/// Only available when the `rag-local` feature is enabled.
#[cfg(feature = "rag-local")]
pub struct FastEmbedProvider {
    model: fastembed::TextEmbedding,
    model_name: String,
    dimension: usize,
}

#[cfg(feature = "rag-local")]
impl FastEmbedProvider {
    /// Create a new provider with the default model (BGE-small-en-v1.5, 384-dim).
    pub fn new() -> AiResult<Self> {
        let model = fastembed::TextEmbedding::try_new(
            fastembed::InitOptions::new(fastembed::EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(false),
        )
        .map_err(|e| AiError::IndexError(format!("failed to initialize FastEmbed model: {e}")))?;

        Ok(Self {
            model,
            model_name: "BAAI/bge-small-en-v1.5".to_string(),
            dimension: 384,
        })
    }
}

#[cfg(feature = "rag-local")]
#[async_trait]
impl EmbeddingProvider for FastEmbedProvider {
    async fn embed(&self, texts: &[String]) -> AiResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // fastembed::TextEmbedding::embed() is sync and CPU-bound, so we
        // run it on the blocking thread pool to avoid starving the async runtime.
        let texts_owned: Vec<String> = texts.to_vec();

        // fastembed is not Send, so we need to handle this carefully.
        // The model itself needs to be accessed in the current thread context.
        self.model
            .embed(texts_owned, None)
            .map_err(|e| AiError::IndexError(format!("FastEmbed embedding failed: {e}")))
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

// ── ApiEmbeddingProvider ────────────────────────────────────────────

/// Embedding provider that delegates to an external API.
///
/// This is a placeholder for phase 2 — it will use the existing `AiClient`
/// or a dedicated embedding endpoint. For now, agents should prefer the
/// local `FastEmbedProvider`.
pub struct ApiEmbeddingProvider {
    model_name: String,
    dimension: usize,
    api_key: String,
    provider: String,
}

impl ApiEmbeddingProvider {
    /// Create a new API-based embedding provider.
    ///
    /// # Arguments
    /// - `provider` — "openai", "anthropic", etc.
    /// - `api_key` — the API key for the provider
    /// - `model_name` — the embedding model to use (e.g. "text-embedding-3-small")
    /// - `dimension` — the expected output dimension
    pub fn new(
        provider: impl Into<String>,
        api_key: impl Into<String>,
        model_name: impl Into<String>,
        dimension: usize,
    ) -> Self {
        Self {
            provider: provider.into(),
            api_key: api_key.into(),
            model_name: model_name.into(),
            dimension,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for ApiEmbeddingProvider {
    async fn embed(&self, texts: &[String]) -> AiResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Phase 2: implement actual API calls to OpenAI/Anthropic embedding endpoints.
        // For now, return an error directing users to use the local provider.
        Err(AiError::IndexError(format!(
            "ApiEmbeddingProvider for '{}' is not yet implemented. \
             Enable the `rag-local` feature to use FastEmbedProvider instead.",
            self.provider,
        )))
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

// Suppress unused field warnings for phase 2 fields.
impl ApiEmbeddingProvider {
    #[allow(dead_code)]
    fn api_key(&self) -> &str {
        &self.api_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn api_provider_returns_error() {
        let provider = ApiEmbeddingProvider::new("openai", "sk-test", "text-embedding-3-small", 1536);
        let result = provider.embed(&["hello".to_string()]).await;
        assert!(result.is_err());
        assert_eq!(provider.dimension(), 1536);
        assert_eq!(provider.model_name(), "text-embedding-3-small");
    }

    #[tokio::test]
    async fn api_provider_empty_input() {
        let provider = ApiEmbeddingProvider::new("openai", "sk-test", "model", 384);
        let result = provider.embed(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
