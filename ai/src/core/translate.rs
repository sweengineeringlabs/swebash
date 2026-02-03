/// NL -> shell command translation logic.
use crate::api::error::{AiError, AiResult};
use crate::api::types::{AiMessage, CompletionOptions, TranslateRequest, TranslateResponse};
use crate::core::prompt;
use crate::spi::AiClient;

/// Translate a natural language request into a shell command.
pub async fn translate(
    client: &dyn AiClient,
    request: TranslateRequest,
) -> AiResult<TranslateResponse> {
    let mut messages = vec![AiMessage::system(prompt::translate_system_prompt())];

    // Add context about the environment
    let context = format!(
        "Current directory: {}\nRecent commands: {}",
        request.cwd,
        if request.recent_commands.is_empty() {
            "(none)".to_string()
        } else {
            request.recent_commands.join(", ")
        }
    );
    messages.push(AiMessage::user(context));
    messages.push(AiMessage::assistant("Understood. I'll use this context for my translation."));

    // The actual request
    messages.push(AiMessage::user(&request.natural_language));

    let options = CompletionOptions {
        temperature: Some(0.1), // Low temperature for precise commands
        max_tokens: Some(256),
    };

    let response = client.complete(messages, options).await?;
    let command = response.content.trim().to_string();

    if command.is_empty() {
        return Err(AiError::ParseError(
            "LLM returned empty command".to_string(),
        ));
    }

    // The command is the direct output; explanation is minimal for the translate feature
    Ok(TranslateResponse {
        command: command.clone(),
        explanation: format!("Suggested command: {}", command),
    })
}
