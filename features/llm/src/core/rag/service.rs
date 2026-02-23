/// `PreprocessingRagIndexService` — a `RagIndexService` implementation that
/// applies a preprocessing hook (markdown-table normalization) between file
/// reading and chunking.
///
/// Functionally equivalent to `llmrag::RagIndexManager` except that file
/// content is passed through [`normalize_markdown_tables`] before being
/// chunked and embedded.  The same `Arc<dyn VectorStore>` and
/// `Arc<dyn EmbeddingProvider>` that back the global `RagIndexManager` are
/// shared here to avoid duplicate model loading and storage.
///
/// # Fingerprinting
///
/// Uses the same two-level fingerprint strategy as `RagIndexManager`:
/// - An in-memory `HashMap<agent_id, fingerprint>` avoids redundant work
///   within a single process run.
/// - `VectorStore::load_fingerprint` / `save_fingerprint` provide
///   cross-restart persistence (no-op for `InMemoryVectorStore`).
///
/// The fingerprint is a SHA-256 hex digest of each resolved file's
/// `(relative_path, mtime_secs, size_bytes)` tuple, sorted for stability.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use llmrag::{
    ChunkerConfig, EmbeddingProvider, RagError, RagIndexService, RagResult, SearchResult,
    VectorStore, chunk_text,
};

use crate::core::rag::normalize::normalize_markdown_tables;

/// `RagIndexService` implementation with markdown-table preprocessing.
pub struct PreprocessingRagIndexService {
    embedder: Arc<dyn EmbeddingProvider>,
    store: Arc<dyn VectorStore>,
    chunker_config: ChunkerConfig,
    /// In-memory fingerprint cache: `agent_id → fingerprint`.
    index_state: Arc<RwLock<HashMap<String, String>>>,
}

impl PreprocessingRagIndexService {
    /// Create a new service sharing the given embedding provider and vector store.
    pub fn new(
        embedder: Arc<dyn EmbeddingProvider>,
        store: Arc<dyn VectorStore>,
        chunker_config: ChunkerConfig,
    ) -> Self {
        Self {
            embedder,
            store,
            chunker_config,
            index_state: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl RagIndexService for PreprocessingRagIndexService {
    /// Ensure the preprocessed index for `agent_id` is up to date.
    ///
    /// Skips rebuild when the file-set fingerprint matches the cached value.
    /// When a rebuild is needed, files are read, normalized via
    /// [`normalize_markdown_tables`], chunked, and embedded before being
    /// stored.
    async fn ensure_index(
        &self,
        agent_id: &str,
        doc_sources: &[String],
        base_dir: &Path,
    ) -> RagResult<()> {
        // 1. Resolve globs to concrete file paths.
        let mut resolved: Vec<PathBuf> = Vec::new();
        for pattern in doc_sources {
            let full = base_dir.join(pattern).to_string_lossy().to_string();
            let entries = glob::glob(&full).map_err(|e| {
                RagError::Index(format!("invalid glob pattern '{pattern}': {e}"))
            })?;
            for entry in entries {
                match entry {
                    Ok(p) if p.is_file() => resolved.push(p),
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "glob entry error, skipping");
                    }
                }
            }
        }
        // Sort for a stable fingerprint regardless of glob iteration order.
        resolved.sort();

        // 2. Compute fingerprint of (rel_path, mtime, size) tuples.
        let fingerprint = compute_fingerprint(&resolved, base_dir);

        // 3. Check in-memory cache.
        {
            let state = self.index_state.read().await;
            if state.get(agent_id).map(|f| f == &fingerprint).unwrap_or(false) {
                tracing::debug!(agent = %agent_id, "RAG index up to date (memory cache)");
                return Ok(());
            }
        }

        // 4. Check persisted fingerprint.
        if let Ok(Some(stored)) = self.store.load_fingerprint(agent_id).await {
            if stored == fingerprint {
                tracing::debug!(agent = %agent_id, "RAG index up to date (persisted fingerprint)");
                let mut state = self.index_state.write().await;
                state.insert(agent_id.to_string(), fingerprint);
                return Ok(());
            }
        }

        tracing::info!(
            agent = %agent_id,
            files = resolved.len(),
            "rebuilding preprocessed RAG index"
        );

        // 5. Delete existing index for this agent.
        self.store.delete_agent(agent_id).await?;

        // 6. Read, normalize, and chunk each file.
        let mut all_chunks = Vec::new();
        for path in &resolved {
            let raw = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "failed to read file for RAG indexing, skipping"
                    );
                    continue;
                }
            };
            let processed = normalize_markdown_tables(&raw);
            let rel = path.strip_prefix(base_dir).unwrap_or(path.as_path());
            let rel_str = rel.to_string_lossy().to_string();
            let chunks = chunk_text(&processed, &rel_str, agent_id, &self.chunker_config);
            all_chunks.extend(chunks);
        }

        if all_chunks.is_empty() {
            // Nothing to embed — still save the fingerprint so we don't retry.
            self.store.save_fingerprint(agent_id, &fingerprint).await?;
            let mut state = self.index_state.write().await;
            state.insert(agent_id.to_string(), fingerprint);
            return Ok(());
        }

        // 7. Embed all chunks in a single batch.
        let texts: Vec<String> = all_chunks.iter().map(|c| c.content.clone()).collect();
        let embeddings = self.embedder.embed(&texts).await?;

        // 8. Upsert into the vector store.
        self.store.upsert(&all_chunks, &embeddings).await?;

        // 9. Persist the fingerprint.
        self.store.save_fingerprint(agent_id, &fingerprint).await?;

        // 10. Update the in-memory cache.
        {
            let mut state = self.index_state.write().await;
            state.insert(agent_id.to_string(), fingerprint);
        }

        tracing::info!(
            agent = %agent_id,
            chunks = all_chunks.len(),
            "preprocessed RAG index built"
        );

        Ok(())
    }

    /// Search the agent's index.  Identical to `RagIndexManager::search`.
    async fn search(
        &self,
        agent_id: &str,
        query: &str,
        top_k: usize,
    ) -> RagResult<Vec<SearchResult>> {
        let embeddings = self.embedder.embed(&[query.to_string()]).await?;
        let query_embedding = embeddings.into_iter().next().ok_or_else(|| {
            RagError::Embedding("empty embedding response for query".to_string())
        })?;
        self.store.search(&query_embedding, agent_id, top_k).await
    }
}

/// Compute a stable SHA-256 fingerprint of a sorted list of files.
///
/// Each file contributes its relative path, last-modified time (seconds),
/// and size to the hash.  Files that cannot be stat'd contribute only their
/// path.
fn compute_fingerprint(files: &[PathBuf], base_dir: &Path) -> String {
    let mut hasher = Sha256::new();
    for path in files {
        let rel = path.strip_prefix(base_dir).unwrap_or(path.as_path());
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update(b"\x00"); // null separator between path and metadata
        if let Ok(meta) = path.metadata() {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            hasher.update(mtime.to_le_bytes());
            hasher.update(meta.len().to_le_bytes());
        }
        hasher.update(b"\x01"); // record separator
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use llmrag::{DocChunk, RagResult, SearchResult};

    // ── Mock EmbeddingProvider ──────────────────────────────────────

    struct MockEmbedder {
        dimension: usize,
    }

    impl MockEmbedder {
        fn new() -> Self {
            Self { dimension: 4 }
        }
    }

    #[async_trait]
    impl EmbeddingProvider for MockEmbedder {
        async fn embed(&self, texts: &[String]) -> RagResult<Vec<Vec<f32>>> {
            // Return a deterministic fixed-length vector per text.
            Ok(texts.iter().map(|_| vec![1.0_f32; self.dimension]).collect())
        }

        fn dimension(&self) -> usize {
            self.dimension
        }

        fn model_name(&self) -> &str {
            "mock-embedder"
        }
    }

    // ── Capturing VectorStore ───────────────────────────────────────

    #[derive(Default)]
    struct CapturingStore {
        upserted: Arc<Mutex<Vec<DocChunk>>>,
        fingerprints: Arc<Mutex<HashMap<String, String>>>,
        upsert_count: Arc<Mutex<usize>>,
    }

    impl CapturingStore {
        fn new() -> Self {
            Self::default()
        }
    }

    #[async_trait]
    impl VectorStore for CapturingStore {
        async fn upsert(
            &self,
            chunks: &[DocChunk],
            _embeddings: &[Vec<f32>],
        ) -> RagResult<()> {
            let mut stored = self.upserted.lock().unwrap();
            stored.extend_from_slice(chunks);
            *self.upsert_count.lock().unwrap() += 1;
            Ok(())
        }

        async fn search(
            &self,
            _query: &[f32],
            _agent_id: &str,
            _top_k: usize,
        ) -> RagResult<Vec<SearchResult>> {
            Ok(vec![])
        }

        async fn delete_agent(&self, _agent_id: &str) -> RagResult<()> {
            let mut stored = self.upserted.lock().unwrap();
            stored.clear();
            Ok(())
        }

        async fn has_index(&self, _agent_id: &str) -> RagResult<bool> {
            Ok(!self.upserted.lock().unwrap().is_empty())
        }

        async fn load_fingerprint(&self, agent_id: &str) -> RagResult<Option<String>> {
            Ok(self.fingerprints.lock().unwrap().get(agent_id).cloned())
        }

        async fn save_fingerprint(
            &self,
            agent_id: &str,
            fingerprint: &str,
        ) -> RagResult<()> {
            self.fingerprints
                .lock()
                .unwrap()
                .insert(agent_id.to_string(), fingerprint.to_string());
            Ok(())
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────

    fn make_service(store: Arc<CapturingStore>) -> PreprocessingRagIndexService {
        PreprocessingRagIndexService::new(
            Arc::new(MockEmbedder::new()),
            store,
            ChunkerConfig::default(),
        )
    }

    // ── Tests ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_ensure_index_skips_rebuild_on_same_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doc.md"), "Hello world.").unwrap();

        let store = Arc::new(CapturingStore::new());
        let svc = make_service(store.clone());

        // First call — builds index.
        svc.ensure_index("agent1", &["doc.md".to_string()], dir.path())
            .await
            .unwrap();
        let first_count = *store.upsert_count.lock().unwrap();
        assert_eq!(first_count, 1, "expected one upsert on initial build");

        // Second call — same files → should skip (in-memory cache hit).
        svc.ensure_index("agent1", &["doc.md".to_string()], dir.path())
            .await
            .unwrap();
        let second_count = *store.upsert_count.lock().unwrap();
        assert_eq!(second_count, 1, "second call should not trigger upsert");
    }

    #[tokio::test]
    async fn test_ensure_index_rebuilds_on_file_change() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("doc.md");
        std::fs::write(&file_path, "Original content.").unwrap();

        let store = Arc::new(CapturingStore::new());
        let svc = make_service(store.clone());

        // Build initial index.
        svc.ensure_index("agent1", &["doc.md".to_string()], dir.path())
            .await
            .unwrap();
        assert_eq!(*store.upsert_count.lock().unwrap(), 1);

        // Modify the file — sleep briefly to ensure mtime changes.
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&file_path, "Updated content.").unwrap();

        // Clear the in-memory cache by creating a new service (simulates restart).
        let svc2 = make_service(store.clone());
        svc2.ensure_index("agent1", &["doc.md".to_string()], dir.path())
            .await
            .unwrap();
        assert_eq!(
            *store.upsert_count.lock().unwrap(),
            2,
            "should rebuild after file modification"
        );
    }

    #[tokio::test]
    async fn test_ensure_index_persisted_fingerprint_skips_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doc.md"), "Stable content.").unwrap();

        let store = Arc::new(CapturingStore::new());

        // First service instance: build index.
        {
            let svc = make_service(store.clone());
            svc.ensure_index("agent1", &["doc.md".to_string()], dir.path())
                .await
                .unwrap();
        }
        assert_eq!(*store.upsert_count.lock().unwrap(), 1);

        // Second service instance: should pick up persisted fingerprint and skip.
        {
            let svc2 = make_service(store.clone());
            svc2.ensure_index("agent1", &["doc.md".to_string()], dir.path())
                .await
                .unwrap();
        }
        assert_eq!(
            *store.upsert_count.lock().unwrap(),
            1,
            "persisted fingerprint should prevent rebuild"
        );
    }

    #[tokio::test]
    async fn test_normalize_markdown_preprocesses_before_chunking() {
        let dir = tempfile::tempdir().unwrap();
        let table_content = "| PORT | 8080 | HTTP listen port |\n\
                              |------|------|------------------|\n\
                              | HOST | localhost | Bind address |\n";
        std::fs::write(dir.path().join("config.md"), table_content).unwrap();

        let store = Arc::new(CapturingStore::new());
        let svc = make_service(store.clone());

        svc.ensure_index("agent1", &["config.md".to_string()], dir.path())
            .await
            .unwrap();

        let chunks = store.upserted.lock().unwrap().clone();
        assert!(!chunks.is_empty(), "expected at least one chunk");

        let all_content: String = chunks.iter().map(|c| c.content.as_str()).collect::<Vec<_>>().join("\n");

        // After normalization, table rows become prose — no raw pipe characters
        // should remain in the embedded content.
        assert!(
            !all_content.contains('|'),
            "table pipes should be normalized away: {all_content:?}"
        );
        // And the prose form should appear.
        assert!(
            all_content.contains("8080") || all_content.contains("PORT"),
            "normalized content should reference port info: {all_content:?}"
        );
    }

    #[tokio::test]
    async fn test_ensure_index_with_no_sources_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(CapturingStore::new());
        let svc = make_service(store.clone());

        // No sources → no files → no chunks, but no error.
        svc.ensure_index("agent1", &[], dir.path())
            .await
            .unwrap();

        assert_eq!(*store.upsert_count.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_search_delegates_to_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(CapturingStore::new());
        let svc = make_service(store.clone());

        // Search on an empty store returns empty results (no panic).
        let results = svc.search("agent1", "what is the port?", 5).await.unwrap();
        assert!(results.is_empty());
    }
}
