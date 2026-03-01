/// NL -> shell command translation logic.
use crate::api::error::{AiError, AiResult};
use crate::api::types::{TranslateRequest, TranslateResponse};
use crate::spi::GatewayClient;

/// Translate a natural language request into a shell command via gateway.
pub async fn translate_via_gateway(
    gateway: &GatewayClient,
    request: TranslateRequest,
) -> AiResult<TranslateResponse> {
    // Build the translation prompt
    let context = format!(
        "Current directory: {}\nRecent commands: {}\n\nTranslate this request into a shell command: {}",
        request.cwd,
        if request.recent_commands.is_empty() {
            "(none)".to_string()
        } else {
            request.recent_commands.join(", ")
        },
        request.natural_language
    );

    // Execute through the shell agent (or current agent)
    let response = gateway.execute("shell", &context).await?;
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
