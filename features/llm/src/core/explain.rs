/// Command explanation logic.
use crate::api::error::AiResult;
use crate::api::types::{ExplainRequest, ExplainResponse};
use crate::spi::GatewayClient;

/// Explain what a shell command does via gateway.
pub async fn explain_via_gateway(
    gateway: &GatewayClient,
    request: ExplainRequest,
) -> AiResult<ExplainResponse> {
    let prompt = format!(
        "Explain what this shell command does in simple terms: {}",
        request.command
    );

    let response = gateway.execute("shell", &prompt).await?;

    Ok(ExplainResponse {
        explanation: response.content.trim().to_string(),
    })
}
