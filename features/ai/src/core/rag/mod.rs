/// RAG (Retrieval-Augmented Generation) subsystem.
///
/// Provides document chunking, embedding, vector storage, index management,
/// and a tool that agents can invoke to search their documentation on demand.
///
/// # Module layout
///
/// - `chunker` — splits documents into overlapping text chunks
/// - `embeddings` — `EmbeddingProvider` implementations (local ONNX, API)
/// - `stores` — `VectorStore` implementations (in-memory, file, SQLite)
/// - `index` — `RagIndexManager` orchestrating build/query lifecycle
/// - `tool` — `RagTool` implementing the rustratify `Tool` trait

pub mod chunker;
pub mod embeddings;
pub mod index;
pub mod stores;
pub mod tool;
