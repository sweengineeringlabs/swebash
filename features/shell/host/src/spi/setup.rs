use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;

use super::config::SwebashConfig;
use super::git_config::{BranchGate, BranchPipeline, GateAction, GitConfig};

// ── Table formatting ────────────────────────────────────────────────────────

/// Column definition for table formatting.
struct TableColumn {
    header: &'static str,
    width: usize,
}

/// Calculate visible length of a string, excluding ANSI escape codes.
fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            len += 1;
        }
    }
    len
}

/// Pad a string (possibly with ANSI codes) to a visible width.
fn pad_to_width(s: &str, width: usize) -> String {
    let visible = visible_len(s);
    if visible >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - visible))
    }
}

/// Print a formatted table with headers, separator, and alternating row colors.
fn print_table(columns: &[TableColumn], rows: &[Vec<String>]) {
    // Print header
    print!("  ");
    for col in columns {
        print!("{:<width$}  ", col.header, width = col.width);
    }
    println!();

    // Print separator
    print!("  ");
    for col in columns {
        print!("{}  ", "─".repeat(col.width));
    }
    println!();

    // Print rows with alternating colors
    for (i, row) in rows.iter().enumerate() {
        // Odd rows get dim background - use full-width background
        let (bg_start, bg_end) = if i % 2 == 1 {
            ("\x1b[48;5;236m", "\x1b[0m")
        } else {
            ("", "")
        };

        print!("{bg_start}  ");
        for (j, col) in columns.iter().enumerate() {
            let cell = row.get(j).map(|s| s.as_str()).unwrap_or("");
            // Use pad_to_width for proper alignment with ANSI codes
            print!("{}  ", pad_to_width(cell, col.width));
        }
        // Pad to total width for consistent background
        println!("{bg_end}");
    }
}

/// Format a GateAction as a colored string.
fn format_gate_action(action: GateAction) -> String {
    match action {
        GateAction::Allow => "\x1b[32mallow\x1b[0m".to_string(),
        GateAction::BlockWithOverride => "\x1b[33mblock_with_override\x1b[0m".to_string(),
        GateAction::Deny => "\x1b[31mdeny\x1b[0m".to_string(),
    }
}

// ── Prompt helpers ──────────────────────────────────────────────────────────

/// Print a yes/no prompt and return the boolean answer.
/// Defaults to `default` when the user presses Enter with no input.
fn prompt_yn(msg: &str, default: bool) -> Result<bool, ()> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("{msg} {hint} ");
    io::stdout().flush().unwrap_or(());

    let answer = read_line_or_skip()?;
    let trimmed = answer.trim().to_lowercase();
    if trimmed.is_empty() {
        Ok(default)
    } else {
        Ok(trimmed.starts_with('y'))
    }
}

/// Print a prompt and read a single line of text.
/// Returns `Err(())` if the user types "skip" or sends EOF.
/// Supports "help" or "?" to show help text if provided.
fn prompt_line(msg: &str, default: &str) -> Result<String, ()> {
    prompt_line_with_help(msg, default, None)
}

/// Print a prompt with optional help text.
fn prompt_line_with_help(msg: &str, default: &str, help: Option<&str>) -> Result<String, ()> {
    loop {
        if default.is_empty() {
            print!("  {msg}: ");
        } else {
            print!("  {msg} [\x1b[90m{default}\x1b[0m]: ");
        }
        io::stdout().flush().unwrap_or(());

        let answer = read_line_or_skip()?;
        let trimmed = answer.trim();

        // Handle help request
        if trimmed.eq_ignore_ascii_case("help") || trimmed == "?" {
            if let Some(help_text) = help {
                println!();
                println!("\x1b[36m  ℹ {help_text}\x1b[0m");
                println!();
                continue;
            } else {
                println!("  \x1b[90mNo help available for this prompt.\x1b[0m");
                continue;
            }
        }

        if trimmed.is_empty() {
            return Ok(default.to_string());
        } else {
            return Ok(trimmed.to_string());
        }
    }
}

/// Print a numbered list of choices and return the selected index (0-based).
fn prompt_choice(msg: &str, choices: &[&str], default: usize) -> Result<usize, ()> {
    println!("  {msg}");
    for (i, choice) in choices.iter().enumerate() {
        let marker = if i == default { ">" } else { " " };
        println!("    {marker} {}: {choice}", i + 1);
    }
    print!("    Choice [{}]: ", default + 1);
    io::stdout().flush().unwrap_or(());

    let answer = read_line_or_skip()?;
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    match trimmed.parse::<usize>() {
        Ok(n) if n >= 1 && n <= choices.len() => Ok(n - 1),
        _ => {
            println!("    Invalid choice, using default.");
            Ok(default)
        }
    }
}

/// Read one line from stdin, returning `Err(())` on EOF or if the user types
/// "skip".
fn read_line_or_skip() -> Result<String, ()> {
    let stdin = io::stdin();
    let mut line = String::new();
    match stdin.lock().read_line(&mut line) {
        Ok(0) => Err(()),     // EOF
        Err(_) => Err(()),    // I/O error
        Ok(_) => {
            if line.trim().eq_ignore_ascii_case("skip") {
                Err(())
            } else {
                Ok(line)
            }
        }
    }
}

// ── Wizard steps ────────────────────────────────────────────────────────────

/// Information about a detected git repository.
#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub local_path: String,
    pub remote_url: Option<String>,
}

/// Information about a GitHub account from `gh auth status`.
#[derive(Debug, Clone)]
struct GhAccount {
    username: String,
    is_active: bool,
}

/// Parse gh auth status text output into account list.
fn parse_gh_auth_output(text: &str) -> Vec<GhAccount> {
    let mut accounts = Vec::new();
    let mut current_username: Option<String> = None;

    for line in text.lines() {
        // Look for "Logged in to github.com account USERNAME"
        if line.contains("Logged in to") && line.contains("account ") {
            // Save the username, wait for Active account line
            current_username = line
                .split("account ")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .map(|s| s.to_string());
        }
        // "Active account: true/false" comes AFTER the account line
        if let Some(ref username) = current_username {
            if line.contains("Active account:") {
                let is_active = line.contains("true");
                accounts.push(GhAccount {
                    username: username.clone(),
                    is_active,
                });
                current_username = None;
            }
        }
    }

    accounts
}

/// Run `gh auth status` and parse all accounts.
fn get_gh_accounts() -> Vec<GhAccount> {
    let output = Command::new("gh")
        .args(["auth", "status"])
        .output()
        .ok();

    let Some(output) = output else {
        return vec![];
    };

    // gh auth status may write to stdout or stderr depending on version/platform
    // Try stdout first, fall back to stderr
    let stdout_text = String::from_utf8_lossy(&output.stdout);
    let stderr_text = String::from_utf8_lossy(&output.stderr);

    let accounts = parse_gh_auth_output(&stdout_text);
    if !accounts.is_empty() {
        return accounts;
    }

    parse_gh_auth_output(&stderr_text)
}

/// Check if a remote URL belongs to a GitHub account.
fn remote_matches_account(remote: &str, account: &str) -> bool {
    let remote_lower = remote.to_lowercase();
    let account_lower = account.to_lowercase();

    // SSH format: git@github.com:USER/repo.git
    if remote_lower.contains(&format!(":{}/", account_lower))
        || remote_lower.contains(&format!(":{account_lower}.")) {
        return true;
    }

    // HTTPS format: https://github.com/USER/repo.git
    if remote_lower.contains(&format!("/{}/", account_lower)) {
        return true;
    }

    false
}

/// Get repo info for a given path.
fn get_repo_info(path: &str) -> Option<RepoInfo> {
    let output = Command::new("git")
        .args(["-C", path, "rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let local_path = String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .to_string();

    let remote_url = Command::new("git")
        .args(["-C", path, "remote", "get-url", "origin"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    Some(RepoInfo { local_path, remote_url })
}

/// Step 1: Detect or initialize a git repository and bind workspace.
fn step_git_repo(config: &SwebashConfig) -> Result<RepoInfo, ()> {
    println!();
    println!("\x1b[1;33m── Step 1/6: Repository & Workspace Binding ──\x1b[0m");
    println!("  \x1b[90mWorkspaces are permanently bound to a repository to prevent accidental commits.\x1b[0m");
    println!();

    // First, show GitHub accounts
    let gh_accounts = get_gh_accounts();
    if !gh_accounts.is_empty() {
        println!("  GitHub Accounts (gh auth status):");
        println!();

        let columns = [
            TableColumn { header: "ACCOUNT", width: 25 },
            TableColumn { header: "STATUS", width: 15 },
        ];

        let rows: Vec<Vec<String>> = gh_accounts
            .iter()
            .map(|acc| {
                vec![
                    acc.username.clone(),
                    if acc.is_active {
                        "\x1b[32m● active\x1b[0m".to_string()
                    } else {
                        "\x1b[90m○ inactive\x1b[0m".to_string()
                    },
                ]
            })
            .collect();
        print_table(&columns, &rows);
        println!();

        // Check if there's an active account
        let active_account = gh_accounts.iter().find(|a| a.is_active);
        if active_account.is_none() && gh_accounts.len() > 1 {
            println!("  \x1b[33m!\x1b[0m No active GitHub account. Please select one:");
            println!();

            for (i, acc) in gh_accounts.iter().enumerate() {
                println!("    [{}] {}", i + 1, acc.username);
            }
            println!();

            loop {
                print!("  Select account [1-{}]: ", gh_accounts.len());
                io::stdout().flush().ok();

                let mut input = String::new();
                if io::stdin().lock().read_line(&mut input).is_err() {
                    return Err(());
                }

                let input = input.trim();
                if input.eq_ignore_ascii_case("skip") {
                    return Err(());
                }

                if let Ok(idx) = input.parse::<usize>() {
                    if idx >= 1 && idx <= gh_accounts.len() {
                        let selected = &gh_accounts[idx - 1];
                        println!();
                        println!("  Switching to account: {}", selected.username);

                        // Run gh auth switch
                        let switch_result = Command::new("gh")
                            .args(["auth", "switch", "-u", &selected.username])
                            .status();

                        match switch_result {
                            Ok(s) if s.success() => {
                                println!("  \x1b[32m✓\x1b[0m Switched to {}", selected.username);
                                println!();
                            }
                            _ => {
                                eprintln!("  \x1b[31m✗\x1b[0m Failed to switch account");
                                return Err(());
                            }
                        }
                        break;
                    }
                }
                println!("  \x1b[31mInvalid selection. Try again.\x1b[0m");
            }
        }
    }

    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_default();

    // Check if this workspace is already bound
    if let Some(bound) = config.find_workspace_for_path(&cwd) {
        println!("  \x1b[33m!\x1b[0m This workspace is already bound to a repository:");
        println!("    Workspace: {}", bound.workspace_path);
        println!("    Repo:      {}", bound.repo_remote);
        println!("    Bound at:  {}", bound.bound_at);
        println!();
        println!("  \x1b[90mWorkspace bindings cannot be changed. To use a different repo,");
        println!("  start swebash from a different directory.\x1b[0m");
        return Err(());
    }

    // Detect current repo
    let repo_info = get_repo_info(&cwd);

    match repo_info {
        Some(ref info) => {
            println!("  Detected repository:");
            println!();

            let columns = [
                TableColumn { header: "TYPE", width: 10 },
                TableColumn { header: "PATH/URL", width: 60 },
            ];

            let mut rows = vec![
                vec!["Local".to_string(), info.local_path.clone()],
                vec![
                    "Remote".to_string(),
                    info.remote_url.clone().unwrap_or_else(|| "\x1b[90m(none)\x1b[0m".to_string()),
                ],
            ];

            // Show which account owns this repo
            if let Some(ref remote) = info.remote_url {
                if let Some(owner) = gh_accounts.iter().find(|a| remote_matches_account(remote, &a.username)) {
                    rows.push(vec![
                        "Owner".to_string(),
                        format!(
                            "{}{}",
                            owner.username,
                            if owner.is_active { " \x1b[32m(active)\x1b[0m" } else { " \x1b[33m(not active)\x1b[0m" }
                        ),
                    ]);
                }
            }

            print_table(&columns, &rows);
            println!();

            // Check if remote is configured
            if info.remote_url.is_none() {
                println!("  \x1b[33m!\x1b[0m No remote configured. Workspace binding requires a remote URL.");
                println!("  \x1b[90mAdd a remote with: git remote add origin <url>\x1b[0m");
                return Err(());
            }

            // Warn if repo owner doesn't match active account
            if let Some(ref remote) = info.remote_url {
                let active = gh_accounts.iter().find(|a| a.is_active);
                if let Some(active_acc) = active {
                    if !remote_matches_account(remote, &active_acc.username) {
                        println!("  \x1b[33m!\x1b[0m Warning: This repo's remote doesn't match your active GitHub account.");
                        println!("  \x1b[90mActive account: {}\x1b[0m", active_acc.username);
                        println!("  \x1b[90mYou may want to run: gh auth switch -u <account>\x1b[0m");
                        println!();
                    }
                }
            }

            // Confirm binding
            println!("  \x1b[1mIMPORTANT:\x1b[0m Once bound, this workspace will ONLY allow commits to this repository.");
            println!("  This prevents accidental commits to the wrong repo.");
            println!();

            if !prompt_yn("  Bind this workspace to this repository?", true)? {
                println!("  \x1b[90mSetup cancelled. Run 'setup' again when ready.\x1b[0m");
                return Err(());
            }

            Ok(info.clone())
        }
        None => {
            println!("  \x1b[33m!\x1b[0m No git repository found in the current directory.");
            if prompt_yn("  Initialize a new git repository here?", true)? {
                let status = Command::new("git").arg("init").status();
                match status {
                    Ok(s) if s.success() => {
                        println!("  \x1b[32m✓\x1b[0m Git repository initialized.");
                        println!();
                        println!("  \x1b[33m!\x1b[0m You need to add a remote before binding.");
                        println!("  \x1b[90mRun: git remote add origin <url>\x1b[0m");
                        println!("  \x1b[90mThen run 'setup' again.\x1b[0m");
                        Err(())
                    }
                    _ => {
                        eprintln!("  \x1b[31m✗\x1b[0m Failed to initialize git repository.");
                        Err(())
                    }
                }
            } else {
                println!("  Skipping. Setup requires a git repository.");
                Err(())
            }
        }
    }
}

/// Step 2: Detect or set the user ID.
fn step_user_id() -> Result<String, ()> {
    println!();
    println!("\x1b[1;33m── Step 2/6: User Identity ──\x1b[0m");

    // Check git config user.name
    let git_user = Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        });

    // Check gh auth status for GitHub username
    let gh_user = Command::new("gh")
        .args(["auth", "status", "--active"])
        .output()
        .ok()
        .and_then(|o| {
            // Parse "Logged in to github.com account USERNAME" from output
            let output = String::from_utf8_lossy(&o.stderr);
            output
                .lines()
                .find(|l| l.contains("Logged in to"))
                .and_then(|line| {
                    // Extract username from "account USERNAME (keyring)"
                    line.split("account ")
                        .nth(1)
                        .and_then(|s| s.split_whitespace().next())
                        .map(|s| s.to_string())
                })
        });

    // Display detected identities
    let has_git = git_user.is_some();
    let has_gh = gh_user.is_some();

    if let Some(ref user) = git_user {
        println!("  \x1b[32m✓\x1b[0m Git user.name:  \x1b[1m{}\x1b[0m", user);
    }
    if let Some(ref user) = gh_user {
        println!("  \x1b[32m✓\x1b[0m GitHub account: \x1b[1m{}\x1b[0m", user);
    }
    if !has_git && !has_gh {
        println!("  \x1b[33m!\x1b[0m No git or GitHub identity detected.");
    }

    // Prefer GitHub account, fallback to git user.name
    let default_id = gh_user.or(git_user).unwrap_or_default();

    println!("  \x1b[90mType 'help' or '?' for more info\x1b[0m");
    let user_id = prompt_line_with_help(
        "User ID for branch naming",
        &default_id,
        Some("This ID is used as a prefix for your feature branches (e.g., 'alice/feature-xyz').\n  It helps identify branch ownership in team repos. Typically your name or username.")
    )?;
    if user_id.is_empty() {
        eprintln!("  \x1b[31m✗\x1b[0m User ID cannot be empty.");
        return Err(());
    }

    // Sanitize: replace spaces and special chars with underscores
    let sanitized: String = user_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if sanitized != user_id {
        println!(
            "  \x1b[90m(sanitized to: {})\x1b[0m",
            sanitized
        );
    }

    Ok(sanitized)
}

/// Step 3: Present and optionally customize the branch pipeline.
fn step_pipeline(user_id: &str) -> Result<BranchPipeline, ()> {
    println!();
    println!("\x1b[1;33m── Step 3/6: Branch Pipeline ──\x1b[0m");

    let default_pipeline = BranchPipeline::default_for_user(user_id);

    println!("  Default promotion pipeline:");
    for (i, branch) in default_pipeline.branches.iter().enumerate() {
        let prot = if branch.protected {
            " \x1b[31m[protected]\x1b[0m"
        } else {
            ""
        };
        let arrow = if i + 1 < default_pipeline.branches.len() {
            " →"
        } else {
            ""
        };
        println!(
            "    {}: \x1b[1m{}\x1b[0m ({}){prot}{arrow}",
            i + 1,
            branch.name,
            branch.role
        );
    }
    println!();

    if prompt_yn("Accept this pipeline?", true)? {
        return Ok(default_pipeline);
    }

    // Interactive customization — let users rename branches
    println!("  Enter custom branch names (press Enter to keep default):");
    let mut pipeline = default_pipeline;
    for branch in &mut pipeline.branches {
        let new_name = prompt_line(
            &format!("  {} ({})", branch.role, branch.name),
            &branch.name,
        )?;
        if !new_name.is_empty() {
            branch.name = new_name;
        }

        let is_protected = prompt_yn(
            &format!("  Mark '{}' as protected?", branch.name),
            branch.protected,
        )?;
        branch.protected = is_protected;
    }

    Ok(pipeline)
}

/// Step 4: Present and optionally customize the gate matrix.
fn step_gates(pipeline: &BranchPipeline) -> Result<Vec<BranchGate>, ()> {
    println!();
    println!("\x1b[1;33m── Step 4/6: Safety Gates ──\x1b[0m");
    println!("  \x1b[90mGates control what operations are allowed on each branch.\x1b[0m");
    println!();

    let default_gates = GitConfig::default_gates(pipeline);

    let columns = [
        TableColumn { header: "BRANCH", width: 18 },
        TableColumn { header: "COMMIT", width: 20 },
        TableColumn { header: "PUSH", width: 20 },
        TableColumn { header: "MERGE", width: 20 },
        TableColumn { header: "FORCE-PUSH", width: 20 },
    ];

    let rows: Vec<Vec<String>> = default_gates
        .iter()
        .map(|gate| {
            vec![
                gate.branch.clone(),
                format_gate_action(gate.can_commit),
                format_gate_action(gate.can_push),
                format_gate_action(gate.can_merge),
                format_gate_action(gate.can_force_push),
            ]
        })
        .collect();

    print_table(&columns, &rows);
    println!();

    if prompt_yn("  Accept these gate rules?", true)? {
        return Ok(default_gates);
    }

    // Interactive per-branch gate customization
    let action_choices = &["allow", "block_with_override", "deny"];
    let mut gates = default_gates;

    for gate in &mut gates {
        println!();
        println!("  \x1b[1m{}\x1b[0m:", gate.branch);

        let commit_idx = prompt_choice(
            &format!("    can_commit (current: {})", gate.can_commit),
            action_choices,
            gate_action_index(gate.can_commit),
        )?;
        gate.can_commit = index_to_gate_action(commit_idx);

        let push_idx = prompt_choice(
            &format!("    can_push (current: {})", gate.can_push),
            action_choices,
            gate_action_index(gate.can_push),
        )?;
        gate.can_push = index_to_gate_action(push_idx);

        let merge_idx = prompt_choice(
            &format!("    can_merge (current: {})", gate.can_merge),
            action_choices,
            gate_action_index(gate.can_merge),
        )?;
        gate.can_merge = index_to_gate_action(merge_idx);

        let force_push_idx = prompt_choice(
            &format!("    can_force_push (current: {})", gate.can_force_push),
            action_choices,
            gate_action_index(gate.can_force_push),
        )?;
        gate.can_force_push = index_to_gate_action(force_push_idx);
    }

    Ok(gates)
}

fn gate_action_index(action: GateAction) -> usize {
    match action {
        GateAction::Allow => 0,
        GateAction::BlockWithOverride => 1,
        GateAction::Deny => 2,
    }
}

fn index_to_gate_action(idx: usize) -> GateAction {
    match idx {
        0 => GateAction::Allow,
        1 => GateAction::BlockWithOverride,
        _ => GateAction::Deny,
    }
}

/// Check if a branch exists locally.
fn branch_exists_local(name: &str) -> bool {
    Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{name}")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if a branch exists on remote (origin).
fn branch_exists_remote(name: &str) -> bool {
    Command::new("git")
        .args(["ls-remote", "--exit-code", "--heads", "origin", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Format branch existence status as colored string.
fn format_exists(exists: bool) -> String {
    if exists {
        "\x1b[32m✓ exists\x1b[0m".to_string()
    } else {
        "\x1b[33m✗ missing\x1b[0m".to_string()
    }
}

/// Step 5: List existing vs missing branches and offer to create missing ones.
fn step_branches(pipeline: &BranchPipeline) -> Result<(), ()> {
    println!();
    println!("\x1b[1;33m── Step 5/6: Create Branches ──\x1b[0m");
    println!("  \x1b[90mThese branches are defined in your pipeline configuration.\x1b[0m");
    println!();

    // Check if we are inside a git repo
    let in_git = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !in_git {
        println!("  \x1b[90mNot in a git repository — skipping branch creation.\x1b[0m");
        return Ok(());
    }

    struct BranchStatus<'a> {
        name: &'a str,
        local: bool,
        remote: bool,
    }

    // Collect branch statuses first
    let mut statuses: Vec<BranchStatus> = Vec::new();
    for branch in &pipeline.branches {
        let local = branch_exists_local(&branch.name);
        let remote = branch_exists_remote(&branch.name);
        statuses.push(BranchStatus {
            name: &branch.name,
            local,
            remote,
        });
    }

    // Print table using common formatter
    let columns = [
        TableColumn { header: "BRANCH", width: 18 },
        TableColumn { header: "LOCAL", width: 12 },
        TableColumn { header: "REMOTE", width: 12 },
    ];

    let rows: Vec<Vec<String>> = statuses
        .iter()
        .map(|s| {
            vec![
                s.name.to_string(),
                format_exists(s.local),
                format_exists(s.remote),
            ]
        })
        .collect();

    print_table(&columns, &rows);

    // Collect branches missing locally
    let missing_local: Vec<&str> = statuses
        .iter()
        .filter(|s| !s.local)
        .map(|s| s.name)
        .collect();

    if missing_local.is_empty() {
        println!();
        println!("  \x1b[32m✓\x1b[0m All pipeline branches exist locally.");
        return Ok(());
    }

    println!();
    println!(
        "  {} branch(es) missing locally. Create them individually:",
        missing_local.len()
    );
    println!();

    for name in &missing_local {
        if prompt_yn(&format!("  Create '{name}' from current HEAD?"), true)? {
            let status = Command::new("git").args(["branch", name]).status();
            match status {
                Ok(s) if s.success() => {
                    println!("    \x1b[32m✓\x1b[0m Created branch: {name}");
                }
                _ => {
                    eprintln!("    \x1b[31m✗\x1b[0m Failed to create branch: {name}");
                }
            }
        } else {
            println!("    \x1b[90mSkipped: {name}\x1b[0m");
        }
    }

    Ok(())
}

// ── Config persistence ──────────────────────────────────────────────────────

/// Save git config to the per-repo `.swebash/git.toml` file.
fn save_repo_config(git_config: &GitConfig) {
    let repo_root = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    if let Some(root) = repo_root {
        let swebash_dir = PathBuf::from(&root).join(".swebash");
        if let Err(e) = std::fs::create_dir_all(&swebash_dir) {
            eprintln!(
                "  \x1b[31m✗\x1b[0m Could not create {}: {e}",
                swebash_dir.display()
            );
            return;
        }
        let git_toml_path = swebash_dir.join("git.toml");
        match toml::to_string_pretty(git_config) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&git_toml_path, content) {
                    eprintln!(
                        "  \x1b[31m✗\x1b[0m Failed to write {}: {e}",
                        git_toml_path.display()
                    );
                } else {
                    println!(
                        "  \x1b[32m✓\x1b[0m Saved per-repo config: {}",
                        git_toml_path.display()
                    );
                }
            }
            Err(e) => eprintln!("  \x1b[31m✗\x1b[0m Failed to serialize git config: {e}"),
        }
    }
}

// ── Main wizard entry point ─────────────────────────────────────────────────

/// Run the first-run setup wizard. Returns `Ok(())` if the wizard completed
/// (or was skipped), `Err(())` if it was aborted.
///
/// Mutates `config` in-place and saves it to `~/.config/swebash/config.toml`.
pub fn run_setup_wizard(config: &mut SwebashConfig) -> Result<(), ()> {
    println!();
    println!("\x1b[1;36m╔══════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║   swebash — First-Run Setup Wizard   ║\x1b[0m");
    println!("\x1b[1;36m╚══════════════════════════════════════╝\x1b[0m");
    println!();
    println!("  This wizard will bind this workspace to a git repository and configure");
    println!("  your branch pipeline and safety gates.");
    println!("  Type \x1b[1mskip\x1b[0m at any prompt to skip the wizard.");
    println!();

    if !prompt_yn("Continue with setup?", true)? {
        println!("  Setup skipped.");
        config.setup_completed = true;
        let _ = super::config::save_config(config);
        return Ok(());
    }

    // Step 1: Git repo and workspace binding
    let repo_info = step_git_repo(config)?;

    // Step 2: User ID
    let user_id = step_user_id()?;

    // Step 3: Pipeline
    let pipeline = step_pipeline(&user_id)?;

    // Step 4: Gates
    let gates = step_gates(&pipeline)?;

    // Step 5: Branches
    step_branches(&pipeline)?;

    // Build GitConfig
    let git_config = GitConfig {
        user_id,
        pipeline,
        gates,
    };

    // Create workspace binding
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_default();

    let bound_workspace = super::config::BoundWorkspace {
        workspace_path: cwd,
        repo_remote: repo_info.remote_url.clone().unwrap_or_default(),
        repo_local: repo_info.local_path.clone(),
        bound_at: chrono_lite_now(),
        git: Some(git_config.clone()),
    };

    // Add to bound workspaces
    config.bound_workspaces.push(bound_workspace);
    config.setup_completed = true;

    // Also set legacy git field for backwards compatibility
    config.git = Some(git_config.clone());

    match super::config::save_config(config) {
        Ok(()) => println!(
            "\n  \x1b[32m✓\x1b[0m Saved config: ~/.config/swebash/config.toml"
        ),
        Err(e) => eprintln!("\n  \x1b[31m✗\x1b[0m Failed to save config: {e}"),
    }

    // Save per-repo config
    save_repo_config(&git_config);

    // Print summary
    print_setup_summary_with_binding(&git_config, &repo_info);

    println!();
    println!("\x1b[1;32m  Setup complete!\x1b[0m You can re-run this wizard with the \x1b[1msetup\x1b[0m command.");
    println!();

    Ok(())
}

/// Get current timestamp in ISO 8601 format (simplified, no external deps).
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO 8601 format: YYYY-MM-DDTHH:MM:SSZ
    // This is a rough approximation without full date math
    let days = now / 86400;
    let years = 1970 + days / 365;
    let remaining_days = days % 365;
    let months = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    let hours = (now % 86400) / 3600;
    let minutes = (now % 3600) / 60;
    let seconds = now % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        years, months, day, hours, minutes, seconds
    )
}

/// Print summary including workspace binding.
fn print_setup_summary_with_binding(config: &GitConfig, repo: &RepoInfo) {
    println!();
    println!("\x1b[1;36m╔══════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║         Configuration Summary        ║\x1b[0m");
    println!("\x1b[1;36m╚══════════════════════════════════════╝\x1b[0m");
    println!();

    // Workspace binding
    println!("  \x1b[1mWorkspace Binding:\x1b[0m");
    println!("    Repo:   {}", repo.remote_url.as_deref().unwrap_or("(none)"));
    println!("    Local:  {}", repo.local_path);
    println!("    \x1b[90mCommits to other repos will be blocked.\x1b[0m");
    println!();

    // User ID
    println!("  \x1b[1mUser ID:\x1b[0m {}", config.user_id);
    println!();

    // Pipeline branches
    println!("  \x1b[1mBranch Pipeline:\x1b[0m");
    let branch_columns = [
        TableColumn { header: "BRANCH", width: 18 },
        TableColumn { header: "PROTECTED", width: 10 },
    ];
    let branch_rows: Vec<Vec<String>> = config
        .pipeline
        .branches
        .iter()
        .map(|b| {
            vec![
                b.name.clone(),
                if b.protected {
                    "\x1b[33myes\x1b[0m".to_string()
                } else {
                    "\x1b[90mno\x1b[0m".to_string()
                },
            ]
        })
        .collect();
    print_table(&branch_columns, &branch_rows);
    println!();

    // Gates
    println!("  \x1b[1mSafety Gates:\x1b[0m");
    let gate_columns = [
        TableColumn { header: "BRANCH", width: 18 },
        TableColumn { header: "COMMIT", width: 20 },
        TableColumn { header: "PUSH", width: 20 },
        TableColumn { header: "MERGE", width: 20 },
        TableColumn { header: "FORCE-PUSH", width: 20 },
    ];
    let gate_rows: Vec<Vec<String>> = config
        .gates
        .iter()
        .map(|g| {
            vec![
                g.branch.clone(),
                format_gate_action(g.can_commit),
                format_gate_action(g.can_push),
                format_gate_action(g.can_merge),
                format_gate_action(g.can_force_push),
            ]
        })
        .collect();
    print_table(&gate_columns, &gate_rows);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Table formatting tests ──────────────────────────────────────────────

    #[test]
    fn table_column_has_header_and_width() {
        let col = TableColumn {
            header: "NAME",
            width: 20,
        };
        assert_eq!(col.header, "NAME");
        assert_eq!(col.width, 20);
    }

    #[test]
    fn format_gate_action_allow_is_green() {
        let result = format_gate_action(GateAction::Allow);
        assert!(result.contains("allow"));
        assert!(result.contains("\x1b[32m")); // green
    }

    #[test]
    fn format_gate_action_block_is_yellow() {
        let result = format_gate_action(GateAction::BlockWithOverride);
        assert!(result.contains("block_with_override"));
        assert!(result.contains("\x1b[33m")); // yellow
    }

    #[test]
    fn format_gate_action_deny_is_red() {
        let result = format_gate_action(GateAction::Deny);
        assert!(result.contains("deny"));
        assert!(result.contains("\x1b[31m")); // red
    }

    #[test]
    fn format_exists_true_shows_checkmark() {
        let result = format_exists(true);
        assert!(result.contains("✓"));
        assert!(result.contains("exists"));
        assert!(result.contains("\x1b[32m")); // green
    }

    #[test]
    fn format_exists_false_shows_x() {
        let result = format_exists(false);
        assert!(result.contains("✗"));
        assert!(result.contains("missing"));
        assert!(result.contains("\x1b[33m")); // yellow
    }

    // ── Gate action index conversion tests ──────────────────────────────────

    #[test]
    fn gate_action_index_allow_is_0() {
        assert_eq!(gate_action_index(GateAction::Allow), 0);
    }

    #[test]
    fn gate_action_index_block_is_1() {
        assert_eq!(gate_action_index(GateAction::BlockWithOverride), 1);
    }

    #[test]
    fn gate_action_index_deny_is_2() {
        assert_eq!(gate_action_index(GateAction::Deny), 2);
    }

    #[test]
    fn index_to_gate_action_0_is_allow() {
        assert_eq!(index_to_gate_action(0), GateAction::Allow);
    }

    #[test]
    fn index_to_gate_action_1_is_block() {
        assert_eq!(index_to_gate_action(1), GateAction::BlockWithOverride);
    }

    #[test]
    fn index_to_gate_action_2_is_deny() {
        assert_eq!(index_to_gate_action(2), GateAction::Deny);
    }

    #[test]
    fn index_to_gate_action_invalid_defaults_to_deny() {
        assert_eq!(index_to_gate_action(99), GateAction::Deny);
    }

    // ── Print table output tests ────────────────────────────────────────────

    #[test]
    fn print_table_does_not_panic_with_empty_rows() {
        let columns = [
            TableColumn { header: "A", width: 5 },
            TableColumn { header: "B", width: 5 },
        ];
        let rows: Vec<Vec<String>> = vec![];
        // Should not panic
        print_table(&columns, &rows);
    }

    #[test]
    fn print_table_does_not_panic_with_single_row() {
        let columns = [
            TableColumn { header: "COL1", width: 10 },
            TableColumn { header: "COL2", width: 10 },
        ];
        let rows = vec![vec!["val1".to_string(), "val2".to_string()]];
        // Should not panic
        print_table(&columns, &rows);
    }

    #[test]
    fn print_table_handles_multiple_rows() {
        let columns = [
            TableColumn { header: "NAME", width: 15 },
            TableColumn { header: "STATUS", width: 10 },
        ];
        let rows = vec![
            vec!["alpha".to_string(), "ok".to_string()],
            vec!["beta".to_string(), "fail".to_string()],
            vec!["gamma".to_string(), "ok".to_string()],
        ];
        // Should not panic, rows 1 and 3 should have different background
        print_table(&columns, &rows);
    }

    #[test]
    fn print_table_handles_missing_cells() {
        let columns = [
            TableColumn { header: "A", width: 5 },
            TableColumn { header: "B", width: 5 },
            TableColumn { header: "C", width: 5 },
        ];
        // Row has fewer cells than columns
        let rows = vec![vec!["x".to_string()]];
        // Should not panic, missing cells should be empty
        print_table(&columns, &rows);
    }

    // ── ANSI-aware string length tests ──────────────────────────────────────

    #[test]
    fn visible_len_plain_text() {
        assert_eq!(visible_len("hello"), 5);
        assert_eq!(visible_len(""), 0);
        assert_eq!(visible_len("a"), 1);
    }

    #[test]
    fn visible_len_with_ansi_codes() {
        // Green "allow" - escape codes should not count
        let green_allow = "\x1b[32mallow\x1b[0m";
        assert_eq!(visible_len(green_allow), 5); // just "allow"
    }

    #[test]
    fn visible_len_with_multiple_ansi_codes() {
        // Yellow "block_with_override"
        let yellow_block = "\x1b[33mblock_with_override\x1b[0m";
        assert_eq!(visible_len(yellow_block), 19); // just "block_with_override"
    }

    #[test]
    fn visible_len_mixed_content() {
        // "prefix" + colored "middle" + "suffix"
        let mixed = "prefix\x1b[31mmiddle\x1b[0msuffix";
        assert_eq!(visible_len(mixed), 18); // "prefix" + "middle" + "suffix"
    }

    #[test]
    fn pad_to_width_plain_text() {
        let padded = pad_to_width("hi", 5);
        assert_eq!(padded, "hi   ");
    }

    #[test]
    fn pad_to_width_with_ansi_codes() {
        let green = "\x1b[32mok\x1b[0m";
        let padded = pad_to_width(green, 5);
        // Should add 3 spaces after the reset code
        assert_eq!(visible_len(&padded), 5);
        assert!(padded.ends_with("   ")); // 3 trailing spaces
    }

    #[test]
    fn pad_to_width_already_wide_enough() {
        let text = "hello";
        let padded = pad_to_width(text, 3);
        // No padding needed, return as-is
        assert_eq!(padded, "hello");
    }

    #[test]
    fn pad_to_width_exact_width() {
        let text = "exact";
        let padded = pad_to_width(text, 5);
        assert_eq!(padded, "exact");
    }

    // ── GitHub account matching tests ──────────────────────────────────────────

    #[test]
    fn remote_matches_account_ssh_format() {
        let remote = "git@github.com:sweengineeringlabs/swebash.git";
        assert!(remote_matches_account(remote, "sweengineeringlabs"));
        assert!(!remote_matches_account(remote, "phdsystems"));
    }

    #[test]
    fn remote_matches_account_https_format() {
        let remote = "https://github.com/sweengineeringlabs/swebash.git";
        assert!(remote_matches_account(remote, "sweengineeringlabs"));
        assert!(!remote_matches_account(remote, "phdsystems"));
    }

    #[test]
    fn remote_matches_account_case_insensitive() {
        let remote = "git@github.com:SweEngineeringLabs/swebash.git";
        assert!(remote_matches_account(remote, "sweengineeringlabs"));
        assert!(remote_matches_account(remote, "SWEENGINEERINGLABS"));
    }

    #[test]
    fn remote_matches_account_different_repo() {
        let remote1 = "git@github.com:alice/repo.git";
        let remote2 = "git@github.com:bob/repo.git";
        assert!(remote_matches_account(remote1, "alice"));
        assert!(!remote_matches_account(remote1, "bob"));
        assert!(remote_matches_account(remote2, "bob"));
    }

    #[test]
    fn gh_account_struct_fields() {
        let account = GhAccount {
            username: "testuser".to_string(),
            is_active: true,
        };
        assert_eq!(account.username, "testuser");
        assert!(account.is_active);
    }

    #[test]
    fn parse_gh_auth_status_output() {
        // Actual gh auth status output format
        let output = r#"github.com
  ✓ Logged in to github.com account sweengineeringlabs (keyring)
  - Active account: true
  - Git operations protocol: ssh
  - Token: gho_************************************
  - Token scopes: 'admin:public_key', 'gist', 'read:org', 'repo'

  ✓ Logged in to github.com account phdsystems (keyring)
  - Active account: false
  - Git operations protocol: ssh
  - Token: gho_************************************
  - Token scopes: 'gist', 'read:org', 'repo'"#;

        let accounts = parse_gh_auth_output(output);

        assert_eq!(accounts.len(), 2, "Should find 2 accounts");
        assert_eq!(accounts[0].username, "sweengineeringlabs");
        assert!(accounts[0].is_active, "sweengineeringlabs should be active");
        assert_eq!(accounts[1].username, "phdsystems");
        assert!(!accounts[1].is_active, "phdsystems should be inactive");
    }

    #[test]
    fn parse_gh_auth_status_single_account() {
        let output = r#"github.com
  ✓ Logged in to github.com account myuser (keyring)
  - Active account: true
  - Git operations protocol: https"#;

        let accounts = parse_gh_auth_output(output);

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].username, "myuser");
        assert!(accounts[0].is_active);
    }

    #[test]
    fn parse_gh_auth_status_no_accounts() {
        let output = "You are not logged in to any GitHub hosts.";
        let accounts = parse_gh_auth_output(output);
        assert!(accounts.is_empty());
    }
}
