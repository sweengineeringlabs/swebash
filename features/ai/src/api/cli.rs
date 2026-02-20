use std::io::{self, BufRead};

use crate::{
    AiResult, AiService, AutocompleteRequest, ChatRequest, ChatStreamEvent, DefaultAiService,
    ExplainRequest, TranslateRequest,
};

use super::commands::AiCommand;

/// Handle an AI command.
///
/// This is called from the REPL loop when an AI command trigger is detected.
/// The `service` is `Option` — if `None`, the user gets a friendly "not configured" message.
///
/// Returns `true` if AI mode should be entered (for EnterMode command).
pub async fn handle_ai_command(
    service: &Option<DefaultAiService>,
    command: AiCommand,
    recent_commands: &[String],
) -> bool {
    match command {
        AiCommand::EnterMode => {
            // Signal to REPL that AI mode should be entered
            true
        }
        AiCommand::ExitMode => {
            // This should only be called from AI mode, but handle gracefully
            super::output::ai_info("Not in AI mode.");
            false
        }
        AiCommand::Status => {
            handle_status(service).await;
            false
        }
        AiCommand::History => {
            handle_history(service).await;
            false
        }
        AiCommand::Clear => {
            handle_clear(service).await;
            false
        }
        AiCommand::ListAgents => {
            handle_list_agents(service).await;
            false
        }
        AiCommand::SwitchAgent(agent_id) => {
            handle_switch_agent(service, &agent_id).await;
            true
        }
        AiCommand::AgentChat { agent, text } => {
            handle_agent_chat(service, &agent, &text).await;
            false
        }
        _ => {
            // All other commands require a configured service
            let Some(svc) = service else {
                super::output::ai_not_configured();
                return false;
            };
            super::output::ai_thinking();
            let result = match command {
                AiCommand::Ask(text) => handle_ask(svc, &text, recent_commands).await,
                AiCommand::Explain(cmd) => handle_explain(svc, &cmd).await,
                AiCommand::Chat(text) => handle_chat(svc, &text).await,
                AiCommand::Suggest => handle_suggest(svc, recent_commands).await,
                _ => unreachable!(),
            };
            if let Err(e) = result {
                super::output::ai_thinking_done();
                super::output::ai_error(&e.to_string());
            }
            false
        }
    }
}

/// Handle `ai ask` / `?` — translate NL to shell command.
async fn handle_ask(
    service: &DefaultAiService,
    text: &str,
    recent_commands: &[String],
) -> AiResult<()> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    let request = TranslateRequest {
        natural_language: text.to_string(),
        cwd,
        recent_commands: recent_commands.to_vec(),
    };

    let response = service.translate(request).await?;
    super::output::ai_thinking_done();

    super::output::ai_command(&response.command);
    super::output::ai_confirm_prompt();

    // Read confirmation
    let mut input = String::new();
    let _ = io::stdin().lock().read_line(&mut input);
    let choice = input.trim().to_lowercase();

    match choice.as_str() {
        "" | "y" | "yes" => {
            super::output::ai_info(&format!("Executing: {}", response.command));
            let status = std::process::Command::new(if cfg!(windows) {
                "cmd"
            } else {
                "sh"
            })
            .args(if cfg!(windows) {
                vec!["/C", &response.command]
            } else {
                vec!["-c", &response.command]
            })
            .status();

            match status {
                Ok(s) if !s.success() => {
                    super::output::ai_warn(&format!("Command exited with {}", s));
                }
                Err(e) => {
                    super::output::ai_error(&format!("Failed to execute: {}", e));
                }
                _ => {}
            }
        }
        "e" | "edit" => {
            super::output::ai_info(&format!("Command: {}", response.command));
            super::output::ai_info("Copy and edit the command above, then paste it.");
        }
        _ => {
            super::output::ai_info("Cancelled.");
        }
    }
    Ok(())
}

/// Handle `ai explain` / `??` — explain a command.
async fn handle_explain(service: &DefaultAiService, cmd: &str) -> AiResult<()> {
    let request = ExplainRequest {
        command: cmd.to_string(),
    };

    let response = service.explain(request).await?;
    super::output::ai_thinking_done();
    super::output::ai_explanation(&response.explanation);
    Ok(())
}

/// Handle `ai chat` — conversational assistant with streaming output.
async fn handle_chat(service: &DefaultAiService, text: &str) -> AiResult<()> {
    let request = ChatRequest {
        message: text.to_string(),
    };

    let mut rx = service.chat_streaming(request).await?;
    super::output::ai_thinking_done();
    super::output::ai_reply_start();

    while let Some(event) = rx.recv().await {
        match event {
            ChatStreamEvent::Delta(delta) => {
                super::output::ai_reply_delta(&delta);
            }
            ChatStreamEvent::Done(_) => {
                break;
            }
        }
    }

    super::output::ai_reply_end();
    Ok(())
}

/// Handle `ai suggest` — autocomplete suggestions.
async fn handle_suggest(service: &DefaultAiService, recent_commands: &[String]) -> AiResult<()> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    // List current directory entries for context
    let cwd_entries = std::fs::read_dir(&cwd)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .take(50) // limit for prompt size
                .collect()
        })
        .unwrap_or_default();

    let request = AutocompleteRequest {
        partial_input: String::new(),
        cwd,
        cwd_entries,
        recent_commands: recent_commands.to_vec(),
    };

    let response = service.autocomplete(request).await?;
    super::output::ai_thinking_done();

    if response.suggestions.is_empty() {
        super::output::ai_info("No suggestions available.");
    } else {
        super::output::ai_suggestions(&response.suggestions);
    }
    Ok(())
}

/// Handle `ai status` — show configuration.
async fn handle_status(service: &Option<DefaultAiService>) {
    match service {
        Some(svc) => {
            let status = svc.status().await;
            super::output::ai_status(status.enabled, &status.provider, &status.model, status.ready);
        }
        None => {
            super::output::ai_status(false, "none", "none", false);
            super::output::ai_not_configured();
        }
    }
}

/// Handle `ai history` — show chat history.
async fn handle_history(service: &Option<DefaultAiService>) {
    match service {
        Some(svc) => {
            let history = svc.format_history().await;
            super::output::ai_explanation(&history);
        }
        None => {
            super::output::ai_not_configured();
        }
    }
}

/// Handle `ai clear` — clear chat history.
async fn handle_clear(service: &Option<DefaultAiService>) {
    match service {
        Some(svc) => {
            svc.clear_history().await;
            super::output::ai_success("Chat history cleared.");
        }
        None => {
            super::output::ai_not_configured();
        }
    }
}

/// Handle `ai agents` — list all registered agents.
async fn handle_list_agents(service: &Option<DefaultAiService>) {
    match service {
        Some(svc) => {
            let agents = svc.list_agents().await;
            super::output::ai_agent_list(&agents);
        }
        None => {
            super::output::ai_not_configured();
        }
    }
}

/// Handle `@<agent>` — switch to a different agent.
async fn handle_switch_agent(service: &Option<DefaultAiService>, agent_id: &str) {
    match service {
        Some(svc) => match svc.switch_agent(agent_id).await {
            Ok(()) => {
                let info = svc.current_agent().await;
                super::output::ai_agent_switched(&info.id, &info.display_name);
            }
            Err(e) => {
                super::output::ai_error(&e.to_string());
            }
        },
        None => {
            super::output::ai_not_configured();
        }
    }
}

/// Handle `ai @<agent> <text>` — one-shot chat with a specific agent.
///
/// Temporarily switches to the specified agent, sends the message,
/// then switches back to the previous agent.
async fn handle_agent_chat(service: &Option<DefaultAiService>, agent_id: &str, text: &str) {
    let Some(svc) = service else {
        super::output::ai_not_configured();
        return;
    };

    // Remember current agent
    let previous = svc.active_agent_id().await;

    // Switch to requested agent
    if let Err(e) = svc.switch_agent(agent_id).await {
        super::output::ai_error(&e.to_string());
        return;
    }

    let info = svc.current_agent().await;
    super::output::ai_info(&format!("[{}] {}", info.id, info.display_name));

    // Chat with the agent (thinking indicator managed by caller)
    super::output::ai_thinking();
    if let Err(e) = handle_chat(svc, text).await {
        super::output::ai_error(&e.to_string());
    }

    // Switch back to previous agent
    let _ = svc.switch_agent(&previous).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SwitchAgent should return true so the REPL enters AI mode.
    /// This is the core fix — previously it returned false, causing
    /// subsequent input to be executed as shell commands.
    #[tokio::test]
    async fn switch_agent_enters_ai_mode() {
        let result =
            handle_ai_command(&None, AiCommand::SwitchAgent("devops".to_string()), &[]).await;
        assert!(result, "SwitchAgent should return true to enter AI mode");
    }

    /// SwitchAgent should enter AI mode for any agent name.
    #[tokio::test]
    async fn switch_agent_enters_ai_mode_all_agents() {
        for agent in &["devops", "git", "review", "shell"] {
            let result =
                handle_ai_command(&None, AiCommand::SwitchAgent(agent.to_string()), &[]).await;
            assert!(
                result,
                "SwitchAgent({}) should return true to enter AI mode",
                agent
            );
        }
    }

    /// EnterMode (bare `ai` command) should also return true.
    #[tokio::test]
    async fn enter_mode_enters_ai_mode() {
        let result = handle_ai_command(&None, AiCommand::EnterMode, &[]).await;
        assert!(result, "EnterMode should return true");
    }

    /// ExitMode should return false (not enter AI mode).
    #[tokio::test]
    async fn exit_mode_does_not_enter_ai_mode() {
        let result = handle_ai_command(&None, AiCommand::ExitMode, &[]).await;
        assert!(!result, "ExitMode should return false");
    }

    /// One-shot AgentChat should NOT enter persistent AI mode.
    #[tokio::test]
    async fn agent_chat_does_not_enter_ai_mode() {
        let cmd = AiCommand::AgentChat {
            agent: "devops".to_string(),
            text: "hello".to_string(),
        };
        let result = handle_ai_command(&None, cmd, &[]).await;
        assert!(!result, "AgentChat (one-shot) should not enter AI mode");
    }

    /// Status command should not enter AI mode.
    #[tokio::test]
    async fn status_does_not_enter_ai_mode() {
        let result = handle_ai_command(&None, AiCommand::Status, &[]).await;
        assert!(!result, "Status should return false");
    }

    /// ListAgents command should not enter AI mode.
    #[tokio::test]
    async fn list_agents_does_not_enter_ai_mode() {
        let result = handle_ai_command(&None, AiCommand::ListAgents, &[]).await;
        assert!(!result, "ListAgents should return false");
    }

    /// History command should not enter AI mode.
    #[tokio::test]
    async fn history_does_not_enter_ai_mode() {
        let result = handle_ai_command(&None, AiCommand::History, &[]).await;
        assert!(!result, "History should return false");
    }

    /// Clear command should not enter AI mode.
    #[tokio::test]
    async fn clear_does_not_enter_ai_mode() {
        let result = handle_ai_command(&None, AiCommand::Clear, &[]).await;
        assert!(!result, "Clear should return false");
    }

    /// Chat command should not enter AI mode.
    #[tokio::test]
    async fn chat_does_not_enter_ai_mode() {
        let result =
            handle_ai_command(&None, AiCommand::Chat("hello".to_string()), &[]).await;
        assert!(!result, "Chat should return false");
    }
}
