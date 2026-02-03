/// Command explanation logic.
use crate::api::error::AiResult;
use crate::api::types::{AiMessage, CompletionOptions, ExplainRequest, ExplainResponse};
use crate::core::prompt;
use crate::spi::AiClient;

/// Explain what a shell command does.
pub async fn explain(
    client: &dyn AiClient,
    request: ExplainRequest,
) -> AiResult<ExplainResponse> {
    let messages = vec![
        AiMessage::system(prompt::explain_system_prompt()),
        AiMessage::user(format!("Explain this command: {}", request.command)),
    ];

    let options = CompletionOptions {
        temperature: Some(0.3),
        max_tokens: Some(512),
    };

    let response = client.complete(messages, options).await?;

    Ok(ExplainResponse {
        explanation: response.content.trim().to_string(),
    })
}
