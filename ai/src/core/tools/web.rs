/// Web search tool implementation
///
/// Provides web search capabilities using DuckDuckGo's HTML interface.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use llm_provider::ToolDefinition;
use super::{ToolExecutor, ToolError, ToolResult};

/// Web search tool using DuckDuckGo
pub struct WebSearchTool {
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
        }
    }

    async fn search(&self, query: &str, num_results: usize) -> ToolResult<String> {
        // Validate query
        if query.is_empty() {
            return Err(ToolError::InvalidArguments(
                "Query cannot be empty".to_string()
            ));
        }

        if query.len() > 500 {
            return Err(ToolError::InvalidArguments(
                "Query too long (max 500 characters)".to_string()
            ));
        }

        // Limit results
        let num_results = num_results.min(10);

        // Use DuckDuckGo's instant answer API
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1",
            urlencoding::encode(query)
        );

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!(
                "Failed to fetch search results: {}",
                e
            )))?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "Search request failed with status: {}",
                response.status()
            )));
        }

        let data: DuckDuckGoResponse = response
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!(
                "Failed to parse search results: {}",
                e
            )))?;

        // Extract results
        let mut results = Vec::new();

        // Add abstract if available
        if !data.abstract_text.is_empty() {
            results.push(json!({
                "title": data.heading.unwrap_or_else(|| "Answer".to_string()),
                "url": data.abstract_url.unwrap_or_default(),
                "snippet": data.abstract_text
            }));
        }

        // Add related topics
        for topic in data.related_topics.iter().take(num_results.saturating_sub(results.len())) {
            if let Some(url) = &topic.first_url {
                results.push(json!({
                    "title": topic.text.split(" - ").next().unwrap_or(&topic.text),
                    "url": url,
                    "snippet": topic.text
                }));
            }
        }

        // If no results, return informative message
        if results.is_empty() {
            return Ok(json!({
                "success": true,
                "query": query,
                "results": [],
                "message": "No instant answers found. The query might be too specific or DuckDuckGo has no instant answer for it."
            }).to_string());
        }

        Ok(json!({
            "success": true,
            "query": query,
            "results": results
        }).to_string())
    }
}

#[derive(Debug, Deserialize)]
struct SearchArgs {
    query: String,
    #[serde(default = "default_num_results")]
    num_results: usize,
}

fn default_num_results() -> usize {
    5
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DuckDuckGoResponse {
    #[serde(default)]
    abstract_text: String,
    #[serde(default)]
    abstract_url: Option<String>,
    #[serde(default)]
    heading: Option<String>,
    #[serde(default)]
    related_topics: Vec<RelatedTopic>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RelatedTopic {
    #[serde(default)]
    text: String,
    #[serde(default)]
    first_url: Option<String>,
}

#[async_trait]
impl ToolExecutor for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_search".to_string(),
            description: "Search the web for information using DuckDuckGo. Returns relevant results with titles, URLs, and snippets. Best for factual queries, definitions, and general information.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "num_results": {
                        "type": "integer",
                        "description": "Number of results to return (default: 5, max: 10)",
                        "default": 5
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, arguments: &str) -> ToolResult<String> {
        let args: SearchArgs = serde_json::from_str(arguments)?;
        self.search(&args.query, args.num_results).await
    }

    fn requires_confirmation(&self) -> bool {
        false // Web search is safe
    }

    fn describe_call(&self, arguments: &str) -> String {
        if let Ok(args) = serde_json::from_str::<SearchArgs>(arguments) {
            format!("Search web: {}", args.query)
        } else {
            "Web search".to_string()
        }
    }
}

// Add urlencoding module for encoding query parameters
mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}
