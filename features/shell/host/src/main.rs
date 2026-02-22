mod spi;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use swebash_ai::{commands, handle_ai_command, output, AiCommand, AiService, DefaultAiService};
use swebash_readline::{
    Completer, EditorAction, Hinter, History, LineEditor, ReadlineConfig, ValidationResult,
    Validator,
};

use spi::tab::{TabInner, TabKind, TabManager};

/// Maximum number of recent commands kept per tab for AI context.
const MAX_RECENT: usize = 10;

/// Shorten a CWD path by replacing the home directory prefix with `~`.
/// Also normalizes backslashes to forward slashes for copy-paste compatibility.
fn display_cwd(cwd: &str, home: Option<&PathBuf>) -> String {
    // Normalize backslashes to forward slashes
    let cwd = cwd.replace('\\', "/");

    let result = match home {
        Some(h) => {
            let home_str = h.to_string_lossy().replace('\\', "/");
            if cwd == home_str {
                String::from("~")
            } else if cwd.starts_with(&home_str) {
                let rest = &cwd[home_str.len()..];
                if rest.starts_with('/') {
                    format!("~{rest}")
                } else {
                    cwd.to_string()
                }
            } else {
                cwd.to_string()
            }
        }
        None => cwd.to_string(),
    };

    result
}

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
    let mut config = spi::config::load_config();

    // Run first-run setup wizard if not yet completed
    if !config.setup_completed {
        // Wizard may fail (user abort / Ctrl+C) — that's fine, we continue
        let _ = spi::setup::run_setup_wizard(&mut config);
    }

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
    let mut policy = config.to_policy();

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

    // Determine initial working directory
    let initial_cwd = if workspace_root.exists() {
        workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.clone())
    } else {
        eprintln!(
            "warning: SWEBASH_WORKSPACE path does not exist: {}",
            workspace_root.display()
        );
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    };

    // Build git safety gate enforcer from merged config
    let git_enforcer = Arc::new(spi::git_gates::load_gates(&initial_cwd));
    let git_enforcer_opt = Some(git_enforcer.clone());

    // Initialize AI service (None if disabled or unconfigured — shell continues without AI)
    // Env var SWEBASH_AI_ENABLED takes precedence over config file
    let ai_enabled = std::env::var("SWEBASH_AI_ENABLED")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(config.ai.enabled);
    let ai_service = if ai_enabled {
        swebash_ai::create_ai_service().await.ok()
    } else {
        None
    };

    // Load readline configuration
    let rl_config = ReadlineConfig::load("swebash");

    // Initialize history with file persistence (shared across all tabs)
    // XDG-compliant: ~/.local/state/swebash/history
    let history_path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
        .map(|h| {
            let xdg_state_dir = h.join(".local").join("state").join("swebash");
            let xdg_history_path = xdg_state_dir.join("history");
            let legacy_path = h.join(".swebash_history");

            // Ensure XDG state directory exists
            if let Err(e) = std::fs::create_dir_all(&xdg_state_dir) {
                eprintln!("warning: could not create {}: {e}", xdg_state_dir.display());
            }

            // Migrate legacy history file to XDG location if needed
            if legacy_path.exists() && !xdg_history_path.exists() {
                if let Err(e) = std::fs::rename(&legacy_path, &xdg_history_path) {
                    eprintln!(
                        "warning: could not migrate {} to {}: {e}",
                        legacy_path.display(),
                        xdg_history_path.display()
                    );
                }
            }

            xdg_history_path
        })
        .unwrap_or_else(|| PathBuf::from(".swebash_history"));
    let history = Arc::new(Mutex::new(History::with_file(
        rl_config.max_history_size,
        history_path,
    )));

    // Initialize readline features
    let completer = Completer::new();
    let hinter = Hinter::new(rl_config.colors.clone());
    let mut editor = LineEditor::new(rl_config.clone(), hinter);

    // Create tab manager with one initial shell tab
    let mut tab_mgr = TabManager::new(history.clone());
    tab_mgr.create_shell_tab(initial_cwd, policy.clone(), git_enforcer_opt.clone())?;
    tab_mgr.switch_to(0);

    let home_dir = dirs::home_dir();
    let mut tab_bar_active = false;

    loop {
        // Manage tab bar visibility: show when 2+ tabs exist
        let should_show_bar = tab_mgr.tabs.len() > 1;
        if should_show_bar && !tab_bar_active {
            spi::tab_bar::setup_scroll_region();
            tab_bar_active = true;
        } else if !should_show_bar && tab_bar_active {
            spi::tab_bar::reset_scroll_region();
            tab_bar_active = false;
        }
        if tab_bar_active {
            spi::tab_bar::render_tab_bar(&tab_mgr, home_dir.as_ref());
        }

        let tab = tab_mgr.active_tab();
        let cwd = tab.virtual_cwd();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let dcwd = display_cwd(&cwd_str, home_dir.as_ref());
        let ai_mode = tab.ai_mode;
        let ai_agent_id = tab.ai_agent_id.clone();
        let multiline_empty = tab.multiline_buffer.is_empty();

        // Determine prompt based on tab kind and AI mode
        let tab_kind = tab.kind();
        let prompt = match tab_kind {
            TabKind::HistoryView => "\x1b[1;33m[history]\x1b[0m search> ".to_string(),
            TabKind::Ai => {
                if multiline_empty {
                    format!("\x1b[1;36m[AI:{}]\x1b[0m > ", ai_agent_id)
                } else {
                    "\x1b[1;36m...\x1b[0m> ".to_string()
                }
            }
            TabKind::Shell => {
                if ai_mode {
                    if multiline_empty {
                        format!("\x1b[1;36m[AI:{}]\x1b[0m > ", ai_agent_id)
                    } else {
                        "\x1b[1;36m...\x1b[0m> ".to_string()
                    }
                } else if multiline_empty {
                    format!("\x1b[1;32m{}\x1b[0m/> ", dcwd)
                } else {
                    "\x1b[1;32m...\x1b[0m> ".to_string()
                }
            }
        };

        // Read line with editor
        let history_guard = history.lock().unwrap();
        let action = editor.read_line(&prompt, &history_guard)?;
        drop(history_guard);

        // Handle tab-related editor actions
        match action {
            EditorAction::TabNew => {
                let cwd = tab_mgr.active_tab().virtual_cwd();
                match tab_mgr.create_shell_tab(cwd, policy.clone(), git_enforcer_opt.clone()) {
                    Ok(idx) => {
                        tab_mgr.switch_to(idx);
                        println!("Switched to tab {}.", idx + 1);
                    }
                    Err(e) => eprintln!("tab new: failed to create tab: {e}"),
                }
                continue;
            }
            EditorAction::TabClose => {
                if tab_mgr.close_active() {
                    break;
                }
                println!("Tab closed. Now on tab {}.", tab_mgr.active + 1);
                continue;
            }
            EditorAction::TabNext => {
                tab_mgr.switch_next();
                println!("Switched to tab {}.", tab_mgr.active + 1);
                continue;
            }
            EditorAction::TabPrev => {
                tab_mgr.switch_prev();
                println!("Switched to tab {}.", tab_mgr.active + 1);
                continue;
            }
            EditorAction::TabGoto(n) => {
                if n < tab_mgr.tabs.len() {
                    tab_mgr.switch_to(n);
                    println!("Switched to tab {}.", n + 1);
                } else {
                    println!("tab: no tab {}", n + 1);
                }
                continue;
            }
            EditorAction::Eof => {
                let tab = tab_mgr.active_tab_mut();
                if !tab.multiline_buffer.is_empty() {
                    tab.multiline_buffer.clear();
                    continue;
                }
                // Close current tab; exit if it was the last
                if tab_mgr.close_active() {
                    break;
                }
                continue;
            }
            EditorAction::Continue => {
                // Should not reach here from read_line, but handle gracefully
                continue;
            }
            EditorAction::Submit => {
                // Line is ready — fall through to process it
            }
        }

        let line = editor.line().to_string();

        let input = line.trim_end();

        // Check for tab completion request
        if rl_config.enable_completion && (input.ends_with("  ") || input.ends_with('\t')) {
            let completion_line = input.trim_end();
            let completions = completer.complete(completion_line, completion_line.len());

            if !completions.is_empty() {
                println!("\nCompletions:");
                for comp in &completions {
                    println!("  {}", comp.display);
                }
                println!();
                tab_mgr.active_tab_mut().multiline_buffer = completion_line.to_string();
                continue;
            }
        }

        // Add to multi-line buffer
        {
            let tab = tab_mgr.active_tab_mut();
            if !tab.multiline_buffer.is_empty() {
                tab.multiline_buffer.push('\n');
            }
            tab.multiline_buffer.push_str(input);
        }

        // Check if command is complete
        let validator = Validator::new();
        if validator.validate(&tab_mgr.active_tab().multiline_buffer) == ValidationResult::Incomplete
        {
            continue;
        }

        // Command is complete, process it
        let cmd_with_leading_space;
        let cmd;
        {
            let tab = tab_mgr.active_tab_mut();
            cmd_with_leading_space = tab.multiline_buffer.trim_end().to_string();
            cmd = tab.multiline_buffer.trim().to_string();
            tab.multiline_buffer.clear();
        }

        if cmd.is_empty() {
            continue;
        }

        // Handle exit/quit: close mode tabs, exit AI mode, or close shell tab
        if cmd == "exit" || cmd == "quit" {
            let tab = tab_mgr.active_tab();
            let kind = tab.kind();
            match kind {
                TabKind::Ai | TabKind::HistoryView => {
                    // Mode tabs: close the tab on exit/quit
                    if tab_mgr.close_active() {
                        break;
                    }
                    println!("Tab closed. Now on tab {}.", tab_mgr.active + 1);
                    continue;
                }
                TabKind::Shell => {
                    let tab = tab_mgr.active_tab_mut();
                    if tab.ai_mode {
                        tab.ai_mode = false;
                        println!("Exited AI mode.");
                        continue;
                    }
                    // Close shell tab; exit if it was the last
                    if tab_mgr.close_active() {
                        break;
                    }
                    continue;
                }
            }
        }

        // Re-run setup wizard on demand
        if cmd == "setup" {
            let _ = spi::setup::run_setup_wizard(&mut config);
            continue;
        }

        // For HistoryView tabs: 'q' also closes the tab
        if cmd == "q" && tab_mgr.active_tab().kind() == TabKind::HistoryView {
            if tab_mgr.close_active() {
                break;
            }
            println!("Tab closed. Now on tab {}.", tab_mgr.active + 1);
            continue;
        }

        // Add to shared history after checking for exit
        {
            let mut h = history.lock().unwrap();
            h.add(cmd_with_leading_space);
        }

        // --- Tab commands (intercepted before AI/WASM dispatch) ---
        if let Some(action) = parse_tab_command(&cmd) {
            handle_tab_command(&mut tab_mgr, action, &policy, &ai_service, &git_enforcer_opt).await;
            continue;
        }

        // --- Dispatch based on tab kind ---
        let kind = tab_mgr.active_tab().kind();
        match kind {
            TabKind::HistoryView => {
                // In history tab: typed text searches history, Enter copies to clipboard
                process_history_view(&tab_mgr, &cmd);
                continue;
            }
            TabKind::Ai => {
                // AI tabs are always in AI mode
                process_ai_mode(&mut tab_mgr, &ai_service, &cmd).await;
                continue;
            }
            TabKind::Shell => {
                // Fall through to shell processing
            }
        }

        // Handle commands based on current mode (shell tabs only)
        let tab_ai_mode = tab_mgr.active_tab().ai_mode;

        if tab_ai_mode {
            process_ai_mode(
                &mut tab_mgr,
                &ai_service,
                &cmd,
            )
            .await;
            continue;
        }

        // In shell mode: check for AI command triggers
        {
            let tab = tab_mgr.active_tab();
            if let Some(ai_cmd) = commands::parse_ai_command(&cmd) {
                let recent = tab.recent_commands.clone();
                let enter_ai_mode =
                    handle_ai_command(&ai_service, ai_cmd, &recent).await;
                if enter_ai_mode {
                    let tab = tab_mgr.active_tab_mut();
                    tab.ai_mode = true;
                    if let Some(svc) = &ai_service {
                        tab.ai_agent_id = svc.active_agent_id().await;
                    }
                    println!("Entered AI mode. Type 'exit' or 'quit' to return to shell.");
                }
                continue;
            }
        }

        // Track recent commands for AI context
        {
            let tab = tab_mgr.active_tab_mut();
            tab.recent_commands.push(cmd.clone());
            if tab.recent_commands.len() > MAX_RECENT {
                tab.recent_commands.remove(0);
            }
        }

        // Dispatch to WASM engine (shell tabs only — type-level guarantee)
        {
            let tab = tab_mgr.active_tab_mut();
            let TabInner::Shell(ref mut wasm) = tab.inner else {
                unreachable!("only Shell tabs reach WASM dispatch");
            };
            let cmd_bytes = cmd.as_bytes();
            if cmd_bytes.len() > wasm.buf_cap {
                eprintln!(
                    "[host] command too long ({} bytes, max {})",
                    cmd_bytes.len(),
                    wasm.buf_cap
                );
                continue;
            }
            wasm.memory
                .write(&mut wasm.store, wasm.buf_ptr, cmd_bytes)?;
            wasm.shell_eval
                .call(&mut wasm.store, cmd_bytes.len() as u32)?;
        }
    }

    // Reset scroll region before exiting
    if tab_bar_active {
        spi::tab_bar::reset_scroll_region();
    }

    // History is automatically saved on Drop (via Arc/Mutex)
    Ok(())
}

/// Process a command while in AI mode for the active tab.
async fn process_ai_mode(
    tab_mgr: &mut TabManager,
    ai_service: &Option<DefaultAiService>,
    cmd: &str,
) {
    let ai_cmd = commands::parse_ai_mode_command(cmd);
    match ai_cmd {
        AiCommand::ExitMode => {
            let tab = tab_mgr.active_tab_mut();
            tab.ai_mode = false;
            println!("Exited AI mode.");
        }
        _ => {
            // Auto-detect agent from input keywords before dispatch
            if let Some(svc) = ai_service {
                if let Some(new_agent) = svc.auto_detect_and_switch(cmd).await {
                    tab_mgr.active_tab_mut().ai_agent_id = new_agent.clone();
                    let info = svc.current_agent().await;
                    output::ai_agent_switched(&info.id, &info.display_name);
                }
            }

            let is_switch = matches!(&ai_cmd, AiCommand::SwitchAgent(_));
            let recent = tab_mgr.active_tab().recent_commands.clone();
            handle_ai_command(ai_service, ai_cmd, &recent).await;
            if is_switch {
                if let Some(svc) = ai_service {
                    tab_mgr.active_tab_mut().ai_agent_id = svc.active_agent_id().await;
                }
            }
        }
    }
}

/// Process a command in the history view tab.
/// Typed text searches the shared history; an empty Enter shows all entries.
fn process_history_view(tab_mgr: &TabManager, query: &str) {
    let history = tab_mgr.history.lock().unwrap();
    let query_lower = query.to_lowercase();
    let mut shown = 0usize;

    if query.is_empty() {
        // Show all history entries
        for i in 0..history.len() {
            if let Some(entry) = history.get(i) {
                println!("{:>5}  {}", i + 1, entry);
                shown += 1;
            }
        }
    } else {
        // Search history
        for i in 0..history.len() {
            if let Some(entry) = history.get(i) {
                if entry.to_lowercase().contains(&query_lower) {
                    println!("{:>5}  {}", i + 1, entry);
                    shown += 1;
                }
            }
        }
    }

    if shown == 0 {
        println!("(no matching history entries)");
    }
    println!();
    println!("Type a search term to filter, 'q' or 'exit' to close this tab.");
}

// ---------------------------------------------------------------------------
// Tab command parsing and handling
// ---------------------------------------------------------------------------

/// Parsed tab command.
enum TabAction {
    List,
    New { path: Option<PathBuf> },
    Ai { agent: Option<String> },
    History,
    Close,
    SwitchTo(usize),
    Rename(String),
}

/// Try to parse a `tab ...` command. Returns `None` if the input is not a
/// tab command.
fn parse_tab_command(cmd: &str) -> Option<TabAction> {
    let trimmed = cmd.trim();
    if !trimmed.starts_with("tab") {
        return None;
    }
    // Must be exactly "tab" or "tab " followed by something
    if trimmed.len() > 3 && !trimmed.as_bytes()[3].is_ascii_whitespace() {
        return None;
    }
    let args: Vec<&str> = trimmed.split_whitespace().skip(1).collect();
    match args.first().copied() {
        None | Some("list") => Some(TabAction::List),
        Some("new") => {
            let path = args.get(1).map(|s| PathBuf::from(s));
            Some(TabAction::New { path })
        }
        Some("ai") => {
            let agent = args.get(1).map(|s| s.to_string());
            Some(TabAction::Ai { agent })
        }
        Some("history") => Some(TabAction::History),
        Some("close") => Some(TabAction::Close),
        Some("rename") => {
            let name = args[1..].join(" ");
            if name.is_empty() {
                println!("usage: tab rename <name>");
                return None;
            }
            Some(TabAction::Rename(name))
        }
        Some(n) => {
            // Try to parse as tab number (1-based for the user)
            if let Ok(num) = n.parse::<usize>() {
                if num >= 1 {
                    Some(TabAction::SwitchTo(num - 1))
                } else {
                    println!("tab: invalid tab number");
                    None
                }
            } else {
                println!("tab: unknown subcommand '{n}'");
                None
            }
        }
    }
}

/// Execute a parsed tab command.
async fn handle_tab_command(
    tab_mgr: &mut TabManager,
    action: TabAction,
    policy: &spi::state::SandboxPolicy,
    ai_service: &Option<DefaultAiService>,
    git_enforcer: &Option<Arc<spi::git_gates::GitGateEnforcer>>,
) {
    let home_dir = dirs::home_dir();
    match action {
        TabAction::List => {
            for (i, tab) in tab_mgr.tabs.iter().enumerate() {
                let marker = if i == tab_mgr.active { "*" } else { " " };
                let label = tab.display_label(home_dir.as_ref());
                println!("{marker}{}  [{}]", i + 1, label);
            }
        }
        TabAction::New { path } => {
            let cwd = match path {
                Some(p) => {
                    let resolved = if p.is_absolute() {
                        p
                    } else {
                        tab_mgr.active_tab().virtual_cwd().join(&p)
                    };
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    if !canonical.is_dir() {
                        eprintln!("tab new: not a directory: {}", canonical.display());
                        return;
                    }
                    canonical
                }
                None => tab_mgr.active_tab().virtual_cwd(),
            };
            match tab_mgr.create_shell_tab(cwd, policy.clone(), git_enforcer.clone()) {
                Ok(idx) => {
                    tab_mgr.switch_to(idx);
                    println!("Switched to tab {}.", idx + 1);
                }
                Err(e) => eprintln!("tab new: failed to create tab: {e}"),
            }
        }
        TabAction::Ai { agent } => {
            let agent_id = agent.unwrap_or_else(|| "shell".to_string());
            // If an AI service is available, try to switch agent
            if let Some(svc) = ai_service {
                let _ = svc.auto_detect_and_switch(&agent_id).await;
            }
            let cwd = tab_mgr.active_tab().virtual_cwd();
            let idx = tab_mgr.create_ai_tab(&agent_id, cwd);
            println!("Opened AI tab {} (agent: {}).", idx + 1, agent_id);
        }
        TabAction::History => {
            let cwd = tab_mgr.active_tab().virtual_cwd();
            let idx = tab_mgr.create_history_tab(cwd);
            println!("Opened history tab {}.", idx + 1);
        }
        TabAction::Close => {
            let idx = tab_mgr.active + 1;
            if tab_mgr.close_active() {
                // This was the last tab — the caller REPL loop will catch EOF
                // on the next iteration. Print a message and return.
                println!("Last tab closed. Exiting.");
                std::process::exit(0);
            }
            println!("Closed tab {idx}. Now on tab {}.", tab_mgr.active + 1);
        }
        TabAction::SwitchTo(index) => {
            if index < tab_mgr.tabs.len() {
                tab_mgr.switch_to(index);
                println!("Switched to tab {}.", index + 1);
            } else {
                println!("tab: no tab {}", index + 1);
            }
        }
        TabAction::Rename(name) => {
            tab_mgr.active_tab_mut().label = name.clone();
            println!("Tab renamed to '{name}'.");
        }
    }
}
