/// Decorator that logs every LLM request/response to JSON files.
///
/// When a `log_dir` is configured, `LoggingLlmService` wraps an inner
/// `LlmService` and writes one JSON file per `complete()` or
/// `complete_stream()` call. Other methods are passed through.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures::stream::StreamExt;
use serde::Serialize;

use llm_provider::{
    CompletionRequest, CompletionResponse, CompletionStream, LlmResult, LlmService,
    ModelInfo, StreamChunk,
};

// ── Public API ───────────────────────────────────────────────────────────

/// Logging decorator for `LlmService`.
pub struct LoggingLlmService {
    inner: Arc<dyn LlmService>,
    log_dir: PathBuf,
}

impl LoggingLlmService {
    /// Conditionally wrap an `LlmService` with logging.
    ///
    /// Returns the inner service unchanged when `log_dir` is `None`,
    /// or a logging wrapper when `Some`.
    pub fn wrap(inner: Arc<dyn LlmService>, log_dir: Option<PathBuf>) -> Arc<dyn LlmService> {
        match log_dir {
            Some(dir) => Arc::new(Self {
                inner,
                log_dir: dir,
            }),
            None => inner,
        }
    }
}

#[async_trait]
impl LlmService for LoggingLlmService {
    async fn complete(&self, request: CompletionRequest) -> LlmResult<CompletionResponse> {
        let id = format!("{}-complete", uuid::Uuid::new_v4());
        let timestamp = epoch_ms();
        let request_value = serde_json::to_value(&request).unwrap_or_default();
        let start = Instant::now();

        let result = self.inner.complete(request).await;
        let duration_ms = start.elapsed().as_millis();

        let log_result = match &result {
            Ok(resp) => LogResult::Success {
                response: serde_json::to_value(resp).unwrap_or_default(),
            },
            Err(e) => LogResult::Error {
                error: e.to_string(),
            },
        };

        let entry = LogEntry {
            id: id.clone(),
            timestamp_epoch_ms: timestamp,
            duration_ms,
            kind: "complete",
            request: request_value,
            result: log_result,
        };

        write_log_entry(self.log_dir.clone(), id, entry);

        result
    }

    async fn complete_stream(&self, request: CompletionRequest) -> LlmResult<CompletionStream> {
        let id = format!("{}-stream", uuid::Uuid::new_v4());
        let timestamp = epoch_ms();
        let request_value = serde_json::to_value(&request).unwrap_or_default();
        let start = Instant::now();

        let result = self.inner.complete_stream(request).await;

        match result {
            Ok(inner_stream) => {
                let log_dir = self.log_dir.clone();
                let id_clone = id.clone();
                let chunks: Arc<parking_lot::Mutex<Vec<StreamChunk>>> =
                    Arc::new(parking_lot::Mutex::new(Vec::new()));
                let chunks_ref = chunks.clone();

                let mapped = inner_stream.map(move |chunk_result| {
                    if let Ok(ref chunk) = chunk_result {
                        chunks_ref.lock().push(chunk.clone());
                    }
                    chunk_result
                });

                // StreamLogger wraps the mapped stream directly.
                // The Drop impl flushes accumulated chunks when the stream
                // is fully consumed or dropped early.
                let stream = StreamLogger {
                    inner: Box::pin(mapped),
                    chunks,
                    log_dir,
                    id: id_clone,
                    timestamp,
                    start,
                    request: request_value,
                    finished: false,
                };

                Ok(Box::pin(stream))
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis();
                let entry = LogEntry {
                    id: id.clone(),
                    timestamp_epoch_ms: timestamp,
                    duration_ms,
                    kind: "complete_stream",
                    request: request_value,
                    result: LogResult::Error {
                        error: e.to_string(),
                    },
                };
                write_log_entry(self.log_dir.clone(), id, entry);
                Err(e)
            }
        }
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        self.inner.list_models().await
    }

    async fn model_info(&self, model: &str) -> LlmResult<ModelInfo> {
        self.inner.model_info(model).await
    }

    async fn is_model_available(&self, model: &str) -> bool {
        self.inner.is_model_available(model).await
    }

    async fn providers(&self) -> Vec<String> {
        self.inner.providers().await
    }
}

// ── Stream logger ────────────────────────────────────────────────────────

/// A stream wrapper that accumulates chunks and logs on completion/drop.
struct StreamLogger {
    inner: std::pin::Pin<Box<dyn futures::Stream<Item = LlmResult<StreamChunk>> + Send>>,
    chunks: Arc<parking_lot::Mutex<Vec<StreamChunk>>>,
    log_dir: PathBuf,
    id: String,
    timestamp: u128,
    start: Instant,
    request: serde_json::Value,
    finished: bool,
}

impl futures::Stream for StreamLogger {
    type Item = LlmResult<StreamChunk>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;

        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(item)),
            Poll::Ready(None) => {
                // Stream ended — flush log
                if !self.finished {
                    self.finished = true;
                    self.flush_log();
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl StreamLogger {
    fn flush_log(&self) {
        let duration_ms = self.start.elapsed().as_millis();
        let chunks = self.chunks.lock().clone();
        let assembled = assemble_stream_response(&chunks);

        let entry = LogEntry {
            id: self.id.clone(),
            timestamp_epoch_ms: self.timestamp,
            duration_ms,
            kind: "complete_stream",
            request: self.request.clone(),
            result: LogResult::Success {
                response: serde_json::to_value(&assembled).unwrap_or_default(),
            },
        };

        write_log_entry(self.log_dir.clone(), self.id.clone(), entry);
    }
}

impl Drop for StreamLogger {
    fn drop(&mut self) {
        if !self.finished {
            self.finished = true;
            self.flush_log();
        }
    }
}

// ── Log entry types ──────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct LogEntry {
    pub id: String,
    pub timestamp_epoch_ms: u128,
    pub duration_ms: u128,
    pub kind: &'static str,
    pub request: serde_json::Value,
    pub result: LogResult,
}

#[derive(Serialize)]
#[serde(tag = "status")]
pub(crate) enum LogResult {
    #[serde(rename = "success")]
    Success { response: serde_json::Value },
    #[serde(rename = "error")]
    Error { error: String },
}

/// Assembled summary of a streamed response for logging.
#[derive(Debug, Serialize)]
pub(crate) struct AssembledStreamResponse {
    pub content: Option<String>,
    pub tool_call_deltas: Vec<serde_json::Value>,
    pub finish_reason: Option<String>,
    pub chunk_count: usize,
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Fire-and-forget write of a log entry to a JSON file.
fn write_log_entry(log_dir: PathBuf, id: String, entry: LogEntry) {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            tracing::warn!("Failed to create AI log directory {}: {e}", log_dir.display());
            return;
        }
        let path = log_dir.join(format!("{id}.json"));
        match serde_json::to_string_pretty(&entry) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::warn!("Failed to write AI log file {}: {e}", path.display());
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize AI log entry: {e}");
            }
        }
    });
}

/// Assemble accumulated stream chunks into a summary for logging.
pub(crate) fn assemble_stream_response(chunks: &[StreamChunk]) -> AssembledStreamResponse {
    let mut content_parts: Vec<String> = Vec::new();
    let mut tool_call_deltas: Vec<serde_json::Value> = Vec::new();
    let mut finish_reason: Option<String> = None;

    for chunk in chunks {
        if let Some(ref text) = chunk.delta.content {
            content_parts.push(text.clone());
        }
        if let Some(ref tcs) = chunk.delta.tool_calls {
            for tc in tcs {
                if let Ok(val) = serde_json::to_value(tc) {
                    tool_call_deltas.push(val);
                }
            }
        }
        if let Some(ref reason) = chunk.finish_reason {
            finish_reason = Some(format!("{:?}", reason));
        }
    }

    let content = if content_parts.is_empty() {
        None
    } else {
        Some(content_parts.concat())
    };

    AssembledStreamResponse {
        content,
        tool_call_deltas,
        finish_reason,
        chunk_count: chunks.len(),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use llm_provider::MockLlmService;

    #[test]
    fn log_entry_serialization() {
        let entry = LogEntry {
            id: "test-123-complete".into(),
            timestamp_epoch_ms: 1700000000000,
            duration_ms: 42,
            kind: "complete",
            request: serde_json::json!({"model": "gpt-4o"}),
            result: LogResult::Success {
                response: serde_json::json!({"content": "hello"}),
            },
        };

        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["id"], "test-123-complete");
        assert_eq!(json["kind"], "complete");
        assert_eq!(json["result"]["status"], "success");
        assert_eq!(json["result"]["response"]["content"], "hello");
        assert_eq!(json["duration_ms"], 42);
    }

    #[test]
    fn log_entry_error_serialization() {
        let entry = LogEntry {
            id: "test-456-complete".into(),
            timestamp_epoch_ms: 1700000000000,
            duration_ms: 10,
            kind: "complete",
            request: serde_json::json!({}),
            result: LogResult::Error {
                error: "connection refused".into(),
            },
        };

        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["result"]["status"], "error");
        assert_eq!(json["result"]["error"], "connection refused");
    }

    #[test]
    fn wrap_none_returns_inner() {
        let inner: Arc<dyn LlmService> = Arc::new(MockLlmService::new());
        let wrapped = LoggingLlmService::wrap(inner.clone(), None);
        assert!(Arc::ptr_eq(&inner, &wrapped));
    }

    #[test]
    fn wrap_some_returns_logging() {
        let inner: Arc<dyn LlmService> = Arc::new(MockLlmService::new());
        let wrapped = LoggingLlmService::wrap(inner.clone(), Some(PathBuf::from("/tmp/test-logs")));
        assert!(!Arc::ptr_eq(&inner, &wrapped));
    }

    #[test]
    fn assemble_stream_empty() {
        let assembled = assemble_stream_response(&[]);
        assert!(assembled.content.is_none());
        assert!(assembled.tool_call_deltas.is_empty());
        assert!(assembled.finish_reason.is_none());
        assert_eq!(assembled.chunk_count, 0);
    }

    #[test]
    fn assemble_stream_text_chunks() {
        let chunks = vec![
            StreamChunk {
                id: "c1".into(),
                delta: llm_provider::StreamDelta {
                    content: Some("Hello".into()),
                    tool_calls: None,
                },
                finish_reason: None,
            },
            StreamChunk {
                id: "c2".into(),
                delta: llm_provider::StreamDelta {
                    content: Some(" world".into()),
                    tool_calls: None,
                },
                finish_reason: Some(llm_provider::FinishReason::Stop),
            },
        ];

        let assembled = assemble_stream_response(&chunks);
        assert_eq!(assembled.content, Some("Hello world".into()));
        assert_eq!(assembled.chunk_count, 2);
        assert_eq!(assembled.finish_reason, Some("Stop".into()));
    }
}
