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

/// Step 1: Detect or initialize a git repository.
fn step_git_repo() -> Result<(), ()> {
    println!();
    println!("\x1b[1;33m── Step 1/5: Git Repository ──\x1b[0m");

    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output();

    match output {
        Ok(ref o) if o.status.success() => {
            let root = Command::new("git")
                .args(["rev-parse", "--show-toplevel"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .unwrap_or_default();
            println!(
                "  \x1b[32m✓\x1b[0m Local:  {}",
                root.trim()
            );

            // Show remote URL if available
            let remote = Command::new("git")
                .args(["remote", "get-url", "origin"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                });
            if let Some(url) = remote {
                println!("  \x1b[32m✓\x1b[0m Remote: {}", url);
            } else {
                println!("  \x1b[33m!\x1b[0m Remote: (none configured)");
            }
            Ok(())
        }
        _ => {
            println!("  \x1b[33m!\x1b[0m No git repository found in the current directory.");
            if prompt_yn("  Initialize a new git repository here?", true)? {
                let status = Command::new("git").arg("init").status();
                match status {
                    Ok(s) if s.success() => {
                        println!("  \x1b[32m✓\x1b[0m Git repository initialized.");
                        Ok(())
                    }
                    _ => {
                        eprintln!("  \x1b[31m✗\x1b[0m Failed to initialize git repository.");
                        Err(())
                    }
                }
            } else {
                println!("  Skipping git init. Some features may be unavailable.");
                Ok(())
            }
        }
    }
}

/// Step 2: Detect or set the user ID.
fn step_user_id() -> Result<String, ()> {
    println!();
    println!("\x1b[1;33m── Step 2/5: User Identity ──\x1b[0m");

    let detected = Command::new("git")
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

    let default_id = detected.unwrap_or_default();
    if !default_id.is_empty() {
        println!(
            "  \x1b[32m✓\x1b[0m Detected git user: \x1b[1m{}\x1b[0m",
            default_id
        );
    }

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
    println!("\x1b[1;33m── Step 3/5: Branch Pipeline ──\x1b[0m");

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
    println!("\x1b[1;33m── Step 4/5: Safety Gates ──\x1b[0m");
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
    println!("\x1b[1;33m── Step 5/5: Create Branches ──\x1b[0m");
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

/// Print a summary of the completed setup.
fn print_setup_summary(config: &GitConfig) {
    println!();
    println!("\x1b[1;36m╔══════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║         Configuration Summary        ║\x1b[0m");
    println!("\x1b[1;36m╚══════════════════════════════════════╝\x1b[0m");
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
    println!("  This wizard will configure your git branch pipeline and safety gates.");
    println!("  Type \x1b[1mskip\x1b[0m at any prompt to skip the wizard.");
    println!();

    if !prompt_yn("Continue with setup?", true)? {
        println!("  Setup skipped.");
        config.setup_completed = true;
        let _ = super::config::save_config(config);
        return Ok(());
    }

    // Step 1: Git repo
    step_git_repo()?;

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

    // Save to global config
    config.git = Some(git_config.clone());
    config.setup_completed = true;

    match super::config::save_config(config) {
        Ok(()) => println!(
            "\n  \x1b[32m✓\x1b[0m Saved global config: ~/.config/swebash/config.toml"
        ),
        Err(e) => eprintln!("\n  \x1b[31m✗\x1b[0m Failed to save global config: {e}"),
    }

    // Save per-repo config
    save_repo_config(&git_config);

    // Print summary
    print_setup_summary(&git_config);

    println!();
    println!("\x1b[1;32m  Setup complete!\x1b[0m You can re-run this wizard with the \x1b[1msetup\x1b[0m command.");
    println!();

    Ok(())
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
}
