/// Conversational assistant logic.
///
/// Delegates to a `ChatEngine` implementation (SimpleChatEngine or ToolAwareChatEngine),
/// which handles conversation memory, context windowing, and LLM interaction.
use std::sync::Arc;

use futures::StreamExt;

use crate::api::error::{AiError, AiResult};
use crate::api::types::{ChatRequest, ChatResponse, ChatStreamEvent};

use chat_engine::{ChatEngine, ChatMessage};
use react::AgentEvent;

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
/// that yields `ChatStreamEvent::Delta` for each token chunk, followed
/// by `ChatStreamEvent::Done` with the full assembled reply.
pub async fn chat_streaming(
    engine: &Arc<dyn ChatEngine>,
    request: ChatRequest,
) -> AiResult<tokio::sync::mpsc::Receiver<ChatStreamEvent>> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let message = ChatMessage::user(&request.message);
    let (events, mut event_stream) = react::event_stream(64);

    let engine = engine.clone();
    let tx_err = tx.clone();

    // Task B: forward real-time content deltas.
    //
    // Only non-final events are forwarded as Delta.  The final
    // `is_final: true` event carries the full accumulated content
    // (duplicating the deltas), so it is intentionally ignored here.
    // Task A is the sole source of the Done event.
    let task_b = tokio::spawn(async move {
        while let Some(event) = event_stream.next().await {
            if let AgentEvent::Content { content, is_final: false } = event {
                if tx.send(ChatStreamEvent::Delta(content)).await.is_err() {
                    tracing::warn!("stream receiver dropped, stopping delta forwarding");
                    break;
                }
            }
        }
    });

    // Task A: drive the engine, then send Done.
    //
    // After `send_streaming` returns, the event sender is dropped,
    // which closes the event stream and lets Task B finish.  We wait
    // for Task B to drain any remaining buffered deltas, then send
    // Done with the authoritative response content.
    tokio::spawn(async move {
        match engine.as_ref().send_streaming(message, events).await {
            Ok(response) => {
                if let Err(e) = task_b.await {
                    tracing::error!("delta forwarding task panicked: {e}");
                }
                if tx_err
                    .send(ChatStreamEvent::Done(
                        response.message.content.trim().to_string(),
                    ))
                    .await
                    .is_err()
                {
                    tracing::warn!("stream receiver dropped before Done event");
                }
            }
            Err(e) => {
                let err_msg = format!("Error: {e}");
                if tx_err
                    .send(ChatStreamEvent::Done(err_msg.clone()))
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
