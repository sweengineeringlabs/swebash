/// AiService contract verifiers.
///
/// Each `verify_*` function takes an `&dyn AiService` and asserts one
/// documented API contract. Use `verify_all_contracts` to run the full
/// suite, or call individual verifiers in targeted tests.

use swebash_ai::api::error::AiError;
use swebash_ai::api::types::TranslateRequest;
use swebash_ai::api::AiService;

use crate::error::TestError;

/// Verify that `is_available()` returns `true`.
pub async fn verify_available(service: &dyn AiService) -> Result<(), TestError> {
    let available = service.is_available().await;
    if !available {
        return Err(TestError::Contract(
            "is_available() returned false".into(),
        ));
    }
    Ok(())
}

/// Verify that `status()` returns enabled=true with non-empty provider and model.
pub async fn verify_status(service: &dyn AiService) -> Result<(), TestError> {
    let status = service.status().await;
    if !status.enabled {
        return Err(TestError::Contract("status.enabled is false".into()));
    }
    if status.provider.is_empty() {
        return Err(TestError::Contract("status.provider is empty".into()));
    }
    if status.model.is_empty() {
        return Err(TestError::Contract("status.model is empty".into()));
    }
    Ok(())
}

/// Verify that `list_agents()` returns at least one agent.
pub async fn verify_list_agents_non_empty(service: &dyn AiService) -> Result<(), TestError> {
    let agents = service.list_agents().await;
    if agents.is_empty() {
        return Err(TestError::Contract(
            "list_agents() returned empty list".into(),
        ));
    }
    Ok(())
}

/// Verify that `current_agent()` returns an agent with non-empty id and active=true.
pub async fn verify_current_agent(service: &dyn AiService) -> Result<(), TestError> {
    let agent = service.current_agent().await;
    if agent.id.is_empty() {
        return Err(TestError::Contract("current_agent().id is empty".into()));
    }
    if !agent.active {
        return Err(TestError::Contract(
            "current_agent().active is false".into(),
        ));
    }
    Ok(())
}

/// Verify that switching to a non-existent agent returns `NotConfigured`.
pub async fn verify_switch_unknown_agent_fails(service: &dyn AiService) -> Result<(), TestError> {
    let result = service
        .switch_agent("__nonexistent_agent_id_that_should_never_exist__")
        .await;
    match result {
        Err(AiError::NotConfigured(_)) => Ok(()),
        Err(other) => Err(TestError::Contract(format!(
            "switch_agent with unknown id returned unexpected error: {other:?}"
        ))),
        Ok(()) => Err(TestError::Contract(
            "switch_agent with unknown id should fail but returned Ok".into(),
        )),
    }
}

/// Verify that `translate()` returns a non-empty command.
pub async fn verify_translate_returns_command(service: &dyn AiService) -> Result<(), TestError> {
    let request = TranslateRequest {
        natural_language: "list files".into(),
        cwd: "/tmp".into(),
        recent_commands: vec![],
    };
    let response = service.translate(request).await.map_err(|e| {
        TestError::Contract(format!("translate() returned error: {e:?}"))
    })?;
    if response.command.is_empty() {
        return Err(TestError::Contract(
            "translate() returned empty command".into(),
        ));
    }
    Ok(())
}

/// Run all contract verifiers against the given service.
///
/// Returns the first failure encountered, or `Ok(())` if all pass.
pub async fn verify_all_contracts(service: &dyn AiService) -> Result<(), TestError> {
    verify_available(service).await?;
    verify_status(service).await?;
    verify_list_agents_non_empty(service).await?;
    verify_current_agent(service).await?;
    verify_switch_unknown_agent_fails(service).await?;
    verify_translate_returns_command(service).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::create_mock_service;

    #[tokio::test]
    async fn contract_verify_available() {
        let service = create_mock_service();
        verify_available(&service).await.unwrap();
    }

    #[tokio::test]
    async fn contract_verify_status() {
        let service = create_mock_service();
        verify_status(&service).await.unwrap();
    }

    #[tokio::test]
    async fn contract_verify_list_agents_non_empty() {
        let service = create_mock_service();
        verify_list_agents_non_empty(&service).await.unwrap();
    }

    #[tokio::test]
    async fn contract_verify_current_agent() {
        let service = create_mock_service();
        verify_current_agent(&service).await.unwrap();
    }

    #[tokio::test]
    async fn contract_verify_switch_unknown_agent_fails() {
        let service = create_mock_service();
        verify_switch_unknown_agent_fails(&service).await.unwrap();
    }

    #[tokio::test]
    async fn contract_verify_translate_returns_command() {
        let service = create_mock_service();
        verify_translate_returns_command(&service).await.unwrap();
    }

    #[tokio::test]
    async fn contract_verify_all() {
        let service = create_mock_service();
        verify_all_contracts(&service).await.unwrap();
    }
}
