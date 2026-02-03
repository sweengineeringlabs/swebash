/// Autocomplete suggestion logic.
use crate::api::error::AiResult;
use crate::api::types::{
    AiMessage, AutocompleteRequest, AutocompleteResponse, CompletionOptions,
};
use crate::core::prompt;
use crate::spi::AiClient;

/// Generate autocomplete suggestions based on context.
pub async fn autocomplete(
    client: &dyn AiClient,
    request: AutocompleteRequest,
) -> AiResult<AutocompleteResponse> {
    let context = format!(
        "Current directory: {}\nFiles in directory: {}\nRecent commands: {}\nPartial input: {}",
        request.cwd,
        if request.cwd_entries.is_empty() {
            "(empty)".to_string()
        } else {
            request.cwd_entries.join(", ")
        },
        if request.recent_commands.is_empty() {
            "(none)".to_string()
        } else {
            request.recent_commands.join(", ")
        },
        if request.partial_input.is_empty() {
            "(none)".to_string()
        } else {
            request.partial_input.clone()
        }
    );

    let messages = vec![
        AiMessage::system(prompt::autocomplete_system_prompt()),
        AiMessage::user(context),
    ];

    let options = CompletionOptions {
        temperature: Some(0.3),
        max_tokens: Some(256),
    };

    let response = client.complete(messages, options).await?;

    let suggestions: Vec<String> = response
        .content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .take(5)
        .collect();

    Ok(AutocompleteResponse { suggestions })
}
