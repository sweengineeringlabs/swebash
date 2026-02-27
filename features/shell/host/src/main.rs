mod spi;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use mdc_logging::{LogContext, set_agent_id, set_conversation_turn, with_log_context};
use tracing::{debug, error, info_span, instrument, warn};
use tracing_subscriber::prelude::*;
use swebash_llm::{commands, handle_ai_command, output, AiCommand, AiService, DefaultAiService};
use swebash_readline::{
    Completer, EditorAction, Hinter, History, LineEditor, ReadlineConfig, ValidationResult,
    Validator,
};

use spi::tab::{TabInner, TabKind, TabManager};

/// Maximum number of recent commands kept per tab for AI context.
const MAX_RECENT: usize = 10;

/// Context for the main REPL loop, grouping all required state.
struct MainLoopContext {
    // Session identity
    session_id: String,
    user_id: String,

    // Shell state
    tab_mgr: TabManager,
    editor: LineEditor,
    history: Arc<Mutex<History>>,
    completer: Completer,

    // Configuration
    config: spi::config::SwebashConfig,
    policy: spi::state::SandboxPolicy,
    rl_config: ReadlineConfig,

    // Services
    ai_service: Option<DefaultAiService>,
    git_enforcer: Option<Arc<spi::git_gates::GitGateEnforcer>>,

    // Environment
    home_dir: Option<PathBuf>,
}

/// Builder for MainLoopContext.
struct MainLoopContextBuilder {
    session_id: Option<String>,
    user_id: Option<String>,
    tab_mgr: Option<TabManager>,
    editor: Option<LineEditor>,
    history: Option<Arc<Mutex<History>>>,
    completer: Option<Completer>,
    config: Option<spi::config::SwebashConfig>,
    policy: Option<spi::state::SandboxPolicy>,
    rl_config: Option<ReadlineConfig>,
    ai_service: Option<DefaultAiService>,
    git_enforcer: Option<Arc<spi::git_gates::GitGateEnforcer>>,
    home_dir: Option<PathBuf>,
}

impl MainLoopContextBuilder {
    fn new() -> Self {
        Self {
            session_id: None,
            user_id: None,
            tab_mgr: None,
            editor: None,
            history: None,
            completer: None,
            config: None,
            policy: None,
            rl_config: None,
            ai_service: None,
            git_enforcer: None,
            home_dir: None,
        }
    }

    fn session_id(mut self, id: String) -> Self {
        self.session_id = Some(id);
        self
    }

    fn user_id(mut self, id: String) -> Self {
        self.user_id = Some(id);
        self
    }

    fn tab_mgr(mut self, mgr: TabManager) -> Self {
        self.tab_mgr = Some(mgr);
        self
    }

    fn editor(mut self, editor: LineEditor) -> Self {
        self.editor = Some(editor);
        self
    }

    fn history(mut self, history: Arc<Mutex<History>>) -> Self {
        self.history = Some(history);
        self
    }

    fn completer(mut self, completer: Completer) -> Self {
        self.completer = Some(completer);
        self
    }

    fn config(mut self, config: spi::config::SwebashConfig) -> Self {
        self.config = Some(config);
        self
    }

    fn policy(mut self, policy: spi::state::SandboxPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    fn rl_config(mut self, rl_config: ReadlineConfig) -> Self {
        self.rl_config = Some(rl_config);
        self
    }

    fn ai_service(mut self, service: Option<DefaultAiService>) -> Self {
        self.ai_service = service;
        self
    }

    fn git_enforcer(mut self, enforcer: Option<Arc<spi::git_gates::GitGateEnforcer>>) -> Self {
        self.git_enforcer = enforcer;
        self
    }

    fn home_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.home_dir = dir;
        self
    }

    fn build(self) -> Result<MainLoopContext> {
        Ok(MainLoopContext {
            session_id: self.session_id.ok_or_else(|| anyhow::anyhow!("session_id required"))?,
            user_id: self.user_id.ok_or_else(|| anyhow::anyhow!("user_id required"))?,
            tab_mgr: self.tab_mgr.ok_or_else(|| anyhow::anyhow!("tab_mgr required"))?,
            editor: self.editor.ok_or_else(|| anyhow::anyhow!("editor required"))?,
            history: self.history.ok_or_else(|| anyhow::anyhow!("history required"))?,
            completer: self.completer.ok_or_else(|| anyhow::anyhow!("completer required"))?,
            config: self.config.ok_or_else(|| anyhow::anyhow!("config required"))?,
            policy: self.policy.ok_or_else(|| anyhow::anyhow!("policy required"))?,
            rl_config: self.rl_config.ok_or_else(|| anyhow::anyhow!("rl_config required"))?,
            ai_service: self.ai_service,
            git_enforcer: self.git_enforcer,
            home_dir: self.home_dir,
        })
    }
}

/// Shorten a CWD path by replacing the home directory prefix with `~`.
/// Also normalizes backslashes to forward slashes for copy-paste compatibility.
fn display_cwd(cwd: &str, home: Option<&PathBuf>) -> String {
    // Strip Windows extended-length path prefix if present
    let cwd = cwd.strip_prefix(r"\\?\").unwrap_or(cwd);
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
    // Default: warnings only. Example: RUST_LOG=swebash=debug
    // Set SWEBASH_LOG_FORMAT=json for JSON output (useful for log aggregation).
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));

    let use_json = std::env::var("SWEBASH_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    if use_json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json().with_writer(std::io::stderr))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .init();
    }

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
            warn!(
                path = %workspace_root.display(),
                error = %e,
                "could not create workspace directory"
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
        warn!(
            path = %workspace_root.display(),
            "SWEBASH_WORKSPACE path does not exist, using home directory"
        );
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    };

    // Build git safety gate enforcer from merged config
    let git_enforcer = Arc::new(spi::git_gates::load_gates(&initial_cwd));
    let git_enforcer_opt = Some(git_enforcer.clone());

    // Verify workspace-repository binding if configured
    // This prevents accidentally working in a workspace bound to a different repo
    if let Some(remote) = spi::git_gates::current_remote(&initial_cwd, "origin") {
        let cwd_str = initial_cwd.to_string_lossy();
        if let Err(msg) = config.verify_repo_binding(&cwd_str, &remote) {
            error!(
                workspace = %cwd_str,
                remote = %remote,
                "workspace-repository binding mismatch"
            );
            // Also print user-friendly message since this is fatal
            eprintln!("\x1b[1;31merror:\x1b[0m {msg}");
            eprintln!(
                "\x1b[90mTo unbind, remove entry from ~/.config/swebash/config.toml\x1b[0m"
            );
            std::process::exit(1);
        }
    }

    // Initialize AI service (None if disabled or unconfigured — shell continues without AI)
    // XDG config file provides fallback values; env vars always take precedence
    // Set env vars from config file if not already set
    if std::env::var("LLM_PROVIDER").is_err() {
        if let Some(ref provider) = config.ai.provider {
            std::env::set_var("LLM_PROVIDER", provider);
        }
    }
    if std::env::var("LLM_DEFAULT_MODEL").is_err() {
        if let Some(ref model) = config.ai.model {
            std::env::set_var("LLM_DEFAULT_MODEL", model);
        }
    }
    // Set API keys from config if not in env (XDG config file is less secure than env vars,
    // but more convenient for users who don't want to set env vars every session)
    if std::env::var("OPENAI_API_KEY").is_err() {
        if let Some(ref key) = config.ai.openai_api_key {
            std::env::set_var("OPENAI_API_KEY", key);
        }
    }
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        if let Some(ref key) = config.ai.anthropic_api_key {
            std::env::set_var("ANTHROPIC_API_KEY", key);
        }
    }
    if std::env::var("GEMINI_API_KEY").is_err() {
        if let Some(ref key) = config.ai.gemini_api_key {
            std::env::set_var("GEMINI_API_KEY", key);
        }
    }

    // Env var SWEBASH_AI_ENABLED takes precedence over config file
    let ai_enabled = std::env::var("SWEBASH_AI_ENABLED")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(config.ai.enabled);

    // Warn if AI is enabled but no API key is configured
    if ai_enabled && !config.ai.has_api_key() {
        let provider = config.ai.effective_provider();
        let env_var = match provider.as_str() {
            "anthropic" => "ANTHROPIC_API_KEY",
            "gemini" => "GEMINI_API_KEY",
            _ => "OPENAI_API_KEY",
        };
        warn!(
            provider = %provider,
            env_var = %env_var,
            "AI mode enabled but no API key configured"
        );
    }

    let ai_service = if ai_enabled {
        // Create AI sandbox from workspace policy
        let ai_sandbox = if policy.enabled {
            let mut rules = Vec::new();
            for path_rule in &policy.allowed_paths {
                let mode = match path_rule.mode {
                    spi::state::AccessMode::ReadOnly => swebash_llm::SandboxAccessMode::ReadOnly,
                    spi::state::AccessMode::ReadWrite => swebash_llm::SandboxAccessMode::ReadWrite,
                };
                rules.push(swebash_llm::SandboxRule {
                    path: path_rule.root.clone(),
                    mode,
                });
            }
            // Initialize sandbox with initial_cwd for correct relative path resolution
            Some(Arc::new(swebash_llm::ToolSandbox::with_rules_and_cwd(
                rules,
                true,
                initial_cwd.clone(),
            )))
        } else {
            None
        };
        swebash_llm::create_ai_service_with_sandbox(ai_sandbox).await.ok()
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
            if let Err(e) = std::fs::create_dir_all(&xdg_state_dir) {
                warn!(path = %xdg_state_dir.display(), error = %e, "could not create XDG state directory");
            }
            xdg_state_dir.join("history")
        })
        .unwrap_or_else(|| PathBuf::from(".swebash_history"));
    let history = Arc::new(Mutex::new(History::with_file(
        rl_config.max_history_size,
        history_path,
    )));

    // Initialize readline features
    let completer = Completer::new();
    let hinter = Hinter::new(rl_config.colors.clone());
    let editor = LineEditor::new(rl_config.clone(), hinter);

    // Create tab manager with one initial shell tab
    let mut tab_mgr = TabManager::new(history.clone());
    tab_mgr.create_shell_tab(initial_cwd, policy.clone(), git_enforcer_opt.clone())?;
    tab_mgr.switch_to(0);

    let home_dir = dirs::home_dir();

    // MDC context fields (per logging-strategies.md)
    let session_id = uuid::Uuid::new_v4().to_string();
    let user_id = config
        .git
        .as_ref()
        .map(|g| g.user_id.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Create LogContext for MDC propagation across async task boundaries
    let log_ctx = LogContext::builder()
        .session_id(&session_id)
        .user_id(&user_id)
        .build();

    // Build main loop context
    let ctx = MainLoopContextBuilder::new()
        .session_id(session_id.clone())
        .user_id(user_id.clone())
        .tab_mgr(tab_mgr)
        .editor(editor)
        .history(history)
        .completer(completer)
        .config(config)
        .policy(policy)
        .rl_config(rl_config)
        .ai_service(ai_service)
        .git_enforcer(git_enforcer_opt)
        .home_dir(home_dir)
        .build()?;

    // Run main loop with MDC context
    with_log_context(log_ctx, run_main_loop(ctx)).await
}

/// Main REPL loop, wrapped with MDC context for async propagation.
async fn run_main_loop(mut ctx: MainLoopContext) -> Result<()> {
    let session_span = info_span!(
        "session",
        session_id = %ctx.session_id,
        user_id = %ctx.user_id,
        workspace = %ctx.policy.workspace_root.display(),
    );
    let _session_guard = session_span.enter();

    let mut tab_bar_active = false;
    let mut cmd_count: u64 = 0;

    loop {
        // Manage tab bar visibility: show when 2+ tabs exist
        let should_show_bar = ctx.tab_mgr.tabs.len() > 1;
        if should_show_bar && !tab_bar_active {
            spi::tab_bar::setup_scroll_region();
            tab_bar_active = true;
        } else if !should_show_bar && tab_bar_active {
            spi::tab_bar::reset_scroll_region();
            tab_bar_active = false;
        }
        if tab_bar_active {
            spi::tab_bar::render_tab_bar(&ctx.tab_mgr, ctx.home_dir.as_ref());
        }

        let tab = ctx.tab_mgr.active_tab();
        let cwd = tab.virtual_cwd();
        let cwd_str = cwd.to_string_lossy().into_owned();
        let dcwd = display_cwd(&cwd_str, ctx.home_dir.as_ref());
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
        // TODO(backlog): Review lock scope - using block instead of explicit drop()
        // to satisfy clippy::await_holding_lock. Consider async-aware mutex if needed.
        let action = {
            let history_guard = ctx.history.lock().unwrap();
            ctx.editor.read_line(&prompt, &history_guard)?
        };

        // Handle tab-related editor actions
        match action {
            EditorAction::TabNew => {
                let cwd = ctx.tab_mgr.active_tab().virtual_cwd();
                match ctx.tab_mgr.create_shell_tab(cwd, ctx.policy.clone(), ctx.git_enforcer.clone()) {
                    Ok(idx) => {
                        ctx.tab_mgr.switch_to(idx);
                        println!("Switched to tab {}.", idx + 1);
                    }
                    Err(e) => error!(error = %e, "tab new: failed to create tab"),
                }
                continue;
            }
            EditorAction::TabClose => {
                if ctx.tab_mgr.close_active() {
                    break;
                }
                println!("Tab closed. Now on tab {}.", ctx.tab_mgr.active + 1);
                continue;
            }
            EditorAction::TabNext => {
                ctx.tab_mgr.switch_next();
                println!("Switched to tab {}.", ctx.tab_mgr.active + 1);
                continue;
            }
            EditorAction::TabPrev => {
                ctx.tab_mgr.switch_prev();
                println!("Switched to tab {}.", ctx.tab_mgr.active + 1);
                continue;
            }
            EditorAction::TabGoto(n) => {
                if n < ctx.tab_mgr.tabs.len() {
                    ctx.tab_mgr.switch_to(n);
                    println!("Switched to tab {}.", n + 1);
                } else {
                    println!("tab: no tab {}", n + 1);
                }
                continue;
            }
            EditorAction::Eof => {
                let tab = ctx.tab_mgr.active_tab_mut();
                if !tab.multiline_buffer.is_empty() {
                    tab.multiline_buffer.clear();
                    continue;
                }
                // Close current tab; exit if it was the last
                if ctx.tab_mgr.close_active() {
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

        let line = ctx.editor.line().to_string();

        let input = line.trim_end();

        // Check for tab completion request
        if ctx.rl_config.enable_completion && (input.ends_with("  ") || input.ends_with('\t')) {
            let completion_line = input.trim_end();
            let completions = ctx.completer.complete(completion_line, completion_line.len());

            if !completions.is_empty() {
                println!("\nCompletions:");
                for comp in &completions {
                    println!("  {}", comp.display);
                }
                println!();
                ctx.tab_mgr.active_tab_mut().multiline_buffer = completion_line.to_string();
                continue;
            }
        }

        // Add to multi-line buffer
        {
            let tab = ctx.tab_mgr.active_tab_mut();
            if !tab.multiline_buffer.is_empty() {
                tab.multiline_buffer.push('\n');
            }
            tab.multiline_buffer.push_str(input);
        }

        // Check if command is complete
        // Skip validation for AI mode - natural language doesn't need shell quote validation
        let tab = ctx.tab_mgr.active_tab();
        let skip_validation = tab.ai_mode || matches!(tab.kind(), TabKind::Ai);
        if !skip_validation {
            let validator = Validator::new();
            if validator.validate(&ctx.tab_mgr.active_tab().multiline_buffer) == ValidationResult::Incomplete
            {
                continue;
            }
        }

        // Command is complete, process it
        let cmd_with_leading_space;
        let cmd;
        {
            let tab = ctx.tab_mgr.active_tab_mut();
            cmd_with_leading_space = tab.multiline_buffer.trim_end().to_string();
            cmd = tab.multiline_buffer.trim().to_string();
            tab.multiline_buffer.clear();
        }

        if cmd.is_empty() {
            continue;
        }

        // Create span for this command execution (MDC context)
        cmd_count += 1;
        // Update MDC context with current turn (propagates to spawned async tasks)
        set_conversation_turn(cmd_count as u32);

        let (current_agent_id, tab_kind) = {
            let tab = ctx.tab_mgr.active_tab();
            let agent = if tab.ai_mode || matches!(tab.kind(), TabKind::Ai) {
                tab.ai_agent_id.clone()
            } else {
                "shell".to_string()
            };
            (agent, tab.kind())
        };
        // Update MDC agent_id (propagates to spawned async tasks)
        set_agent_id(&current_agent_id);

        let cmd_span = info_span!(
            "cmd",
            conversation_turn = cmd_count,
            tab = ctx.tab_mgr.active + 1,
            kind = ?tab_kind,
            agent_id = %current_agent_id,
        );
        let _cmd_guard = cmd_span.enter();
        debug!(cmd = %cmd, "executing");

        // Handle exit/quit: close mode tabs, exit AI mode, or close shell tab
        if cmd == "exit" || cmd == "quit" {
            let tab = ctx.tab_mgr.active_tab();
            let kind = tab.kind();
            match kind {
                TabKind::Ai | TabKind::HistoryView => {
                    // Mode tabs: close the tab on exit/quit
                    if ctx.tab_mgr.close_active() {
                        break;
                    }
                    println!("Tab closed. Now on tab {}.", ctx.tab_mgr.active + 1);
                    continue;
                }
                TabKind::Shell => {
                    let tab = ctx.tab_mgr.active_tab_mut();
                    if tab.ai_mode {
                        tab.ai_mode = false;
                        println!("Exited AI mode.");
                        continue;
                    }
                    // Close shell tab; exit if it was the last
                    if ctx.tab_mgr.close_active() {
                        break;
                    }
                    continue;
                }
            }
        }

        // Re-run setup wizard on demand
        if cmd == "setup" {
            let _ = spi::setup::run_setup_wizard(&mut ctx.config);
            continue;
        }

        // For HistoryView tabs: 'q' also closes the tab
        if cmd == "q" && ctx.tab_mgr.active_tab().kind() == TabKind::HistoryView {
            if ctx.tab_mgr.close_active() {
                break;
            }
            println!("Tab closed. Now on tab {}.", ctx.tab_mgr.active + 1);
            continue;
        }

        // Add to shared history after checking for exit
        {
            let mut h = ctx.history.lock().unwrap();
            h.add(cmd_with_leading_space);
        }

        // --- Tab commands (intercepted before AI/WASM dispatch) ---
        if let Some(action) = parse_tab_command(&cmd) {
            handle_tab_command(&mut ctx.tab_mgr, action, &ctx.policy, &ctx.ai_service, &ctx.git_enforcer).await;
            continue;
        }

        // --- Dispatch based on tab kind ---
        let kind = ctx.tab_mgr.active_tab().kind();
        match kind {
            TabKind::HistoryView => {
                // In history tab: typed text searches history, Enter copies to clipboard
                process_history_view(&ctx.tab_mgr, &cmd);
                continue;
            }
            TabKind::Ai => {
                // AI tabs are always in AI mode
                process_ai_mode(&mut ctx.tab_mgr, &ctx.ai_service, &cmd).await;
                continue;
            }
            TabKind::Shell => {
                // Fall through to shell processing
            }
        }

        // Handle commands based on current mode (shell tabs only)
        let tab_ai_mode = ctx.tab_mgr.active_tab().ai_mode;

        if tab_ai_mode {
            process_ai_mode(
                &mut ctx.tab_mgr,
                &ctx.ai_service,
                &cmd,
            )
            .await;
            continue;
        }

        // In shell mode: check for AI command triggers
        {
            let tab = ctx.tab_mgr.active_tab();
            if let Some(ai_cmd) = commands::parse_ai_command(&cmd) {
                let recent = tab.recent_commands.clone();
                let enter_ai_mode =
                    handle_ai_command(&ctx.ai_service, ai_cmd, &recent).await;
                if enter_ai_mode {
                    let tab = ctx.tab_mgr.active_tab_mut();
                    tab.ai_mode = true;
                    if let Some(svc) = &ctx.ai_service {
                        tab.ai_agent_id = svc.active_agent_id().await;
                    }
                    let model_info = ctx.config.ai.effective_model()
                        .map(|m| format!(" (model: {})", m))
                        .unwrap_or_default();
                    println!("Entered AI mode{model_info}. Type 'exit' or 'quit' to return to shell.");
                }
                continue;
            }
        }

        // Track recent commands for AI context
        {
            let tab = ctx.tab_mgr.active_tab_mut();
            tab.recent_commands.push(cmd.clone());
            if tab.recent_commands.len() > MAX_RECENT {
                tab.recent_commands.remove(0);
            }
        }

        // Dispatch to WASM engine (shell tabs only — type-level guarantee)
        {
            let wasm_span = info_span!("wasm_eval");
            let _wasm_guard = wasm_span.enter();

            let tab = ctx.tab_mgr.active_tab_mut();
            let TabInner::Shell(ref mut wasm) = tab.inner else {
                unreachable!("only Shell tabs reach WASM dispatch");
            };
            let cmd_bytes = cmd.as_bytes();
            if cmd_bytes.len() > wasm.buf_cap {
                warn!(
                    size = cmd_bytes.len(),
                    max = wasm.buf_cap,
                    "command too long, ignoring"
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
#[instrument(skip(tab_mgr, ai_service), fields(cmd_len = cmd.len()))]
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
            // Update sandbox cwd to match shell's virtual_cwd before AI processes input
            // This ensures relative paths in AI tool args are resolved correctly
            if let Some(svc) = ai_service {
                let cwd = tab_mgr.active_tab().virtual_cwd();
                svc.set_sandbox_cwd(cwd);
            }

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
#[derive(Debug)]
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
            let path = args.get(1).map(PathBuf::from);
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
#[instrument(skip_all, fields(action = ?action))]
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
                        error!(path = %canonical.display(), "tab new: path is not a directory");
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
                Err(e) => error!(error = %e, "tab new: failed to create tab"),
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
