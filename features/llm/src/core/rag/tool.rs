/// Swebash RAG tool — wraps `RagIndexService` with score-transparency and
/// min-score threshold support.
///
/// This is a drop-in replacement for the vendored `llmrag::RagTool` that adds:
///
/// - **`show_scores`**: when `false`, omits `score: X.XXX` from the formatted output
///   so the LLM cannot use raw cosine similarity as a relevance gate.
/// - **`min_score`**: when `Some(threshold)`, results with `score < threshold` are
///   filtered before the LLM ever sees them.
use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use llmboot_orchestration::{RiskLevel, Tool, ToolCapability, ToolDefinition, ToolError, ToolOutput, ToolExecResult as ToolResult};

use llmrag::RagIndexService;

/// A RAG search tool with configurable score transparency and minimum-score filtering.
pub struct SwebashRagTool {
    agent_id: String,
    index_service: Arc<dyn RagIndexService>,
    top_k: usize,
    /// When `Some(t)`, drop results whose cosine similarity is below `t`.
    min_score: Option<f32>,
    /// When `true`, include `score: X.XXX` in each result header.
    show_scores: bool,
}

impl SwebashRagTool {
    /// Create a new `SwebashRagTool`.
    ///
    /// # Arguments
    ///
    /// * `agent_id` — the agent whose index to search.
    /// * `index_service` — the RAG index service (may be `PreprocessingRagIndexService`
    ///   or the shared `RagIndexManager`).
    /// * `top_k` — maximum results to return before filtering.
    /// * `min_score` — optional minimum cosine similarity threshold.
    /// * `show_scores` — whether to include scores in the formatted output.
    pub fn new(
        agent_id: impl Into<String>,
        index_service: Arc<dyn RagIndexService>,
        top_k: usize,
        min_score: Option<f32>,
        show_scores: bool,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            index_service,
            top_k,
            min_score,
            show_scores,
        }
    }
}

#[async_trait]
impl Tool for SwebashRagTool {
    fn name(&self) -> &str {
        "rag_search"
    }

    fn description(&self) -> &str {
        "Search the agent's documentation index for relevant information. \
         Use this tool to find specific details, API references, or \
         configuration examples from the loaded documentation."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query describing what information you need"
                }
            },
            "required": ["query"]
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::ReadOnly
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn default_timeout_ms(&self) -> u64 {
        30_000
    }

    async fn execute(&self, args: Value) -> ToolResult<ToolOutput> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if query.is_empty() {
            return Err(ToolError::InvalidArguments(
                "rag_search requires a non-empty 'query' parameter".to_string(),
            ));
        }

        let mut results = match self
            .index_service
            .search(&self.agent_id, &query, self.top_k)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return Ok(ToolOutput::text(format!("RAG search error: {e}")));
            }
        };

        // Apply minimum-score threshold filter.
        if let Some(min) = self.min_score {
            results.retain(|r| r.score >= min);
        }

        if results.is_empty() {
            return Ok(ToolOutput::text(
                "No relevant documentation found for your query.".to_string(),
            ));
        }

        let mut output = String::new();
        for (i, result) in results.iter().enumerate() {
            if self.show_scores {
                output.push_str(&format!(
                    "--- Result {} (score: {:.3}, source: {}) ---\n{}\n\n",
                    i + 1,
                    result.score,
                    result.chunk.source_path,
                    result.chunk.content
                ));
            } else {
                output.push_str(&format!(
                    "--- Result {} (source: {}) ---\n{}\n\n",
                    i + 1,
                    result.chunk.source_path,
                    result.chunk.content
                ));
            }
        }

        Ok(ToolOutput::text(output.trim_end().to_string()))
    }

    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
            // RAG search is read-only - no system capabilities required
            capabilities: ToolCapability::empty().bits(),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use llmrag::{DocChunk, RagResult, SearchResult};

    struct MockRagService {
        results: Vec<SearchResult>,
    }

    #[async_trait]
    impl RagIndexService for MockRagService {
        async fn ensure_index(
            &self,
            _agent_id: &str,
            _doc_sources: &[String],
            _base_dir: &Path,
        ) -> RagResult<()> {
            Ok(())
        }

        async fn search(
            &self,
            _agent_id: &str,
            _query: &str,
            _top_k: usize,
        ) -> RagResult<Vec<SearchResult>> {
            Ok(self.results.clone())
        }
    }

    fn make_result(score: f32, content: &str, source: &str) -> SearchResult {
        SearchResult {
            chunk: DocChunk {
                id: format!("test:{source}:0"),
                content: content.to_string(),
                source_path: source.to_string(),
                byte_offset: 0,
                agent_id: "test".to_string(),
            },
            score,
        }
    }

    fn text_of(output: ToolOutput) -> String {
        output.content.as_str().unwrap_or("").to_string()
    }

    // ── min_score filtering ─────────────────────────────────────────

    #[tokio::test]
    async fn test_min_score_removes_low_results() {
        let svc = Arc::new(MockRagService {
            results: vec![
                make_result(0.8, "High score content", "high.md"),
                make_result(0.2, "Low score content", "low.md"),
            ],
        });

        let tool = SwebashRagTool::new("test", svc, 5, Some(0.5), true);
        let out = text_of(tool.execute(serde_json::json!({"query": "x"})).await.unwrap());

        assert!(out.contains("High score content"), "got: {out:?}");
        assert!(!out.contains("Low score content"), "got: {out:?}");
    }

    #[tokio::test]
    async fn test_all_filtered_returns_no_results_message() {
        let svc = Arc::new(MockRagService {
            results: vec![make_result(0.1, "Low score content", "low.md")],
        });

        let tool = SwebashRagTool::new("test", svc, 5, Some(0.5), true);
        let out = text_of(tool.execute(serde_json::json!({"query": "x"})).await.unwrap());

        assert_eq!(out, "No relevant documentation found for your query.");
    }

    #[tokio::test]
    async fn test_empty_results_returns_no_results_message() {
        let svc = Arc::new(MockRagService { results: vec![] });

        let tool = SwebashRagTool::new("test", svc, 5, None, true);
        let out = text_of(tool.execute(serde_json::json!({"query": "x"})).await.unwrap());

        assert_eq!(out, "No relevant documentation found for your query.");
    }

    // ── show_scores formatting ──────────────────────────────────────

    #[tokio::test]
    async fn test_show_scores_false_omits_score_field() {
        let svc = Arc::new(MockRagService {
            results: vec![make_result(0.9, "Content here", "doc.md")],
        });

        let tool = SwebashRagTool::new("test", svc, 5, None, false);
        let out = text_of(tool.execute(serde_json::json!({"query": "x"})).await.unwrap());

        assert!(!out.contains("score:"), "score should be hidden: {out:?}");
        assert!(out.contains("source: doc.md"), "got: {out:?}");
        assert!(out.contains("Content here"), "got: {out:?}");
    }

    #[tokio::test]
    async fn test_show_scores_true_includes_score_field() {
        let svc = Arc::new(MockRagService {
            results: vec![make_result(0.9, "Content here", "doc.md")],
        });

        let tool = SwebashRagTool::new("test", svc, 5, None, true);
        let out = text_of(tool.execute(serde_json::json!({"query": "x"})).await.unwrap());

        assert!(out.contains("score:"), "score should be visible: {out:?}");
        assert!(out.contains("0.900"), "score value should be present: {out:?}");
        assert!(out.contains("Content here"), "got: {out:?}");
    }

    #[tokio::test]
    async fn test_show_scores_true_formats_score_to_three_decimals() {
        let svc = Arc::new(MockRagService {
            results: vec![make_result(0.12345, "Body", "f.md")],
        });

        let tool = SwebashRagTool::new("test", svc, 5, None, true);
        let out = text_of(tool.execute(serde_json::json!({"query": "x"})).await.unwrap());

        // {:.3} rounds 0.12345 → "0.123"
        assert!(out.contains("0.123"), "got: {out:?}");
    }

    // ── min_score = None passes all results ─────────────────────────

    #[tokio::test]
    async fn test_no_min_score_passes_all_results() {
        let svc = Arc::new(MockRagService {
            results: vec![
                make_result(0.05, "Very low", "a.md"),
                make_result(0.99, "Very high", "b.md"),
            ],
        });

        let tool = SwebashRagTool::new("test", svc, 5, None, false);
        let out = text_of(tool.execute(serde_json::json!({"query": "x"})).await.unwrap());

        assert!(out.contains("Very low"), "got: {out:?}");
        assert!(out.contains("Very high"), "got: {out:?}");
    }

    // ── result ordering preserved ───────────────────────────────────

    #[tokio::test]
    async fn test_result_numbering_is_sequential() {
        let svc = Arc::new(MockRagService {
            results: vec![
                make_result(0.9, "First", "a.md"),
                make_result(0.8, "Second", "b.md"),
                make_result(0.7, "Third", "c.md"),
            ],
        });

        let tool = SwebashRagTool::new("test", svc, 5, None, false);
        let out = text_of(tool.execute(serde_json::json!({"query": "x"})).await.unwrap());

        assert!(out.contains("Result 1"), "got: {out:?}");
        assert!(out.contains("Result 2"), "got: {out:?}");
        assert!(out.contains("Result 3"), "got: {out:?}");
    }
}
