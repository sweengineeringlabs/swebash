/// RAG (Retrieval-Augmented Generation) subsystem.
///
/// Re-exports from the `llmrag` crate. Preserves existing import paths
/// (`crate::core::rag::*`) for backwards compatibility.
///
/// # Module layout
///
/// - `normalize` — converts Markdown tables to prose before embedding
/// - `tool` — `SwebashRagTool` with score filtering and configurable display
/// - `service` — `PreprocessingRagIndexService` with normalize hook
/// - `chunker` — splits documents into overlapping text chunks
/// - `embeddings` — `EmbeddingProvider` implementations (local ONNX, API)
/// - `stores` — `VectorStore` implementations (in-memory, file, SQLite)
/// - `index` — `RagIndexManager` orchestrating build/query lifecycle

pub mod normalize;
pub mod tool;
pub mod service;

pub mod chunker {
    pub use llmrag::{chunk_text, ChunkerConfig};
}

pub mod embeddings {
    #[cfg(feature = "rag-local")]
    pub use llmrag::FastEmbedProvider;
}

pub mod stores {
    pub use llmrag::{build_vector_store, FileVectorStore, InMemoryVectorStore, VectorStoreConfig};

    #[cfg(feature = "rag-sqlite")]
    pub use llmrag::SqliteVectorStore;

    #[cfg(feature = "rag-swevecdb")]
    pub use llmrag::SweVecdbVectorStore;
}

pub mod index {
    pub use llmrag::{RagIndexManager, RagIndexService};
}
