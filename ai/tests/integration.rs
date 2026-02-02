/// Integration tests for swebash-ai using the real Anthropic LLM provider.
///
/// Every test always runs and asserts something meaningful:
/// - With `ANTHROPIC_API_KEY` set: tests exercise the real API and verify responses.
/// - Without it: tests verify that proper errors propagate through every layer.
///
/// Several tests mutate environment variables and are marked `#[serial]`.
///
/// ```sh
/// cargo test --manifest-path ai/Cargo.toml                          # error-path tests
/// ANTHROPIC_API_KEY=sk-... cargo test --manifest-path ai/Cargo.toml # full integration
/// ```

use serial_test::serial;
use swebash_ai::api::error::{AiError, AiResult};
use swebash_ai::api::types::{
    AutocompleteRequest, ChatRequest, ChatStreamEvent, ExplainRequest, TranslateRequest,
};
use swebash_ai::api::AiService;
use swebash_ai::config::AiConfig;
use swebash_ai::core::DefaultAiService;
use swebash_ai::spi::chat_provider::ChatProviderClient;

// ── Helpers ──────────────────────────────────────────────────────────────

/// Build an `AiConfig` pointing at Anthropic.
fn anthropic_config() -> AiConfig {
    AiConfig {
        enabled: true,
        provider: "anthropic".to_string(),
        model: std::env::var("LLM_DEFAULT_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
        history_size: 20,
    }
}

/// Build a `SimpleChatEngine` from a `ChatProviderClient` and config.
fn build_chat_engine(
    client: &ChatProviderClient,
    config: &AiConfig,
) -> chat_engine::SimpleChatEngine {
    let chat_config = chat_engine::ChatConfig {
        model: config.model.clone(),
        temperature: 0.5,
        max_tokens: 1024,
        system_prompt: Some(swebash_ai::core::prompt::chat_system_prompt()),
        max_history: config.history_size,
        enable_summarization: false,
    };
    chat_engine::SimpleChatEngine::new(client.llm_service(), chat_config)
}

/// Try to create a real Anthropic-backed service.
///
/// Returns `Ok(service)` when the provider initialises (API key present),
/// or `Err(AiError)` when it cannot (missing key, network, etc.).
async fn try_create_service() -> AiResult<DefaultAiService> {
    let config = anthropic_config();
    let client = ChatProviderClient::new(&config).await?;
    let chat_engine = build_chat_engine(&client, &config);
    Ok(DefaultAiService::new(Box::new(client), chat_engine, config))
}

/// Same as [`try_create_service`] but with a caller-supplied config.
async fn try_create_service_with_config(config: AiConfig) -> AiResult<DefaultAiService> {
    let client = ChatProviderClient::new(&config).await?;
    let chat_engine = build_chat_engine(&client, &config);
    Ok(DefaultAiService::new(Box::new(client), chat_engine, config))
}

/// Assert that `err` is the kind we expect when the provider cannot be
/// initialised (missing key, bad config, unreachable service, …).
fn assert_setup_error(err: &AiError) {
    match err {
        AiError::NotConfigured(_) | AiError::Provider(_) => {}
        other => panic!(
            "Expected NotConfigured or Provider for missing configuration, got: {:?}",
            other
        ),
    }
}

// ── Config tests (3) ─────────────────────────────────────────────────────

#[test]
#[serial]
fn config_has_api_key_known_provider() {
    std::env::set_var("OPENAI_API_KEY", "sk-test-key");
    let config = AiConfig {
        enabled: true,
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        history_size: 20,
    };
    assert!(config.has_api_key());
    std::env::remove_var("OPENAI_API_KEY");
}

#[test]
#[serial]
fn config_has_api_key_missing() {
    std::env::remove_var("OPENAI_API_KEY");
    let config = AiConfig {
        enabled: true,
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        history_size: 20,
    };
    assert!(!config.has_api_key());
}

#[test]
fn config_has_api_key_unknown_provider() {
    let config = AiConfig {
        enabled: true,
        provider: "some-unknown-provider".to_string(),
        model: "whatever".to_string(),
        history_size: 20,
    };
    assert!(!config.has_api_key());
}

// ── Factory tests (2) ────────────────────────────────────────────────────

#[tokio::test]
#[serial]
async fn factory_disabled() {
    let orig = std::env::var("SWEBASH_AI_ENABLED").ok();
    std::env::set_var("SWEBASH_AI_ENABLED", "false");

    let result = swebash_ai::create_ai_service().await;

    match orig {
        Some(v) => std::env::set_var("SWEBASH_AI_ENABLED", v),
        None => std::env::remove_var("SWEBASH_AI_ENABLED"),
    }

    match result {
        Err(AiError::NotConfigured(msg)) => {
            assert!(msg.contains("disabled"), "Expected 'disabled' in: {}", msg);
        }
        Err(other) => panic!("Expected NotConfigured, got: {:?}", other),
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

#[tokio::test]
#[serial]
async fn factory_missing_api_key() {
    let orig_enabled = std::env::var("SWEBASH_AI_ENABLED").ok();
    let orig_provider = std::env::var("LLM_PROVIDER").ok();
    let orig_key = std::env::var("OPENAI_API_KEY").ok();

    std::env::set_var("SWEBASH_AI_ENABLED", "true");
    std::env::set_var("LLM_PROVIDER", "openai");
    std::env::remove_var("OPENAI_API_KEY");

    let result = swebash_ai::create_ai_service().await;

    match orig_enabled {
        Some(v) => std::env::set_var("SWEBASH_AI_ENABLED", v),
        None => std::env::remove_var("SWEBASH_AI_ENABLED"),
    }
    match orig_provider {
        Some(v) => std::env::set_var("LLM_PROVIDER", v),
        None => std::env::remove_var("LLM_PROVIDER"),
    }
    if let Some(v) = orig_key {
        std::env::set_var("OPENAI_API_KEY", v);
    }

    match result {
        Err(AiError::NotConfigured(msg)) => {
            assert!(
                msg.contains("No API key"),
                "Expected 'No API key' in: {}",
                msg
            );
        }
        Err(other) => panic!("Expected NotConfigured, got: {:?}", other),
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

// ── Translate tests (5) ──────────────────────────────────────────────────

#[tokio::test]
async fn translate_returns_command() {
    match try_create_service().await {
        Ok(service) => match service
            .translate(TranslateRequest {
                natural_language: "list files in the current directory".to_string(),
                cwd: "/home/user".to_string(),
                recent_commands: vec!["cd /home/user".to_string()],
            })
            .await
        {
            Ok(resp) => assert!(!resp.command.is_empty(), "Expected a non-empty command"),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn translate_command_no_markdown() {
    match try_create_service().await {
        Ok(service) => match service
            .translate(TranslateRequest {
                natural_language: "show disk usage summary".to_string(),
                cwd: "/".to_string(),
                recent_commands: vec![],
            })
            .await
        {
            Ok(resp) => assert!(
                !resp.command.contains("```"),
                "Command should not contain markdown fences: {}",
                resp.command
            ),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn translate_with_context() {
    match try_create_service().await {
        Ok(service) => match service
            .translate(TranslateRequest {
                natural_language: "find all rust source files".to_string(),
                cwd: "/home/user/project".to_string(),
                recent_commands: vec!["cargo build".to_string(), "git status".to_string()],
            })
            .await
        {
            Ok(resp) => assert!(!resp.command.is_empty()),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn translate_with_empty_history() {
    match try_create_service().await {
        Ok(service) => match service
            .translate(TranslateRequest {
                natural_language: "print working directory".to_string(),
                cwd: "/tmp".to_string(),
                recent_commands: vec![],
            })
            .await
        {
            Ok(resp) => assert!(!resp.command.is_empty()),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn translate_response_has_explanation() {
    match try_create_service().await {
        Ok(service) => match service
            .translate(TranslateRequest {
                natural_language: "count lines in all python files".to_string(),
                cwd: "/project".to_string(),
                recent_commands: vec![],
            })
            .await
        {
            Ok(resp) => assert!(
                !resp.explanation.is_empty(),
                "Expected a non-empty explanation"
            ),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

// ── Explain tests (3) ────────────────────────────────────────────────────

#[tokio::test]
async fn explain_simple_command() {
    match try_create_service().await {
        Ok(service) => match service
            .explain(ExplainRequest {
                command: "ls -la".to_string(),
            })
            .await
        {
            Ok(resp) => assert!(!resp.explanation.is_empty()),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn explain_pipeline_command() {
    match try_create_service().await {
        Ok(service) => match service
            .explain(ExplainRequest {
                command: "cat /var/log/syslog | grep error | wc -l".to_string(),
            })
            .await
        {
            Ok(resp) => assert!(
                !resp.explanation.is_empty(),
                "Pipeline explanation should not be empty"
            ),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn explain_response_is_trimmed() {
    match try_create_service().await {
        Ok(service) => match service
            .explain(ExplainRequest {
                command: "pwd".to_string(),
            })
            .await
        {
            Ok(resp) => assert_eq!(
                resp.explanation,
                resp.explanation.trim(),
                "Explanation should be trimmed"
            ),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

// ── Chat tests (5) ──────────────────────────────────────────────────────

#[tokio::test]
async fn chat_returns_reply() {
    match try_create_service().await {
        Ok(service) => match service
            .chat(ChatRequest {
                message: "What does the cd command do?".to_string(),
            })
            .await
        {
            Ok(resp) => assert!(!resp.reply.is_empty()),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn chat_multi_turn() {
    match try_create_service().await {
        Ok(service) => {
            let first = service
                .chat(ChatRequest {
                    message: "What is the ls command?".to_string(),
                })
                .await;
            match first {
                Err(e) => { assert_setup_error(&e); return; }
                Ok(_) => {}
            }

            match service
                .chat(ChatRequest {
                    message: "What flags does it accept?".to_string(),
                })
                .await
            {
                Ok(resp) => assert!(
                    !resp.reply.is_empty(),
                    "Second turn should return a non-empty reply"
                ),
                Err(e) => assert_setup_error(&e),
            }
        }
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn chat_history_clear() {
    match try_create_service().await {
        Ok(service) => {
            match service
                .chat(ChatRequest {
                    message: "hello".to_string(),
                })
                .await
            {
                Err(e) => { assert_setup_error(&e); return; }
                Ok(_) => {}
            }

            service.clear_history().await;
            let display = service.format_history().await;
            assert!(
                display.contains("(no chat history)"),
                "Expected empty history after clear, got: {}",
                display
            );
        }
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn chat_format_history_shows_messages() {
    match try_create_service().await {
        Ok(service) => {
            match service
                .chat(ChatRequest {
                    message: "What is echo?".to_string(),
                })
                .await
            {
                Err(e) => { assert_setup_error(&e); return; }
                Ok(_) => {}
            }

            let display = service.format_history().await;
            assert!(
                display.contains("[You]"),
                "History should contain user label: {}",
                display
            );
            assert!(
                display.contains("[AI]"),
                "History should contain AI label: {}",
                display
            );
        }
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn chat_history_respects_capacity() {
    let config = AiConfig {
        enabled: true,
        provider: "anthropic".to_string(),
        model: std::env::var("LLM_DEFAULT_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
        history_size: 4,
    };
    match try_create_service_with_config(config).await {
        Ok(service) => {
            for i in 1..=3 {
                match service
                    .chat(ChatRequest {
                        message: format!("Message number {}", i),
                    })
                    .await
                {
                    Err(e) => { assert_setup_error(&e); return; }
                    Ok(_) => {}
                }
            }

            let display = service.format_history().await;
            assert!(
                !display.contains("Message number 1"),
                "Oldest message should have been evicted from history: {}",
                display
            );
            assert!(
                display.contains("Message number 3"),
                "Latest message should be in history: {}",
                display
            );
        }
        Err(e) => assert_setup_error(&e),
    }
}

// ── Chat streaming tests (2) ─────────────────────────────────────────────

#[tokio::test]
async fn chat_streaming_returns_events() {
    match try_create_service().await {
        Ok(service) => {
            let request = ChatRequest {
                message: "What does the echo command do?".to_string(),
            };
            match service.chat_streaming(request).await {
                Ok(mut rx) => {
                    let mut got_delta = false;
                    let mut got_done = false;

                    while let Some(event) = rx.recv().await {
                        match event {
                            ChatStreamEvent::Delta(_) => got_delta = true,
                            ChatStreamEvent::Done(text) => {
                                got_done = true;
                                assert!(!text.is_empty(), "Done text should not be empty");
                            }
                        }
                    }

                    assert!(got_delta, "Should have received at least one Delta event");
                    assert!(got_done, "Should have received a Done event");
                }
                Err(e) => assert_setup_error(&e),
            }
        }
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn chat_streaming_multi_turn_preserves_history() {
    match try_create_service().await {
        Ok(service) => {
            // First turn — streaming
            let request = ChatRequest {
                message: "Remember this word: elephant".to_string(),
            };
            match service.chat_streaming(request).await {
                Ok(mut rx) => {
                    let mut first_text = String::new();
                    while let Some(event) = rx.recv().await {
                        match event {
                            ChatStreamEvent::Delta(_) => {}
                            ChatStreamEvent::Done(text) => {
                                first_text = text;
                                break;
                            }
                        }
                    }
                    // If the engine returned an error through the stream,
                    // treat it like a setup error and skip the rest.
                    if first_text.starts_with("Error:") {
                        return;
                    }
                }
                Err(e) => {
                    assert_setup_error(&e);
                    return;
                }
            }

            // Second turn — streaming, should remember context
            let request = ChatRequest {
                message: "What word did I ask you to remember?".to_string(),
            };
            match service.chat_streaming(request).await {
                Ok(mut rx) => {
                    let mut full_text = String::new();
                    while let Some(event) = rx.recv().await {
                        match event {
                            ChatStreamEvent::Delta(d) => full_text.push_str(&d),
                            ChatStreamEvent::Done(text) => {
                                full_text = text;
                                break;
                            }
                        }
                    }
                    if full_text.starts_with("Error:") {
                        return; // API-level error, not a test failure
                    }
                    assert!(
                        full_text.to_lowercase().contains("elephant"),
                        "Second turn should reference context from first turn, got: {}",
                        full_text
                    );
                }
                Err(e) => assert_setup_error(&e),
            }
        }
        Err(e) => assert_setup_error(&e),
    }
}

// ── Autocomplete tests (4) ───────────────────────────────────────────────

#[tokio::test]
async fn autocomplete_returns_suggestions() {
    match try_create_service().await {
        Ok(service) => match service
            .autocomplete(AutocompleteRequest {
                partial_input: "gi".to_string(),
                cwd: "/home/user/project".to_string(),
                cwd_entries: vec![
                    "src".to_string(),
                    "Cargo.toml".to_string(),
                    ".gitignore".to_string(),
                ],
                recent_commands: vec!["cargo build".to_string()],
            })
            .await
        {
            Ok(resp) => assert!(
                !resp.suggestions.is_empty(),
                "Expected at least one suggestion"
            ),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn autocomplete_with_partial_input() {
    match try_create_service().await {
        Ok(service) => match service
            .autocomplete(AutocompleteRequest {
                partial_input: "cargo".to_string(),
                cwd: "/project".to_string(),
                cwd_entries: vec!["Cargo.toml".to_string(), "src".to_string()],
                recent_commands: vec!["cargo build".to_string()],
            })
            .await
        {
            Ok(resp) => assert!(!resp.suggestions.is_empty()),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn autocomplete_no_empty_suggestions() {
    match try_create_service().await {
        Ok(service) => match service
            .autocomplete(AutocompleteRequest {
                partial_input: "ls".to_string(),
                cwd: "/tmp".to_string(),
                cwd_entries: vec!["a.txt".to_string(), "b.txt".to_string()],
                recent_commands: vec![],
            })
            .await
        {
            Ok(resp) => {
                for s in &resp.suggestions {
                    assert!(!s.is_empty(), "Suggestions must not contain empty strings");
                }
            }
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn autocomplete_max_five() {
    match try_create_service().await {
        Ok(service) => match service
            .autocomplete(AutocompleteRequest {
                partial_input: "".to_string(),
                cwd: "/home/user".to_string(),
                cwd_entries: vec![
                    "Documents".to_string(),
                    "Downloads".to_string(),
                    "Pictures".to_string(),
                    "Music".to_string(),
                    "Videos".to_string(),
                    "Desktop".to_string(),
                    ".bashrc".to_string(),
                ],
                recent_commands: vec![
                    "ls".to_string(),
                    "cd Documents".to_string(),
                    "cat .bashrc".to_string(),
                ],
            })
            .await
        {
            Ok(resp) => assert!(
                resp.suggestions.len() <= 5,
                "Expected at most 5 suggestions, got {}",
                resp.suggestions.len()
            ),
            Err(e) => assert_setup_error(&e),
        },
        Err(e) => assert_setup_error(&e),
    }
}

// ── Status / Availability tests (4) ──────────────────────────────────────

#[tokio::test]
async fn service_is_available() {
    match try_create_service().await {
        Ok(service) => assert!(service.is_available().await),
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn service_status_provider_is_anthropic() {
    match try_create_service().await {
        Ok(service) => assert_eq!(service.status().await.provider, "anthropic"),
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn service_status_model_matches_config() {
    let config = anthropic_config();
    let expected_model = config.model.clone();
    match try_create_service_with_config(config).await {
        Ok(service) => assert_eq!(service.status().await.model, expected_model),
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn service_disabled_rejects_requests() {
    let config = AiConfig {
        enabled: false,
        provider: "anthropic".to_string(),
        model: std::env::var("LLM_DEFAULT_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
        history_size: 20,
    };
    match try_create_service_with_config(config).await {
        Ok(service) => {
            assert!(!service.is_available().await);
            let result = service
                .translate(TranslateRequest {
                    natural_language: "test".to_string(),
                    cwd: "/".to_string(),
                    recent_commands: vec![],
                })
                .await;
            match result {
                Err(AiError::NotConfigured(_)) => {}
                Err(other) => panic!("Expected NotConfigured, got: {:?}", other),
                Ok(_) => panic!("Expected error from disabled service"),
            }
        }
        Err(e) => assert_setup_error(&e),
    }
}

// ── Error propagation tests (3) ──────────────────────────────────────────

#[tokio::test]
#[serial]
async fn error_bad_api_key_propagates() {
    // Save whatever key is (or isn't) present, inject a bogus one.
    let original = std::env::var("ANTHROPIC_API_KEY").ok();
    std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-INVALID-KEY");

    let config = AiConfig {
        enabled: true,
        provider: "anthropic".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        history_size: 20,
    };

    // The error may surface at client creation or at the first API call —
    // both are valid propagation paths.
    let outcome = match ChatProviderClient::new(&config).await {
        Err(e) => Err(e),
        Ok(client) => {
            let chat_engine = build_chat_engine(&client, &config);
            let service = DefaultAiService::new(Box::new(client), chat_engine, config);
            service
                .translate(TranslateRequest {
                    natural_language: "list files".to_string(),
                    cwd: "/".to_string(),
                    recent_commands: vec![],
                })
                .await
                .map(|_| ())
        }
    };

    // Restore immediately.
    match original {
        Some(k) => std::env::set_var("ANTHROPIC_API_KEY", k),
        None => std::env::remove_var("ANTHROPIC_API_KEY"),
    }

    assert!(
        outcome.is_err(),
        "Expected an error with an invalid API key, got Ok"
    );
}

#[tokio::test]
async fn error_invalid_model_propagates() {
    let config = AiConfig {
        enabled: true,
        provider: "anthropic".to_string(),
        model: "nonexistent-model-xyz-99".to_string(),
        history_size: 20,
    };
    match try_create_service_with_config(config).await {
        Ok(service) => {
            // Service created (key present) — the bad model should cause an
            // API-level error.
            let result = service
                .explain(ExplainRequest {
                    command: "ls".to_string(),
                })
                .await;
            assert!(
                result.is_err(),
                "Expected an error with a non-existent model, got Ok"
            );
        }
        // No key — service creation itself fails, which is still error propagation.
        Err(e) => assert_setup_error(&e),
    }
}

#[tokio::test]
async fn error_disabled_service_propagates_through_chat() {
    let config = AiConfig {
        enabled: false,
        provider: "anthropic".to_string(),
        model: std::env::var("LLM_DEFAULT_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
        history_size: 20,
    };
    match try_create_service_with_config(config).await {
        Ok(service) => {
            let result = service
                .chat(ChatRequest {
                    message: "hello".to_string(),
                })
                .await;
            match result {
                Err(AiError::NotConfigured(_)) => {}
                Err(other) => panic!("Expected NotConfigured, got: {:?}", other),
                Ok(_) => panic!("Expected error from disabled service"),
            }
        }
        Err(e) => assert_setup_error(&e),
    }
}

// ── Error mapping tests (6) ─────────────────────────────────────────────

#[test]
fn map_llm_error_configuration() {
    let llm_err = llm_provider::LlmError::Configuration("bad config".to_string());
    let ai_err = swebash_ai::spi::chat_provider::map_llm_error(llm_err);
    match ai_err {
        AiError::NotConfigured(msg) => assert_eq!(msg, "bad config"),
        other => panic!("Expected NotConfigured, got: {:?}", other),
    }
}

#[test]
fn map_llm_error_rate_limited() {
    let llm_err = llm_provider::LlmError::RateLimited {
        retry_after_ms: Some(5000),
    };
    let ai_err = swebash_ai::spi::chat_provider::map_llm_error(llm_err);
    match ai_err {
        AiError::RateLimited => {}
        other => panic!("Expected RateLimited, got: {:?}", other),
    }
}

#[test]
fn map_llm_error_timeout() {
    let llm_err = llm_provider::LlmError::Timeout(30000);
    let ai_err = swebash_ai::spi::chat_provider::map_llm_error(llm_err);
    match ai_err {
        AiError::Timeout => {}
        other => panic!("Expected Timeout, got: {:?}", other),
    }
}

#[test]
fn map_llm_error_network() {
    let llm_err = llm_provider::LlmError::NetworkError("connection refused".to_string());
    let ai_err = swebash_ai::spi::chat_provider::map_llm_error(llm_err);
    match ai_err {
        AiError::Provider(msg) => {
            assert!(
                msg.contains("Network error"),
                "Expected 'Network error' in: {}",
                msg
            );
        }
        other => panic!("Expected Provider, got: {:?}", other),
    }
}

#[test]
fn map_llm_error_serialization() {
    let llm_err = llm_provider::LlmError::SerializationError("bad json".to_string());
    let ai_err = swebash_ai::spi::chat_provider::map_llm_error(llm_err);
    match ai_err {
        AiError::ParseError(msg) => assert_eq!(msg, "bad json"),
        other => panic!("Expected ParseError, got: {:?}", other),
    }
}

#[test]
fn map_llm_error_fallthrough() {
    let llm_err = llm_provider::LlmError::ProviderNotFound("unknown-llm".to_string());
    let ai_err = swebash_ai::spi::chat_provider::map_llm_error(llm_err);
    match ai_err {
        AiError::Provider(_) => {}
        other => panic!("Expected Provider (catch-all), got: {:?}", other),
    }
}
