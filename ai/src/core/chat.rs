/// Conversational assistant logic.
use crate::api::error::AiResult;
use crate::api::types::{AiMessage, ChatRequest, ChatResponse, CompletionOptions};
use crate::core::history::ConversationHistory;
use crate::core::prompt;
use crate::spi::AiClient;

/// Process a chat message, maintaining conversation history.
pub async fn chat(
    client: &dyn AiClient,
    request: ChatRequest,
    history: &mut ConversationHistory,
) -> AiResult<ChatResponse> {
    // Ensure system prompt is at the start
    if history.is_empty() {
        history.push(AiMessage::system(prompt::chat_system_prompt()));
    }

    // Add user message to history
    history.push(AiMessage::user(&request.message));

    // Build messages from history
    let messages: Vec<AiMessage> = history
        .messages()
        .iter()
        .map(|m| AiMessage {
            role: m.role,
            content: m.content.clone(),
        })
        .collect();

    let options = CompletionOptions {
        temperature: Some(0.5),
        max_tokens: Some(1024),
    };

    let response = client.complete(messages, options).await?;
    let reply = response.content.trim().to_string();

    // Add assistant reply to history
    history.push(AiMessage::assistant(&reply));

    Ok(ChatResponse { reply })
}
