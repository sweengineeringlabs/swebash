/// RagIndexManager — orchestrates building, refreshing, and querying
/// per-agent RAG indexes.
///
/// The manager owns an `EmbeddingProvider` and a `VectorStore`, and uses
/// the `chunker` module to split documents before indexing.  Staleness is
/// tracked via a SHA-256 fingerprint of `(path, mtime, size)` tuples so
/// re-indexing only happens when source files change.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use crate::api::error::{AiError, AiResult};
use crate::spi::rag::{EmbeddingProvider, SearchResult, VectorStore};

use super::chunker::{self, ChunkerConfig};

/// Manages RAG indexes for all agents that use the `rag` docs strategy.
pub struct RagIndexManager {
    embedder: Arc<dyn EmbeddingProvider>,
    store: Arc<dyn VectorStore>,
    chunker_config: ChunkerConfig,
    /// Per-agent fingerprint tracking.  Key = agent_id, Value = hex SHA-256.
    index_state: Arc<RwLock<HashMap<String, String>>>,
}

impl RagIndexManager {
    /// Create a new index manager.
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

    /// Ensure the index for `agent_id` is up to date.
    ///
    /// Resolves `doc_sources` (glob patterns) relative to `base_dir`, computes
    /// a fingerprint of the resulting files, and re-indexes only when the
    /// fingerprint has changed (or no index exists yet).
    pub async fn ensure_index(
        &self,
        agent_id: &str,
        doc_sources: &[String],
        base_dir: &Path,
    ) -> AiResult<()> {
        let files = resolve_sources(doc_sources, base_dir)?;
        if files.is_empty() {
            tracing::debug!(agent = %agent_id, "no doc files resolved, skipping index build");
            return Ok(());
        }

        let fingerprint = compute_fingerprint(&files)?;

        // Check if already indexed with this fingerprint.
        {
            let state = self.index_state.read().await;
            if let Some(existing) = state.get(agent_id) {
                if *existing == fingerprint {
                    tracing::debug!(agent = %agent_id, "index is current, skipping rebuild");
                    return Ok(());
                }
            }
        }

        tracing::info!(
            agent = %agent_id,
            files = files.len(),
            "building RAG index"
        );

        // Clear stale data.
        self.store.delete_agent(agent_id).await?;

        // Chunk all files.
        let mut all_chunks = Vec::new();
        for (path, rel_path) in &files {
            let content = std::fs::read_to_string(path).map_err(|e| {
                AiError::IndexError(format!("failed to read {}: {e}", path.display()))
            })?;
            let chunks =
                chunker::chunk_text(&content, rel_path, agent_id, &self.chunker_config);
            all_chunks.extend(chunks);
        }

        if all_chunks.is_empty() {
            tracing::warn!(agent = %agent_id, "chunking produced zero chunks");
            return Ok(());
        }

        // Embed all chunks in batches.
        let texts: Vec<String> = all_chunks.iter().map(|c| c.content.clone()).collect();
        let embeddings = self.embedder.embed(&texts).await?;

        if embeddings.len() != all_chunks.len() {
            return Err(AiError::IndexError(format!(
                "embedding count ({}) doesn't match chunk count ({})",
                embeddings.len(),
                all_chunks.len()
            )));
        }

        // Store.
        self.store.upsert(&all_chunks, &embeddings).await?;

        // Update fingerprint.
        {
            let mut state = self.index_state.write().await;
            state.insert(agent_id.to_string(), fingerprint);
        }

        tracing::info!(
            agent = %agent_id,
            chunks = all_chunks.len(),
            "RAG index built successfully"
        );

        Ok(())
    }

    /// Search the agent's index for chunks relevant to `query`.
    pub async fn search(
        &self,
        agent_id: &str,
        query: &str,
        top_k: usize,
    ) -> AiResult<Vec<SearchResult>> {
        let query_embedding = self
            .embedder
            .embed(&[query.to_string()])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| AiError::IndexError("embedding returned empty result".to_string()))?;

        self.store.search(&query_embedding, agent_id, top_k).await
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Resolve glob patterns to concrete file paths.
///
/// Returns `(absolute_path, relative_display_path)` pairs.
fn resolve_sources(
    sources: &[String],
    base_dir: &Path,
) -> AiResult<Vec<(std::path::PathBuf, String)>> {
    let mut files = Vec::new();

    for pattern in sources {
        let full_pattern = base_dir.join(pattern).to_string_lossy().to_string();
        let paths = glob::glob(&full_pattern).map_err(|e| {
            AiError::IndexError(format!("invalid glob pattern '{}': {e}", pattern))
        })?;

        for entry in paths {
            let path = match entry {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "glob entry error, skipping");
                    continue;
                }
            };

            if path.is_file() {
                let rel = path
                    .strip_prefix(base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                files.push((path, rel));
            }
        }
    }

    Ok(files)
}

/// Compute a SHA-256 fingerprint over a set of files.
///
/// The fingerprint incorporates each file's path, modification time, and
/// size, so it changes whenever a source file is added, removed, or modified.
fn compute_fingerprint(files: &[(std::path::PathBuf, String)]) -> AiResult<String> {
    let mut hasher = Sha256::new();

    for (path, rel) in files {
        hasher.update(rel.as_bytes());

        match std::fs::metadata(path) {
            Ok(meta) => {
                let size = meta.len();
                hasher.update(size.to_le_bytes());

                if let Ok(mtime) = meta.modified() {
                    if let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH) {
                        hasher.update(duration.as_secs().to_le_bytes());
                    }
                }
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to stat file for fingerprint");
                hasher.update(b"unknown");
            }
        }
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spi::rag::{DocChunk, EmbeddingProvider, VectorStore};
    use async_trait::async_trait;

    /// A mock embedding provider that returns deterministic vectors.
    struct MockEmbedder {
        dim: usize,
    }

    #[async_trait]
    impl EmbeddingProvider for MockEmbedder {
        async fn embed(&self, texts: &[String]) -> AiResult<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let mut v = vec![0.0f32; self.dim];
                    v[i % self.dim] = 1.0;
                    v
                })
                .collect())
        }

        fn dimension(&self) -> usize {
            self.dim
        }

        fn model_name(&self) -> &str {
            "mock"
        }
    }

    #[tokio::test]
    async fn ensure_index_builds_from_files() {
        let dir = tempfile::tempdir().unwrap();
        let docs_dir = dir.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("a.md"), "Hello world. This is doc A.").unwrap();
        std::fs::write(docs_dir.join("b.md"), "Goodbye world. This is doc B.").unwrap();

        let store = Arc::new(crate::core::rag::stores::InMemoryVectorStore::new());
        let embedder = Arc::new(MockEmbedder { dim: 8 });
        let manager = RagIndexManager::new(
            embedder,
            store.clone(),
            ChunkerConfig::default(),
        );

        let sources = vec!["docs/*.md".to_string()];
        manager.ensure_index("test-agent", &sources, dir.path()).await.unwrap();

        assert!(store.has_index("test-agent").await.unwrap());
    }

    #[tokio::test]
    async fn ensure_index_skips_rebuild_when_current() {
        let dir = tempfile::tempdir().unwrap();
        let docs_dir = dir.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("a.md"), "Content.").unwrap();

        let store = Arc::new(crate::core::rag::stores::InMemoryVectorStore::new());
        let embedder = Arc::new(MockEmbedder { dim: 4 });
        let manager = RagIndexManager::new(
            embedder,
            store.clone(),
            ChunkerConfig::default(),
        );

        let sources = vec!["docs/*.md".to_string()];
        manager.ensure_index("a1", &sources, dir.path()).await.unwrap();

        // Second call should be a no-op (same fingerprint).
        manager.ensure_index("a1", &sources, dir.path()).await.unwrap();

        // Verify state has the fingerprint.
        let state = manager.index_state.read().await;
        assert!(state.contains_key("a1"));
    }

    #[tokio::test]
    async fn search_returns_results() {
        let dir = tempfile::tempdir().unwrap();
        let docs_dir = dir.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("a.md"), "The quick brown fox.").unwrap();

        let store = Arc::new(crate::core::rag::stores::InMemoryVectorStore::new());
        let embedder = Arc::new(MockEmbedder { dim: 4 });
        let manager = RagIndexManager::new(
            embedder,
            store,
            ChunkerConfig::default(),
        );

        let sources = vec!["docs/*.md".to_string()];
        manager.ensure_index("a1", &sources, dir.path()).await.unwrap();

        let results = manager.search("a1", "fox", 5).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn empty_sources_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(crate::core::rag::stores::InMemoryVectorStore::new());
        let embedder = Arc::new(MockEmbedder { dim: 4 });
        let manager = RagIndexManager::new(
            embedder,
            store.clone(),
            ChunkerConfig::default(),
        );

        manager.ensure_index("a1", &[], dir.path()).await.unwrap();
        assert!(!store.has_index("a1").await.unwrap());
    }

    #[test]
    fn fingerprint_changes_with_different_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello").unwrap();

        let files1 = vec![(dir.path().join("a.txt"), "a.txt".to_string())];
        let fp1 = compute_fingerprint(&files1).unwrap();

        std::fs::write(dir.path().join("b.txt"), "world").unwrap();
        let files2 = vec![
            (dir.path().join("a.txt"), "a.txt".to_string()),
            (dir.path().join("b.txt"), "b.txt".to_string()),
        ];
        let fp2 = compute_fingerprint(&files2).unwrap();

        assert_ne!(fp1, fp2);
    }
}
