/// Conversational assistant logic.
///
/// Delegates to the `SimpleChatEngine` from the `chat` crate, which
/// handles conversation memory, context windowing, and LLM interaction.
use crate::api::error::{AiError, AiResult};
use crate::api::types::{ChatRequest, ChatResponse};

use chat_engine::{ChatEngine, ChatMessage, SimpleChatEngine};

/// Process a chat message using the chat engine.
///
/// The engine manages conversation history internally, including
/// the system prompt, context window, and memory eviction.
pub async fn chat(
    engine: &SimpleChatEngine,
    request: ChatRequest,
) -> AiResult<ChatResponse> {
    let message = ChatMessage::user(&request.message);

    // Create a no-op event sender — swebash doesn't consume agent events.
    let (events, _stream) = react::event_stream(1);

    let response = engine
        .send(message, events)
        .await
        .map_err(|e| AiError::Provider(e.to_string()))?;

    Ok(ChatResponse {
        reply: response.message.content.trim().to_string(),
    })
}
