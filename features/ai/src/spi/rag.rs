/// SPI traits for Retrieval-Augmented Generation (RAG).
///
/// Defines the provider interfaces for embedding generation and vector storage,
/// along with the data types that flow between them. Implementations live in
/// `core::rag`; this module only declares contracts.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::api::error::AiResult;

// ── Data types ──────────────────────────────────────────────────────

/// A chunk of text extracted from a documentation file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocChunk {
    /// Unique identifier for this chunk (typically `{agent_id}:{source_path}:{byte_offset}`).
    pub id: String,
    /// The text content of this chunk.
    pub content: String,
    /// Relative path of the source file this chunk was extracted from.
    pub source_path: String,
    /// Byte offset within the source file where this chunk begins.
    pub byte_offset: usize,
    /// The agent this chunk belongs to.
    pub agent_id: String,
}

/// A search result pairing a chunk with its similarity score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matched chunk.
    pub chunk: DocChunk,
    /// Cosine similarity score (higher is more relevant).
    pub score: f32,
}

// ── Embedding provider ──────────────────────────────────────────────

/// Generates vector embeddings from text.
///
/// Implementations may use local ONNX models (FastEmbed) or remote APIs.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed one or more texts, returning one vector per input text.
    async fn embed(&self, texts: &[String]) -> AiResult<Vec<Vec<f32>>>;

    /// The dimensionality of the embedding vectors produced by this provider.
    fn dimension(&self) -> usize;

    /// Human-readable model name (e.g. "BAAI/bge-small-en-v1.5").
    fn model_name(&self) -> &str;
}

// ── Vector store ────────────────────────────────────────────────────

/// Persists and retrieves document chunks with their embedding vectors.
///
/// Implementations range from in-memory (ephemeral) to SQLite (persistent).
/// All operations are scoped by `agent_id` so each agent maintains an
/// independent index.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Insert or update chunks with their corresponding embeddings.
    ///
    /// `chunks` and `embeddings` must have the same length; each pair
    /// shares the same positional index.
    async fn upsert(&self, chunks: &[DocChunk], embeddings: &[Vec<f32>]) -> AiResult<()>;

    /// Search for the `top_k` most similar chunks to `query_embedding`
    /// within the given agent's index.
    async fn search(
        &self,
        query_embedding: &[f32],
        agent_id: &str,
        top_k: usize,
    ) -> AiResult<Vec<SearchResult>>;

    /// Delete all chunks belonging to the given agent.
    async fn delete_agent(&self, agent_id: &str) -> AiResult<()>;

    /// Check whether any chunks exist for the given agent.
    async fn has_index(&self, agent_id: &str) -> AiResult<bool>;

    /// Load the persisted fingerprint for an agent's index.
    ///
    /// Returns `Ok(None)` if no fingerprint has been persisted (e.g.,
    /// ephemeral stores or first run).  Persistent backends should return
    /// `Ok(Some(hex))` when a valid index + fingerprint exist on disk.
    async fn load_fingerprint(&self, agent_id: &str) -> AiResult<Option<String>>;

    /// Persist the fingerprint after a successful index build.
    ///
    /// Ephemeral stores may no-op.  Persistent backends must store the
    /// fingerprint alongside their vector data so it survives process restarts.
    async fn save_fingerprint(&self, agent_id: &str, fingerprint: &str) -> AiResult<()>;
}
