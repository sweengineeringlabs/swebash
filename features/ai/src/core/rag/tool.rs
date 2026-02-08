/// RagTool — a rustratify `Tool` that agents invoke to search their
/// documentation index via RAG.
///
/// The LLM calls `rag_search` with a `{ "query": "..." }` argument.
/// The tool embeds the query, performs vector search, and returns the
/// top-k matching document chunks formatted as a readable text block.

use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use tool::{RiskLevel, Tool, ToolDefinition, ToolOutput, ToolResult, ToolError};

use super::index::RagIndexManager;

/// A tool that searches an agent's RAG index for relevant documentation.
pub struct RagTool {
    /// The agent ID whose index this tool searches.
    agent_id: String,
    /// The shared RAG index manager.
    index_manager: Arc<RagIndexManager>,
    /// Maximum number of results to return per query.
    top_k: usize,
}

impl RagTool {
    /// Create a new RagTool for the given agent.
    pub fn new(
        agent_id: impl Into<String>,
        index_manager: Arc<RagIndexManager>,
        top_k: usize,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            index_manager,
            top_k,
        }
    }
}

#[async_trait]
impl Tool for RagTool {
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

    async fn execute(&self, args: Value) -> ToolResult<ToolOutput> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidArguments("missing required 'query' parameter".to_string())
            })?;

        if query.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "'query' must not be empty".to_string(),
            ));
        }

        let results = self
            .index_manager
            .search(&self.agent_id, query, self.top_k)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("RAG search failed: {e}")))?;

        if results.is_empty() {
            return Ok(ToolOutput::text(
                "No relevant documentation found for your query.",
            ));
        }

        let mut output = String::new();
        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!(
                "--- Result {} (score: {:.3}, source: {}) ---\n{}\n\n",
                i + 1,
                result.score,
                result.chunk.source_path,
                result.chunk.content,
            ));
        }

        Ok(ToolOutput::text(output.trim_end()))
    }

    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }

    fn default_timeout_ms(&self) -> u64 {
        30_000
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::error::AiResult;
    use crate::core::rag::chunker::ChunkerConfig;
    use crate::core::rag::stores::InMemoryVectorStore;
    use crate::spi::rag::EmbeddingProvider;

    struct MockEmbedder;

    #[async_trait]
    impl EmbeddingProvider for MockEmbedder {
        async fn embed(&self, texts: &[String]) -> AiResult<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let mut v = vec![0.0f32; 4];
                    v[i % 4] = 1.0;
                    v
                })
                .collect())
        }

        fn dimension(&self) -> usize {
            4
        }

        fn model_name(&self) -> &str {
            "mock"
        }
    }

    async fn setup_tool() -> (RagTool, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let docs_dir = dir.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("api.md"), "The API endpoint is /v1/users. It returns a list of users.").unwrap();

        let store = Arc::new(InMemoryVectorStore::new());
        let embedder = Arc::new(MockEmbedder);
        let manager = Arc::new(super::super::index::RagIndexManager::new(
            embedder,
            store,
            ChunkerConfig::default(),
        ));

        manager
            .ensure_index("test-agent", &["docs/*.md".to_string()], dir.path())
            .await
            .unwrap();

        let tool = RagTool::new("test-agent", manager, 5);
        (tool, dir)
    }

    #[test]
    fn tool_metadata() {
        let store = Arc::new(InMemoryVectorStore::new());
        let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedder);
        let manager = Arc::new(super::super::index::RagIndexManager::new(
            embedder,
            store,
            ChunkerConfig::default(),
        ));
        let tool = RagTool::new("a1", manager, 5);

        assert_eq!(tool.name(), "rag_search");
        assert_eq!(tool.risk_level(), RiskLevel::ReadOnly);
        assert!(!tool.requires_confirmation());
        assert!(tool.description().contains("documentation"));
    }

    #[test]
    fn tool_schema_has_query() {
        let store = Arc::new(InMemoryVectorStore::new());
        let embedder: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedder);
        let manager = Arc::new(super::super::index::RagIndexManager::new(
            embedder,
            store,
            ChunkerConfig::default(),
        ));
        let tool = RagTool::new("a1", manager, 5);
        let schema = tool.parameters_schema();

        assert_eq!(schema["properties"]["query"]["type"], "string");
        assert!(schema["required"].as_array().unwrap().contains(&Value::String("query".into())));
    }

    #[tokio::test]
    async fn execute_returns_results() {
        let (tool, _dir) = setup_tool().await;

        let result = tool
            .execute(serde_json::json!({"query": "API endpoint"}))
            .await
            .unwrap();

        let text = result.content.as_str().expect("content should be a string");
        assert!(text.contains("Result 1"));
        assert!(text.contains("score:"));
    }

    #[tokio::test]
    async fn execute_missing_query_is_error() {
        let (tool, _dir) = setup_tool().await;

        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn execute_empty_query_is_error() {
        let (tool, _dir) = setup_tool().await;

        let result = tool.execute(serde_json::json!({"query": "  "})).await;
        assert!(result.is_err());
    }
}
