use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;

use super::config::SwebashConfig;
use super::git_config::{BranchGate, BranchPipeline, GateAction, GitConfig};

// ── Prompt helpers ──────────────────────────────────────────────────────────

/// Print a yes/no prompt and return the boolean answer.
/// Defaults to `default` when the user presses Enter with no input.
fn prompt_yn(msg: &str, default: bool) -> Result<bool, ()> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("\x1b[1;36m?\x1b[0m {msg} {hint} ");
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
fn prompt_line(msg: &str, default: &str) -> Result<String, ()> {
    if default.is_empty() {
        print!("\x1b[1;36m?\x1b[0m {msg}: ");
    } else {
        print!("\x1b[1;36m?\x1b[0m {msg} [\x1b[90m{default}\x1b[0m]: ");
    }
    io::stdout().flush().unwrap_or(());

    let answer = read_line_or_skip()?;
    let trimmed = answer.trim().to_string();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed)
    }
}

/// Print a numbered list of choices and return the selected index (0-based).
fn prompt_choice(msg: &str, choices: &[&str], default: usize) -> Result<usize, ()> {
    println!("\x1b[1;36m?\x1b[0m {msg}");
    for (i, choice) in choices.iter().enumerate() {
        let marker = if i == default { ">" } else { " " };
        println!("  {marker} {}: {choice}", i + 1);
    }
    print!("  Choice [{}]: ", default + 1);
    io::stdout().flush().unwrap_or(());

    let answer = read_line_or_skip()?;
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    match trimmed.parse::<usize>() {
        Ok(n) if n >= 1 && n <= choices.len() => Ok(n - 1),
        _ => {
            println!("  Invalid choice, using default.");
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
                "  \x1b[32m✓\x1b[0m Git repository detected at: {}",
                root.trim()
            );
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

    let user_id = prompt_line("User ID for branch naming", &default_id)?;
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

    let default_gates = GitConfig::default_gates(pipeline);

    println!(
        "  {:20} {:20} {:20} {:20} {:20}",
        "Branch", "Commit", "Push", "Merge", "Force-Push"
    );
    println!("  {}", "─".repeat(100));
    for gate in &default_gates {
        println!(
            "  {:20} {:20} {:20} {:20} {:20}",
            gate.branch, gate.can_commit, gate.can_push, gate.can_merge, gate.can_force_push
        );
    }
    println!();

    if prompt_yn("Accept these gate rules?", true)? {
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

/// Step 5: List existing vs missing branches and offer to create missing ones.
fn step_branches(pipeline: &BranchPipeline) -> Result<(), ()> {
    println!();
    println!("\x1b[1;33m── Step 5/5: Create Branches ──\x1b[0m");

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

    // Get list of existing branches
    let existing_output = Command::new("git")
        .args(["branch", "--list", "--format=%(refname:short)"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_default();

    let existing: Vec<&str> = existing_output.lines().map(|l| l.trim()).collect();

    let mut missing: Vec<&str> = Vec::new();
    for branch in &pipeline.branches {
        if existing.iter().any(|&e| e == branch.name) {
            println!(
                "  \x1b[32m✓\x1b[0m {} \x1b[90m(exists)\x1b[0m",
                branch.name
            );
        } else {
            println!("  \x1b[33m!\x1b[0m {} \x1b[90m(missing)\x1b[0m", branch.name);
            missing.push(&branch.name);
        }
    }

    if missing.is_empty() {
        println!("  All pipeline branches exist.");
        return Ok(());
    }

    println!();
    if !prompt_yn(
        &format!(
            "Create {} missing branch(es) from current HEAD?",
            missing.len()
        ),
        true,
    )? {
        println!("  Skipping branch creation.");
        return Ok(());
    }

    for name in &missing {
        let status = Command::new("git")
            .args(["branch", name])
            .status();
        match status {
            Ok(s) if s.success() => {
                println!("  \x1b[32m✓\x1b[0m Created branch: {name}");
            }
            _ => {
                eprintln!("  \x1b[31m✗\x1b[0m Failed to create branch: {name}");
            }
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

    println!();
    println!("\x1b[1;32m  Setup complete!\x1b[0m You can re-run this wizard with the \x1b[1msetup\x1b[0m command.");
    println!();

    Ok(())
}
