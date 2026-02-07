/// VectorStore implementations for RAG.
///
/// - `InMemoryVectorStore` — ephemeral, brute-force cosine similarity
/// - `FileVectorStore` — JSON-serialized per-agent index files
/// - `SqliteVectorStore` — persistent SQLite storage (feature-gated)
///
/// Brute-force cosine is sufficient for agent doc corpora (100–500 chunks).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::api::error::{AiError, AiResult};
use crate::spi::rag::{DocChunk, SearchResult, VectorStore};

// ── Cosine similarity ───────────────────────────────────────────────

/// Compute cosine similarity between two vectors.
///
/// Returns 0.0 if either vector has zero magnitude.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "vector dimensions must match");
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

// ── InMemoryVectorStore ─────────────────────────────────────────────

/// Ephemeral vector store backed by a `HashMap`.
///
/// Chunks and embeddings are stored in memory grouped by `agent_id`.
/// Search is brute-force cosine similarity over all chunks for the agent.
pub struct InMemoryVectorStore {
    data: Arc<RwLock<HashMap<String, Vec<StoredEntry>>>>,
}

/// A chunk paired with its embedding vector.
#[derive(Clone)]
struct StoredEntry {
    chunk: DocChunk,
    embedding: Vec<f32>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn upsert(&self, chunks: &[DocChunk], embeddings: &[Vec<f32>]) -> AiResult<()> {
        if chunks.len() != embeddings.len() {
            return Err(AiError::IndexError(
                "chunks and embeddings length mismatch".to_string(),
            ));
        }

        let mut data = self.data.write().await;
        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            let entries = data.entry(chunk.agent_id.clone()).or_default();

            // Upsert: replace existing entry with same ID, or append.
            if let Some(existing) = entries.iter_mut().find(|e| e.chunk.id == chunk.id) {
                existing.chunk = chunk.clone();
                existing.embedding = embedding.clone();
            } else {
                entries.push(StoredEntry {
                    chunk: chunk.clone(),
                    embedding: embedding.clone(),
                });
            }
        }
        Ok(())
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        agent_id: &str,
        top_k: usize,
    ) -> AiResult<Vec<SearchResult>> {
        let data = self.data.read().await;
        let entries = match data.get(agent_id) {
            Some(e) => e,
            None => return Ok(Vec::new()),
        };

        let mut scored: Vec<(f32, &StoredEntry)> = entries
            .iter()
            .map(|entry| (cosine_similarity(query_embedding, &entry.embedding), entry))
            .collect();

        // Sort descending by score.
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        Ok(scored
            .into_iter()
            .map(|(score, entry)| SearchResult {
                chunk: entry.chunk.clone(),
                score,
            })
            .collect())
    }

    async fn delete_agent(&self, agent_id: &str) -> AiResult<()> {
        let mut data = self.data.write().await;
        data.remove(agent_id);
        Ok(())
    }

    async fn has_index(&self, agent_id: &str) -> AiResult<bool> {
        let data = self.data.read().await;
        Ok(data.get(agent_id).map_or(false, |v| !v.is_empty()))
    }
}

// ── FileVectorStore ─────────────────────────────────────────────────

/// Persistent vector store that serializes each agent's index to a JSON file.
///
/// Index files are stored at `{store_dir}/{agent_id}.index.json`.
/// Data is loaded into memory for search and flushed to disk on upsert.
pub struct FileVectorStore {
    store_dir: PathBuf,
    cache: Arc<RwLock<HashMap<String, Vec<FileStoredEntry>>>>,
}

/// Serializable entry for file-based storage.
#[derive(Clone, Serialize, Deserialize)]
struct FileStoredEntry {
    chunk: DocChunk,
    embedding: Vec<f32>,
}

impl FileVectorStore {
    /// Create a new FileVectorStore writing index files under `store_dir`.
    ///
    /// The directory is created on first write if it does not exist.
    pub fn new(store_dir: impl Into<PathBuf>) -> Self {
        Self {
            store_dir: store_dir.into(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Path to the index file for a given agent.
    fn index_path(&self, agent_id: &str) -> PathBuf {
        self.store_dir.join(format!("{}.index.json", agent_id))
    }

    /// Load an agent's index from disk into the in-memory cache.
    async fn ensure_loaded(&self, agent_id: &str) -> AiResult<()> {
        {
            let cache = self.cache.read().await;
            if cache.contains_key(agent_id) {
                return Ok(());
            }
        }

        let path = self.index_path(agent_id);
        let entries = if path.is_file() {
            let data = std::fs::read_to_string(&path).map_err(|e| {
                AiError::IndexError(format!("failed to read index file {}: {e}", path.display()))
            })?;
            serde_json::from_str::<Vec<FileStoredEntry>>(&data).map_err(|e| {
                AiError::IndexError(format!(
                    "failed to parse index file {}: {e}",
                    path.display()
                ))
            })?
        } else {
            Vec::new()
        };

        let mut cache = self.cache.write().await;
        cache.entry(agent_id.to_string()).or_insert(entries);
        Ok(())
    }

    /// Flush an agent's in-memory data to disk.
    async fn flush(&self, agent_id: &str) -> AiResult<()> {
        let cache = self.cache.read().await;
        let entries = match cache.get(agent_id) {
            Some(e) => e,
            None => return Ok(()),
        };

        std::fs::create_dir_all(&self.store_dir).map_err(|e| {
            AiError::IndexError(format!(
                "failed to create index dir {}: {e}",
                self.store_dir.display()
            ))
        })?;

        let path = self.index_path(agent_id);
        let json = serde_json::to_string(entries).map_err(|e| {
            AiError::IndexError(format!("failed to serialize index: {e}"))
        })?;
        std::fs::write(&path, json).map_err(|e| {
            AiError::IndexError(format!("failed to write index file {}: {e}", path.display()))
        })?;

        Ok(())
    }
}

#[async_trait]
impl VectorStore for FileVectorStore {
    async fn upsert(&self, chunks: &[DocChunk], embeddings: &[Vec<f32>]) -> AiResult<()> {
        if chunks.len() != embeddings.len() {
            return Err(AiError::IndexError(
                "chunks and embeddings length mismatch".to_string(),
            ));
        }

        for chunk in chunks {
            self.ensure_loaded(&chunk.agent_id).await?;
        }

        {
            let mut cache = self.cache.write().await;
            for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
                let entries = cache.entry(chunk.agent_id.clone()).or_default();
                if let Some(existing) = entries.iter_mut().find(|e| e.chunk.id == chunk.id) {
                    existing.chunk = chunk.clone();
                    existing.embedding = embedding.clone();
                } else {
                    entries.push(FileStoredEntry {
                        chunk: chunk.clone(),
                        embedding: embedding.clone(),
                    });
                }
            }
        }

        // Flush all affected agents.
        let agent_ids: std::collections::HashSet<&str> =
            chunks.iter().map(|c| c.agent_id.as_str()).collect();
        for agent_id in agent_ids {
            self.flush(agent_id).await?;
        }

        Ok(())
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        agent_id: &str,
        top_k: usize,
    ) -> AiResult<Vec<SearchResult>> {
        self.ensure_loaded(agent_id).await?;

        let cache = self.cache.read().await;
        let entries = match cache.get(agent_id) {
            Some(e) => e,
            None => return Ok(Vec::new()),
        };

        let mut scored: Vec<(f32, &FileStoredEntry)> = entries
            .iter()
            .map(|entry| (cosine_similarity(query_embedding, &entry.embedding), entry))
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        Ok(scored
            .into_iter()
            .map(|(score, entry)| SearchResult {
                chunk: entry.chunk.clone(),
                score,
            })
            .collect())
    }

    async fn delete_agent(&self, agent_id: &str) -> AiResult<()> {
        let mut cache = self.cache.write().await;
        cache.remove(agent_id);

        let path = self.index_path(agent_id);
        if path.is_file() {
            let _ = std::fs::remove_file(&path);
        }
        Ok(())
    }

    async fn has_index(&self, agent_id: &str) -> AiResult<bool> {
        self.ensure_loaded(agent_id).await?;
        let cache = self.cache.read().await;
        Ok(cache.get(agent_id).map_or(false, |v| !v.is_empty()))
    }
}

// ── SqliteVectorStore ───────────────────────────────────────────────

/// Persistent vector store backed by SQLite.
///
/// Chunks are stored in a `chunks` table with their embeddings serialized
/// as JSON arrays. Search is brute-force cosine in Rust (loaded from DB).
/// Gated behind the `rag-sqlite` feature flag.
#[cfg(feature = "rag-sqlite")]
pub struct SqliteVectorStore {
    db: Arc<tokio::sync::Mutex<rusqlite::Connection>>,
}

#[cfg(feature = "rag-sqlite")]
impl SqliteVectorStore {
    /// Open or create a SQLite database at `path`.
    pub fn new(path: impl AsRef<Path>) -> AiResult<Self> {
        let conn = rusqlite::Connection::open(path.as_ref()).map_err(|e| {
            AiError::IndexError(format!("failed to open SQLite DB: {e}"))
        })?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                content TEXT NOT NULL,
                source_path TEXT NOT NULL,
                byte_offset INTEGER NOT NULL,
                embedding TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_agent ON chunks(agent_id);",
        )
        .map_err(|e| AiError::IndexError(format!("failed to init SQLite schema: {e}")))?;

        Ok(Self {
            db: Arc::new(tokio::sync::Mutex::new(conn)),
        })
    }
}

#[cfg(feature = "rag-sqlite")]
#[async_trait]
impl VectorStore for SqliteVectorStore {
    async fn upsert(&self, chunks: &[DocChunk], embeddings: &[Vec<f32>]) -> AiResult<()> {
        if chunks.len() != embeddings.len() {
            return Err(AiError::IndexError(
                "chunks and embeddings length mismatch".to_string(),
            ));
        }

        let db = self.db.lock().await;
        let mut stmt = db
            .prepare(
                "INSERT OR REPLACE INTO chunks (id, agent_id, content, source_path, byte_offset, embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(|e| AiError::IndexError(format!("SQLite prepare error: {e}")))?;

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            let emb_json = serde_json::to_string(embedding)
                .map_err(|e| AiError::IndexError(format!("embedding serialization: {e}")))?;
            stmt.execute(rusqlite::params![
                chunk.id,
                chunk.agent_id,
                chunk.content,
                chunk.source_path,
                chunk.byte_offset,
                emb_json,
            ])
            .map_err(|e| AiError::IndexError(format!("SQLite insert error: {e}")))?;
        }

        Ok(())
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        agent_id: &str,
        top_k: usize,
    ) -> AiResult<Vec<SearchResult>> {
        let db = self.db.lock().await;
        let mut stmt = db
            .prepare(
                "SELECT id, agent_id, content, source_path, byte_offset, embedding
                 FROM chunks WHERE agent_id = ?1",
            )
            .map_err(|e| AiError::IndexError(format!("SQLite query error: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params![agent_id], |row| {
                let emb_str: String = row.get(5)?;
                Ok((
                    DocChunk {
                        id: row.get(0)?,
                        agent_id: row.get(1)?,
                        content: row.get(2)?,
                        source_path: row.get(3)?,
                        byte_offset: row.get::<_, i64>(4)? as usize,
                    },
                    emb_str,
                ))
            })
            .map_err(|e| AiError::IndexError(format!("SQLite query_map error: {e}")))?;

        let mut scored: Vec<(f32, DocChunk)> = Vec::new();
        for row in rows {
            let (chunk, emb_str) = row.map_err(|e| {
                AiError::IndexError(format!("SQLite row error: {e}"))
            })?;
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).map_err(|e| {
                AiError::IndexError(format!("embedding deserialization: {e}"))
            })?;
            let score = cosine_similarity(query_embedding, &embedding);
            scored.push((score, chunk));
        }

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        Ok(scored
            .into_iter()
            .map(|(score, chunk)| SearchResult { chunk, score })
            .collect())
    }

    async fn delete_agent(&self, agent_id: &str) -> AiResult<()> {
        let db = self.db.lock().await;
        db.execute("DELETE FROM chunks WHERE agent_id = ?1", rusqlite::params![agent_id])
            .map_err(|e| AiError::IndexError(format!("SQLite delete error: {e}")))?;
        Ok(())
    }

    async fn has_index(&self, agent_id: &str) -> AiResult<bool> {
        let db = self.db.lock().await;
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE agent_id = ?1",
                rusqlite::params![agent_id],
                |row| row.get(0),
            )
            .map_err(|e| AiError::IndexError(format!("SQLite count error: {e}")))?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunks(agent_id: &str, n: usize) -> Vec<DocChunk> {
        (0..n)
            .map(|i| DocChunk {
                id: format!("{}:file.md:{}", agent_id, i * 100),
                content: format!("Chunk {} content about topic {}", i, i),
                source_path: "file.md".to_string(),
                byte_offset: i * 100,
                agent_id: agent_id.to_string(),
            })
            .collect()
    }

    fn make_embeddings(n: usize, dim: usize) -> Vec<Vec<f32>> {
        (0..n)
            .map(|i| {
                let mut v = vec![0.0f32; dim];
                v[i % dim] = 1.0; // one-hot-ish
                v
            })
            .collect()
    }

    // ── Cosine similarity ───────────────────────────────────────────

    #[test]
    fn cosine_identical_vectors_is_one() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_vectors_is_zero() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn cosine_zero_vector_returns_zero() {
        let a = vec![1.0, 2.0];
        let b = vec![0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    // ── InMemoryVectorStore ─────────────────────────────────────────

    #[tokio::test]
    async fn in_memory_upsert_and_search() {
        let store = InMemoryVectorStore::new();
        let chunks = make_chunks("a1", 3);
        let embeddings = make_embeddings(3, 4);

        store.upsert(&chunks, &embeddings).await.unwrap();

        // Search with first embedding — should find first chunk as top result.
        let results = store.search(&embeddings[0], "a1", 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk.id, chunks[0].id);
        assert!((results[0].score - 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn in_memory_has_index() {
        let store = InMemoryVectorStore::new();
        assert!(!store.has_index("a1").await.unwrap());

        let chunks = make_chunks("a1", 1);
        let embeddings = make_embeddings(1, 4);
        store.upsert(&chunks, &embeddings).await.unwrap();

        assert!(store.has_index("a1").await.unwrap());
        assert!(!store.has_index("other").await.unwrap());
    }

    #[tokio::test]
    async fn in_memory_delete_agent() {
        let store = InMemoryVectorStore::new();
        let chunks = make_chunks("a1", 2);
        let embeddings = make_embeddings(2, 4);
        store.upsert(&chunks, &embeddings).await.unwrap();

        store.delete_agent("a1").await.unwrap();
        assert!(!store.has_index("a1").await.unwrap());
    }

    #[tokio::test]
    async fn in_memory_agents_are_isolated() {
        let store = InMemoryVectorStore::new();
        let chunks_a = make_chunks("a1", 2);
        let chunks_b = make_chunks("b1", 3);
        let emb_a = make_embeddings(2, 4);
        let emb_b = make_embeddings(3, 4);

        store.upsert(&chunks_a, &emb_a).await.unwrap();
        store.upsert(&chunks_b, &emb_b).await.unwrap();

        let results = store.search(&emb_a[0], "a1", 10).await.unwrap();
        assert_eq!(results.len(), 2); // only a1's chunks

        store.delete_agent("a1").await.unwrap();
        assert!(store.has_index("b1").await.unwrap());
    }

    #[tokio::test]
    async fn in_memory_upsert_overwrites() {
        let store = InMemoryVectorStore::new();
        let chunks = make_chunks("a1", 1);
        let emb1 = vec![vec![1.0, 0.0, 0.0, 0.0]];
        store.upsert(&chunks, &emb1).await.unwrap();

        // Upsert same chunk with different embedding.
        let emb2 = vec![vec![0.0, 1.0, 0.0, 0.0]];
        store.upsert(&chunks, &emb2).await.unwrap();

        let results = store.search(&[0.0, 1.0, 0.0, 0.0], "a1", 1).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].score - 1.0).abs() < 1e-6, "should match updated embedding");
    }

    #[tokio::test]
    async fn in_memory_mismatched_lengths_error() {
        let store = InMemoryVectorStore::new();
        let chunks = make_chunks("a1", 2);
        let embeddings = make_embeddings(1, 4); // mismatch
        let result = store.upsert(&chunks, &embeddings).await;
        assert!(result.is_err());
    }

    // ── FileVectorStore ─────────────────────────────────────────────

    #[tokio::test]
    async fn file_store_upsert_and_search() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileVectorStore::new(dir.path());

        let chunks = make_chunks("a1", 3);
        let embeddings = make_embeddings(3, 4);
        store.upsert(&chunks, &embeddings).await.unwrap();

        // Verify file was created.
        assert!(dir.path().join("a1.index.json").is_file());

        let results = store.search(&embeddings[0], "a1", 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk.id, chunks[0].id);
    }

    #[tokio::test]
    async fn file_store_persists_across_instances() {
        let dir = tempfile::tempdir().unwrap();

        // Write with first instance.
        {
            let store = FileVectorStore::new(dir.path());
            let chunks = make_chunks("a1", 2);
            let embeddings = make_embeddings(2, 4);
            store.upsert(&chunks, &embeddings).await.unwrap();
        }

        // Read with a fresh instance.
        {
            let store = FileVectorStore::new(dir.path());
            assert!(store.has_index("a1").await.unwrap());
            let results = store
                .search(&make_embeddings(1, 4)[0], "a1", 10)
                .await
                .unwrap();
            assert_eq!(results.len(), 2);
        }
    }

    #[tokio::test]
    async fn file_store_delete_agent() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileVectorStore::new(dir.path());
        let chunks = make_chunks("a1", 1);
        let embeddings = make_embeddings(1, 4);
        store.upsert(&chunks, &embeddings).await.unwrap();

        store.delete_agent("a1").await.unwrap();
        assert!(!store.has_index("a1").await.unwrap());
        assert!(!dir.path().join("a1.index.json").is_file());
    }
}
