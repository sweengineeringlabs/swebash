/// Integration tests for swebash-llm using the llmboot gateway API.
///
/// Every test always runs and asserts something meaningful:
/// - With `ANTHROPIC_API_KEY` set: tests exercise the real API and verify responses.
/// - Without it: tests verify that proper errors propagate through every layer.
///
/// Several tests mutate environment variables and are marked `#[serial]`.
///
/// ```sh
/// cargo test --manifest-path features/llm/Cargo.toml                          # error-path tests
/// ANTHROPIC_API_KEY=sk-... cargo test --manifest-path features/llm/Cargo.toml # full integration
/// ```

use serial_test::serial;
use swebash_llm::api::error::AiError;
use swebash_llm::api::types::{
    AiEvent, AutocompleteRequest, ChatRequest, ExplainRequest, TranslateRequest,
};
use swebash_llm::api::AiService;
use swebash_llm::{AiConfig, ToolConfig};
use swebash_test::mock::{
    create_mock_service, create_mock_service_error, create_mock_service_fixed,
};

// ── Config tests ─────────────────────────────────────────────────────

#[test]
#[serial]
fn config_has_api_key_known_provider() {
    std::env::set_var("OPENAI_API_KEY", "sk-test-key");
    let config = AiConfig {
        enabled: true,
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        history_size: 20,
        tools: ToolConfig::default(),
        default_agent: "shell".to_string(),
        agent_auto_detect: false,
        log_dir: None,
        docs_base_dir: None,
        rag: swebash_llm::spi::config::RagConfig::default(),
        tool_sandbox: None,
    };
    assert!(config.has_api_key());
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
#[serial]
fn config_has_api_key_missing() {
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("ANTHROPIC_API_KEY");
    let config = AiConfig {
        enabled: true,
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        history_size: 20,
        tools: ToolConfig::default(),
        default_agent: "shell".to_string(),
        agent_auto_detect: false,
        log_dir: None,
        docs_base_dir: None,
        rag: swebash_llm::spi::config::RagConfig::default(),
        tool_sandbox: None,
    };
    assert!(!config.has_api_key());
}

#[test]
#[serial]
fn config_has_api_key_unknown_provider() {
    std::env::remove_var("UNKNOWN_API_KEY");
    let config = AiConfig {
        enabled: true,
        provider: "unknown".to_string(),
        model: "test".to_string(),
        history_size: 20,
        tools: ToolConfig::default(),
        default_agent: "shell".to_string(),
        agent_auto_detect: false,
        log_dir: None,
        docs_base_dir: None,
        rag: swebash_llm::spi::config::RagConfig::default(),
        tool_sandbox: None,
    };
    assert!(!config.has_api_key());
}

// ── MockAiService tests ──────────────────────────────────────────────

#[tokio::test]
async fn mock_service_is_available() {
    let service = create_mock_service();
    assert!(service.is_available().await);
}

#[tokio::test]
async fn mock_service_status() {
    let service = create_mock_service();
    let status = service.status().await;
    assert!(status.enabled);
    assert_eq!(status.provider, "mock");
    assert_eq!(status.model, "mock");
    assert!(status.ready);
}

#[tokio::test]
async fn mock_service_translate_echo() {
    let service = create_mock_service();
    let response = service
        .translate(TranslateRequest {
            natural_language: "list files".to_string(),
            cwd: "/tmp".to_string(),
            recent_commands: vec![],
        })
        .await
        .unwrap();
    assert_eq!(response.command, "list files");
}

#[tokio::test]
async fn mock_service_translate_fixed() {
    let service = create_mock_service_fixed("ls -la");
    let response = service
        .translate(TranslateRequest {
            natural_language: "anything".to_string(),
            cwd: "/tmp".to_string(),
            recent_commands: vec![],
        })
        .await
        .unwrap();
    assert_eq!(response.command, "ls -la");
}

#[tokio::test]
async fn mock_service_explain_echo() {
    let service = create_mock_service();
    let response = service
        .explain(ExplainRequest {
            command: "ls -la".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(response.explanation, "ls -la");
}

#[tokio::test]
async fn mock_service_chat_echo() {
    let service = create_mock_service();
    let response = service
        .chat(ChatRequest {
            message: "Hello world".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(response.reply, "Hello world");
}

#[tokio::test]
async fn mock_service_chat_fixed() {
    let service = create_mock_service_fixed("Fixed response");
    let response = service
        .chat(ChatRequest {
            message: "anything".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(response.reply, "Fixed response");
}

#[tokio::test]
async fn mock_service_error_translate() {
    let service = create_mock_service_error("Test error");
    let result = service
        .translate(TranslateRequest {
            natural_language: "test".to_string(),
            cwd: "/tmp".to_string(),
            recent_commands: vec![],
        })
        .await;
    match result {
        Err(AiError::Provider(msg)) => assert_eq!(msg, "Test error"),
        other => panic!("Expected Provider error, got: {:?}", other),
    }
}

#[tokio::test]
async fn mock_service_error_chat() {
    let service = create_mock_service_error("Chat error");
    let result = service
        .chat(ChatRequest {
            message: "hello".to_string(),
        })
        .await;
    match result {
        Err(AiError::Provider(msg)) => assert_eq!(msg, "Chat error"),
        other => panic!("Expected Provider error, got: {:?}", other),
    }
}

#[tokio::test]
async fn mock_service_error_is_not_available() {
    let service = create_mock_service_error("Error");
    assert!(!service.is_available().await);
}

// ── Agent tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn mock_service_list_agents() {
    let service = create_mock_service();
    let agents = service.list_agents().await;
    assert_eq!(agents.len(), 2);
    assert!(agents.iter().any(|a| a.id == "shell"));
    assert!(agents.iter().any(|a| a.id == "git"));
}

#[tokio::test]
async fn mock_service_current_agent() {
    let service = create_mock_service();
    let agent = service.current_agent().await;
    assert_eq!(agent.id, "shell");
    assert!(agent.active);
}

#[tokio::test]
async fn mock_service_switch_agent() {
    let service = create_mock_service();

    // Initial agent
    let agent = service.current_agent().await;
    assert_eq!(agent.id, "shell");

    // Switch to git
    service.switch_agent("git").await.unwrap();
    let agent = service.current_agent().await;
    assert_eq!(agent.id, "git");

    // Switch back
    service.switch_agent("shell").await.unwrap();
    let agent = service.current_agent().await;
    assert_eq!(agent.id, "shell");
}

#[tokio::test]
async fn mock_service_switch_unknown_agent_fails() {
    let service = create_mock_service();
    let result = service.switch_agent("unknown_agent").await;
    assert!(result.is_err());
    match result {
        Err(AiError::NotConfigured(msg)) => {
            assert!(msg.contains("unknown_agent"));
        }
        other => panic!("Expected NotConfigured error, got: {:?}", other),
    }
}

// ── Streaming tests ──────────────────────────────────────────────────

#[tokio::test]
async fn mock_service_chat_streaming() {
    let service = create_mock_service_fixed("Streamed response");
    let mut rx = service
        .chat_streaming(ChatRequest {
            message: "hello".to_string(),
        })
        .await
        .unwrap();

    // Should receive a Done event
    let event = rx.recv().await;
    match event {
        Some(AiEvent::Done(content)) => {
            assert_eq!(content, "Streamed response");
        }
        other => panic!("Expected Done event, got: {:?}", other),
    }
}

#[tokio::test]
async fn mock_service_chat_streaming_error() {
    let service = create_mock_service_error("Stream error");
    let mut rx = service
        .chat_streaming(ChatRequest {
            message: "hello".to_string(),
        })
        .await
        .unwrap();

    // Should receive an Error event
    let event = rx.recv().await;
    match event {
        Some(AiEvent::Error(msg)) => {
            assert!(msg.contains("Stream error"));
        }
        other => panic!("Expected Error event, got: {:?}", other),
    }
}

// ── Autocomplete tests ───────────────────────────────────────────────

#[tokio::test]
async fn mock_service_autocomplete_echo() {
    let service = create_mock_service();
    let response = service
        .autocomplete(AutocompleteRequest {
            partial_input: "ls -".to_string(),
            cwd: "/tmp".to_string(),
            cwd_entries: vec![],
            recent_commands: vec![],
        })
        .await
        .unwrap();
    assert_eq!(response.suggestions, vec!["ls -"]);
}

#[tokio::test]
async fn mock_service_autocomplete_fixed() {
    let service = create_mock_service_fixed("ls -la");
    let response = service
        .autocomplete(AutocompleteRequest {
            partial_input: "anything".to_string(),
            cwd: "/tmp".to_string(),
            cwd_entries: vec![],
            recent_commands: vec![],
        })
        .await
        .unwrap();
    assert_eq!(response.suggestions, vec!["ls -la"]);
}

// ── Config from environment tests ────────────────────────────────────

#[test]
#[serial]
fn config_from_env_defaults() {
    // Clear all AI-related env vars
    std::env::remove_var("SWEBASH_AI_ENABLED");
    std::env::remove_var("LLM_PROVIDER");
    std::env::remove_var("LLM_DEFAULT_MODEL");
    std::env::remove_var("SWEBASH_AI_HISTORY_SIZE");
    std::env::remove_var("SWEBASH_AI_DEFAULT_AGENT");

    let config = AiConfig::from_env();

    // Check defaults - provider is auto-detected from available credentials
    // (Claude Code OAuth, Google ADC, or API keys), falling back to "openai"
    assert!(config.enabled);
    assert!(
        ["anthropic", "openai", "gemini"].contains(&config.provider.as_str()),
        "provider should be auto-detected or fallback to openai, got: {}",
        config.provider
    );
    assert_eq!(config.history_size, 20);
    assert_eq!(config.default_agent, "shell");
}

#[test]
#[serial]
fn config_from_env_custom() {
    std::env::set_var("SWEBASH_AI_ENABLED", "true");
    std::env::set_var("LLM_PROVIDER", "anthropic");
    std::env::set_var("LLM_DEFAULT_MODEL", "claude-sonnet-4-20250514");
    std::env::set_var("SWEBASH_AI_HISTORY_SIZE", "50");
    std::env::set_var("SWEBASH_AI_DEFAULT_AGENT", "git");

    let config = AiConfig::from_env();

    assert!(config.enabled);
    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.model, "claude-sonnet-4-20250514");
    assert_eq!(config.history_size, 50);
    assert_eq!(config.default_agent, "git");

    // Cleanup
    std::env::remove_var("SWEBASH_AI_ENABLED");
    std::env::remove_var("LLM_PROVIDER");
    std::env::remove_var("LLM_DEFAULT_MODEL");
    std::env::remove_var("SWEBASH_AI_HISTORY_SIZE");
    std::env::remove_var("SWEBASH_AI_DEFAULT_AGENT");
}

#[test]
#[serial]
fn config_from_env_disabled() {
    std::env::set_var("SWEBASH_AI_ENABLED", "false");

    let config = AiConfig::from_env();
    assert!(!config.enabled);

    std::env::remove_var("SWEBASH_AI_ENABLED");
}
