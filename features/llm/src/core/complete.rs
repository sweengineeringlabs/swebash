/// Autocomplete suggestion logic.
use crate::api::error::AiResult;
use crate::api::types::{AutocompleteRequest, AutocompleteResponse};
use crate::spi::GatewayClient;

/// Generate autocomplete suggestions via gateway.
pub async fn autocomplete_via_gateway(
    gateway: &GatewayClient,
    request: AutocompleteRequest,
) -> AiResult<AutocompleteResponse> {
    let prompt = format!(
        "Suggest shell command completions for this context.\n\
         Current directory: {}\n\
         Files in directory: {}\n\
         Recent commands: {}\n\
         Partial input: {}\n\n\
         Provide up to 5 completion suggestions, one per line.",
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

    let response = gateway.execute("shell", &prompt).await?;

    let suggestions: Vec<String> = response
        .content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .take(5)
        .collect();

    Ok(AutocompleteResponse { suggestions })
}
