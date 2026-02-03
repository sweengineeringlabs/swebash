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

    // Task A: drive the engine — emits AgentEvents via `events`
    //
    // Some engines (e.g. ToolAwareChatEngine) delegate to a non-streaming
    // implementation that never emits AgentEvent::Content.  In that case
    // Task B will never forward anything.  As a fallback we send the
    // completed response here so the consumer always receives a Done event.
    tokio::spawn(async move {
        match engine.as_ref().send_streaming(message, events).await {
            Ok(response) => {
                let _ = tx_err
                    .send(ChatStreamEvent::Done(
                        response.message.content.trim().to_string(),
                    ))
                    .await;
            }
            Err(e) => {
                let _ = tx_err
                    .send(ChatStreamEvent::Done(format!("Error: {}", e)))
                    .await;
            }
        }
    });

    // Task B: forward AgentEvents → ChatStreamEvents
    tokio::spawn(async move {
        while let Some(event) = event_stream.next().await {
            match event {
                AgentEvent::Content { content, is_final } => {
                    if is_final {
                        let _ = tx
                            .send(ChatStreamEvent::Done(content.trim().to_string()))
                            .await;
                    } else {
                        let _ = tx.send(ChatStreamEvent::Delta(content)).await;
                    }
                }
                _ => {} // Ignore non-content events
            }
        }
    });

    Ok(rx)
}
