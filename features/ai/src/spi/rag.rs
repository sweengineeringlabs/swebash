/// SPI traits for Retrieval-Augmented Generation (RAG).
///
/// Re-exports from the `llmrag` crate. Preserves existing import paths
/// (`crate::spi::rag::*`) for backwards compatibility.

pub use llmrag::{DocChunk, EmbeddingProvider, SearchResult, VectorStore};
