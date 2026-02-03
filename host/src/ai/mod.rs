/// AI command handler for the host REPL.
pub mod commands;
pub mod output;

use std::io::{self, BufRead};

use swebash_ai::{
    AiService, AutocompleteRequest, ChatRequest, ChatStreamEvent, DefaultAiService,
    ExplainRequest, TranslateRequest,
};

use commands::AiCommand;

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
            return true;
        }
        AiCommand::ExitMode => {
            // This should only be called from AI mode, but handle gracefully
            output::ai_info("Not in AI mode.");
            return false;
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
        _ => {
            // All other commands require a configured service
            let Some(svc) = service else {
                output::ai_not_configured();
                return false;
            };
            match command {
                AiCommand::Ask(text) => handle_ask(svc, &text, recent_commands).await,
                AiCommand::Explain(cmd) => handle_explain(svc, &cmd).await,
                AiCommand::Chat(text) => handle_chat(svc, &text).await,
                AiCommand::Suggest => handle_suggest(svc, recent_commands).await,
                _ => unreachable!(),
            }
            false
        }
    }
}

/// Handle `ai ask` / `?` — translate NL to shell command.
async fn handle_ask(service: &DefaultAiService, text: &str, recent_commands: &[String]) {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    let request = TranslateRequest {
        natural_language: text.to_string(),
        cwd,
        recent_commands: recent_commands.to_vec(),
    };

    output::ai_thinking();
    let result = service.translate(request).await;
    output::ai_thinking_done();

    match result {
        Ok(response) => {
            output::ai_command(&response.command);
            output::ai_confirm_prompt();

            // Read confirmation
            let mut input = String::new();
            let _ = io::stdin().lock().read_line(&mut input);
            let choice = input.trim().to_lowercase();

            match choice.as_str() {
                "" | "y" | "yes" => {
                    // Execute the command by printing it so the user can see what ran
                    // The host REPL will need to execute this - we print it for now
                    output::ai_info(&format!("Executing: {}", response.command));
                    // Return the command to the caller would be ideal,
                    // but for now we use the OS to run it directly
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
                            output::ai_warn(&format!("Command exited with {}", s));
                        }
                        Err(e) => {
                            output::ai_error(&format!("Failed to execute: {}", e));
                        }
                        _ => {}
                    }
                }
                "e" | "edit" => {
                    output::ai_info(&format!("Command: {}", response.command));
                    output::ai_info("Copy and edit the command above, then paste it.");
                }
                _ => {
                    output::ai_info("Cancelled.");
                }
            }
        }
        Err(e) => {
            output::ai_error(&e.to_string());
        }
    }
}

/// Handle `ai explain` / `??` — explain a command.
async fn handle_explain(service: &DefaultAiService, cmd: &str) {
    let request = ExplainRequest {
        command: cmd.to_string(),
    };

    output::ai_thinking();
    let result = service.explain(request).await;
    output::ai_thinking_done();

    match result {
        Ok(response) => {
            output::ai_explanation(&response.explanation);
        }
        Err(e) => {
            output::ai_error(&e.to_string());
        }
    }
}

/// Handle `ai chat` — conversational assistant with streaming output.
async fn handle_chat(service: &DefaultAiService, text: &str) {
    let request = ChatRequest {
        message: text.to_string(),
    };

    output::ai_thinking();

    match service.chat_streaming(request).await {
        Ok(mut rx) => {
            output::ai_thinking_done();
            output::ai_reply_start();

            while let Some(event) = rx.recv().await {
                match event {
                    ChatStreamEvent::Delta(delta) => {
                        output::ai_reply_delta(&delta);
                    }
                    ChatStreamEvent::Done(final_text) => {
                        if !final_text.is_empty() {
                            output::ai_reply_delta(&final_text);
                        }
                        break;
                    }
                }
            }

            output::ai_reply_end();
        }
        Err(e) => {
            output::ai_thinking_done();
            output::ai_error(&e.to_string());
        }
    }
}

/// Handle `ai suggest` — autocomplete suggestions.
async fn handle_suggest(service: &DefaultAiService, recent_commands: &[String]) {
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

    output::ai_thinking();
    let result = service.autocomplete(request).await;
    output::ai_thinking_done();

    match result {
        Ok(response) => {
            if response.suggestions.is_empty() {
                output::ai_info("No suggestions available.");
            } else {
                output::ai_suggestions(&response.suggestions);
            }
        }
        Err(e) => {
            output::ai_error(&e.to_string());
        }
    }
}

/// Handle `ai status` — show configuration.
async fn handle_status(service: &Option<DefaultAiService>) {
    match service {
        Some(svc) => {
            let status = svc.status().await;
            output::ai_status(status.enabled, &status.provider, &status.model, status.ready);
        }
        None => {
            output::ai_status(false, "none", "none", false);
            output::ai_not_configured();
        }
    }
}

/// Handle `ai history` — show chat history.
async fn handle_history(service: &Option<DefaultAiService>) {
    match service {
        Some(svc) => {
            let history = svc.format_history().await;
            output::ai_explanation(&history);
        }
        None => {
            output::ai_not_configured();
        }
    }
}

/// Handle `ai clear` — clear chat history.
async fn handle_clear(service: &Option<DefaultAiService>) {
    match service {
        Some(svc) => {
            svc.clear_history().await;
            output::ai_success("Chat history cleared.");
        }
        None => {
            output::ai_not_configured();
        }
    }
}
