/// RAG (Retrieval-Augmented Generation) subsystem.
///
/// Re-exports from the `llmrag` crate. Preserves existing import paths
/// (`crate::core::rag::*`) for backwards compatibility.
///
/// # Module layout
///
/// - `chunker` — splits documents into overlapping text chunks
/// - `embeddings` — `EmbeddingProvider` implementations (local ONNX, API)
/// - `stores` — `VectorStore` implementations (in-memory, file, SQLite)
/// - `index` — `RagIndexManager` orchestrating build/query lifecycle
/// - `tool` — `RagTool` implementing the rustratify `Tool` trait
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

pub mod tool {
    pub use llmrag::RagTool;
}
