/// Conversational assistant logic.
///
/// Delegates to a `ChatEngine` implementation (SimpleChatEngine or ToolAwareChatEngine),
/// which handles conversation memory, context windowing, and LLM interaction.
use std::sync::Arc;

use futures::StreamExt;

use crate::api::error::{AiError, AiResult};
use crate::api::types::{AiEvent, ChatRequest, ChatResponse};

use chat_engine::{ChatEngine, ChatMessage};
use react::AgentEvent;

/// Check if tool call logging is enabled via `SWEBASH_AI_TOOL_LOG` env var.
fn tool_log_enabled() -> bool {
    std::env::var("SWEBASH_AI_TOOL_LOG")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Emit a structured tool call log to stderr.
///
/// Format: `SWEBASH_TOOL:{"tool":"name","params":{}}`
///
/// This enables autotest to parse tool calls without relying on stdout format.
/// Note: Tool parameters are not currently available from AgentEvent::ToolStart,
/// so we emit an empty params object. Future versions may include parameters
/// if the react crate exposes them.
fn log_tool_call(tool: &str) {
    if !tool_log_enabled() {
        return;
    }
    let log_entry = serde_json::json!({
        "tool": tool,
        "params": {}
    });
    eprintln!("SWEBASH_TOOL:{}", log_entry);
}

/// Process a chat message using the chat engine.
///
/// The engine manages conversation history internally, including
/// the system prompt, context window, and memory eviction.
pub async fn chat(
    engine: &dyn ChatEngine,
    request: ChatRequest,
) -> AiResult<ChatResponse> {
    let message = ChatMessage::user(&request.message);

    // Create a no-op event sender — swebash doesn't consume agent events.
    // Buffer must be large enough for all events the engine emits (status,
    // content, complete) so sends never block on an unconsumed receiver.
    let (events, _stream) = react::event_stream(64);

    let response = engine
        .send(message, events)
        .await
        .map_err(|e| AiError::Provider(e.to_string()))?;

    Ok(ChatResponse {
        reply: response.message.content.trim().to_string(),
    })
}

/// Process a chat message with token-by-token streaming.
///
/// Spawns the engine call in a background task and returns a receiver
/// that yields `AiEvent::Delta` for each token chunk, `AiEvent::ToolCall`
/// when the agent invokes a tool, and finally `AiEvent::Done` with the
/// full assembled reply (or `AiEvent::Error` on failure).
pub async fn chat_streaming(
    engine: &Arc<dyn ChatEngine>,
    request: ChatRequest,
) -> AiResult<tokio::sync::mpsc::Receiver<AiEvent>> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let message = ChatMessage::user(&request.message);
    let (events, mut event_stream) = react::event_stream(64);

    let engine = engine.clone();
    let tx_err = tx.clone();

    // Task B: forward real-time content deltas and tool-call notifications.
    //
    // Content { is_final: false } → AiEvent::Delta  (streaming token)
    // ToolStart { tool, .. }      → AiEvent::ToolCall (agent invoked a tool)
    // Content { is_final: true }  → ignored (full content duplicates the deltas;
    //                               Task A is the sole source of AiEvent::Done)
    let task_b = tokio::spawn(async move {
        while let Some(event) = event_stream.next().await {
            match event {
                AgentEvent::Content { content, is_final: false } => {
                    if tx.send(AiEvent::Delta(content)).await.is_err() {
                        tracing::warn!("stream receiver dropped, stopping delta forwarding");
                        break;
                    }
                }
                AgentEvent::ToolStart { tool, .. } => {
                    // Log tool call for autotest when SWEBASH_AI_TOOL_LOG=1
                    log_tool_call(&tool);

                    if tx.send(AiEvent::ToolCall { name: tool }).await.is_err() {
                        break;
                    }
                }
                _ => {}
            }
        }
    });

    // Task A: drive the engine, then send Done (or Error).
    //
    // After `send_streaming` returns, the event sender is dropped,
    // which closes the event stream and lets Task B finish.  We wait
    // for Task B to drain any remaining buffered events, then send
    // Done with the authoritative response content.
    tokio::spawn(async move {
        match engine.as_ref().send_streaming(message, events).await {
            Ok(response) => {
                if let Err(e) = task_b.await {
                    tracing::error!("delta forwarding task panicked: {e}");
                }
                if tx_err
                    .send(AiEvent::Done(
                        response.message.content.trim().to_string(),
                    ))
                    .await
                    .is_err()
                {
                    tracing::warn!("stream receiver dropped before Done event");
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                if tx_err
                    .send(AiEvent::Error(err_msg.clone()))
                    .await
                    .is_err()
                {
                    tracing::error!("failed to send error to stream: {err_msg}");
                    eprintln!("[swebash] chat error (stream lost): {err_msg}");
                }
            }
        }
    });

    Ok(rx)
}
