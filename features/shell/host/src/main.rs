mod spi;

use std::path::PathBuf;

use anyhow::{Context, Result};
use swebash_ai::{commands, handle_ai_command, output, AiCommand, AiService};
use swebash_readline::{Completer, Hinter, History, LineEditor, ReadlineConfig, ValidationResult, Validator};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env from next to the executable first, then fall back to cwd.
    // This means a double-clicked binary finds its .env in the same folder.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let _ = dotenvy::from_path(exe_dir.join(".env"));
        }
    }
    let _ = dotenvy::dotenv();

    // Initialize tracing subscriber. Honors RUST_LOG env var for filtering.
    // Default: warnings only. Example: RUST_LOG=swebash_ai=debug
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(true)
        .with_writer(std::io::stderr)
        .init();

    // Load workspace config from ~/.config/swebash/config.toml
    let config = spi::config::load_config();

    // Check for SWEBASH_WORKSPACE env var override
    let env_workspace = std::env::var("SWEBASH_WORKSPACE")
        .ok()
        .filter(|s| !s.is_empty());

    let has_env_workspace = env_workspace.is_some();

    // Resolve workspace root: env var > config > ~/workspace
    let expand_tilde = |s: &str| -> PathBuf {
        if s.starts_with("~/") || s == "~" {
            dirs::home_dir()
                .map(|h| h.join(s.strip_prefix("~/").unwrap_or("")))
                .unwrap_or_else(|| PathBuf::from(s))
        } else {
            PathBuf::from(s)
        }
    };

    let workspace_root = if let Some(ref env_ws) = env_workspace {
        expand_tilde(env_ws)
    } else {
        expand_tilde(&config.workspace.root)
    };

    // Auto-create workspace directory if it doesn't exist
    if !workspace_root.exists() {
        if let Err(e) = std::fs::create_dir_all(&workspace_root) {
            eprintln!(
                "warning: could not create workspace directory {}: {e}",
                workspace_root.display()
            );
        }
    }

    // Build sandbox policy from config
    let mut policy = config.into_policy();

    // If SWEBASH_WORKSPACE env var was set, override the root and default to
    // RW mode (explicit user choice). This ensures existing tests that set
    // the env var and perform writes continue to pass.
    if has_env_workspace {
        let canonical = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.clone());
        policy.workspace_root = canonical.clone();
        policy.allowed_paths.clear();
        policy.allowed_paths.push(spi::state::PathRule {
            root: canonical,
            mode: spi::state::AccessMode::ReadWrite,
        });
    }

    // Set initial working directory
    if workspace_root.exists() {
        let _ = std::env::set_current_dir(&workspace_root);
    } else {
        eprintln!(
            "warning: SWEBASH_WORKSPACE path does not exist: {}",
            workspace_root.display()
        );
        if let Some(home) = dirs::home_dir() {
            let _ = std::env::set_current_dir(&home);
        }
    }

    // Initialize AI service (None if unconfigured — shell continues without AI)
    let ai_service = swebash_ai::create_ai_service().await.ok();

    let (mut store, instance) = spi::runtime::setup(policy)?;

    // Grab exported functions
    let shell_init = instance
        .get_typed_func::<(), ()>(&mut store, "shell_init")
        .context("missing export: shell_init")?;

    let shell_eval = instance
        .get_typed_func::<u32, ()>(&mut store, "shell_eval")
        .context("missing export: shell_eval")?;

    let get_input_buf = instance
        .get_typed_func::<(), u32>(&mut store, "get_input_buf")
        .context("missing export: get_input_buf")?;

    let get_input_buf_len = instance
        .get_typed_func::<(), u32>(&mut store, "get_input_buf_len")
        .context("missing export: get_input_buf_len")?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .context("missing export: memory")?;

    // Call shell_init
    shell_init.call(&mut store, ())?;

    // REPL loop
    let buf_ptr = get_input_buf.call(&mut store, ())? as usize;
    let buf_cap = get_input_buf_len.call(&mut store, ())? as usize;

    // Load configuration
    let config = ReadlineConfig::load();

    // Initialize history with file persistence
    let history_path = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .or_else(dirs::home_dir)
        .map(|h| h.join(".swebash_history"))
        .unwrap_or_else(|| std::path::PathBuf::from(".swebash_history"));
    let mut history = History::with_file(config.max_history_size, history_path);

    // Initialize readline features
    let completer = Completer::new();
    let hinter = Hinter::new(config.colors.clone());
    let mut editor = LineEditor::new(config.clone(), hinter);

    let mut multiline_buffer = String::new();
    let mut recent_commands: Vec<String> = Vec::new();
    let max_recent: usize = 10;
    let mut ai_mode = false; // Track if we're in AI mode
    let mut ai_agent_id = String::from("shell"); // Track active agent for prompt

    let home_dir = dirs::home_dir();

    loop {
        // Show cwd in prompt, substituting ~ for home directory
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let display_cwd = match &home_dir {
            Some(h) => {
                let home_str = h.to_string_lossy();
                if cwd == home_str.as_ref() {
                    String::from("~")
                } else if cwd.starts_with(home_str.as_ref()) {
                    let rest = &cwd[home_str.len()..];
                    if rest.starts_with('/') || rest.starts_with('\\') {
                        format!("~{}", rest)
                    } else {
                        cwd
                    }
                } else {
                    cwd
                }
            }
            None => cwd,
        };
        // Determine prompt based on AI mode and multi-line mode
        let prompt = if ai_mode {
            if multiline_buffer.is_empty() {
                format!("\x1b[1;36m[AI:{}]\x1b[0m > ", ai_agent_id)
            } else {
                "\x1b[1;36m...\x1b[0m> ".to_string()
            }
        } else if multiline_buffer.is_empty() {
            format!("\x1b[1;32m{}\x1b[0m/> ", display_cwd)
        } else {
            "\x1b[1;32m...\x1b[0m> ".to_string()
        };

        // Read line with editor
        let line = match editor.read_line(&prompt, &history)? {
            Some(line) => line,
            None => {
                // EOF (Ctrl-D)
                if !multiline_buffer.is_empty() {
                    multiline_buffer.clear();
                    continue;
                }
                break;
            }
        };

        let input = line.trim_end();

        // Check for tab completion request (line ends with incomplete word + double space or tab)
        if config.enable_completion && (input.ends_with("  ") || input.ends_with('\t')) {
            let completion_line = input.trim_end();
            let completions = completer.complete(completion_line, completion_line.len());

            if !completions.is_empty() {
                println!("\nCompletions:");
                for comp in &completions {
                    println!("  {}", comp.display);
                }
                println!();
                multiline_buffer = completion_line.to_string();
                continue;
            }
        }

        // Add to multi-line buffer
        if !multiline_buffer.is_empty() {
            multiline_buffer.push('\n');
        }
        multiline_buffer.push_str(input);

        // Check if command is complete
        let validator = Validator::new();
        if validator.validate(&multiline_buffer) == ValidationResult::Incomplete {
            // Need more input
            continue;
        }

        // Command is complete, process it
        let cmd_with_leading_space = multiline_buffer.trim_end().to_string();
        let cmd = multiline_buffer.trim().to_string();

        // Clear multi-line buffer
        multiline_buffer.clear();

        if cmd.is_empty() {
            continue;
        }

        // Handle exit differently based on mode
        if cmd == "exit" {
            if ai_mode {
                // In AI mode, exit returns to shell
                ai_mode = false;
                println!("Exited AI mode.");
                continue;
            } else {
                // In shell mode, exit quits the shell
                break;
            }
        }

        // Add to history after checking for exit
        history.add(cmd_with_leading_space);

        // Handle commands based on current mode
        if ai_mode {
            // In AI mode: use smart detection
            let ai_cmd = commands::parse_ai_mode_command(&cmd);
            match ai_cmd {
                AiCommand::ExitMode => {
                    ai_mode = false;
                    println!("Exited AI mode.");
                }
                _ => {
                    // Auto-detect agent from input keywords before dispatch
                    if let Some(svc) = &ai_service {
                        if let Some(new_agent) = svc.auto_detect_and_switch(&cmd).await {
                            ai_agent_id = new_agent.clone();
                            let info = svc.current_agent().await;
                            output::ai_agent_switched(&info.id, &info.display_name);
                        }
                    }

                    // Track agent switches for the prompt
                    let is_switch = matches!(
                        &ai_cmd,
                        AiCommand::SwitchAgent(_)
                    );
                    handle_ai_command(&ai_service, ai_cmd, &recent_commands).await;
                    if is_switch {
                        if let Some(svc) = &ai_service {
                            ai_agent_id = svc.active_agent_id().await;
                        }
                    }
                }
            }
            continue;
        } else {
            // In shell mode: check for AI command triggers
            if let Some(ai_cmd) = commands::parse_ai_command(&cmd) {
                let enter_ai_mode = handle_ai_command(&ai_service, ai_cmd, &recent_commands).await;
                if enter_ai_mode {
                    ai_mode = true;
                    if let Some(svc) = &ai_service {
                        ai_agent_id = svc.active_agent_id().await;
                    }
                    println!("Entered AI mode. Type 'exit' or 'quit' to return to shell.");
                }
                continue;
            }
        }

        // Track recent commands for AI context
        recent_commands.push(cmd.clone());
        if recent_commands.len() > max_recent {
            recent_commands.remove(0);
        }

        let cmd_bytes = cmd.as_bytes();
        if cmd_bytes.len() > buf_cap {
            eprintln!(
                "[host] command too long ({} bytes, max {})",
                cmd_bytes.len(),
                buf_cap
            );
            continue;
        }

        memory.write(&mut store, buf_ptr, cmd_bytes)?;
        shell_eval.call(&mut store, cmd_bytes.len() as u32)?;
    }

    // History is automatically saved on Drop
    Ok(())
}
