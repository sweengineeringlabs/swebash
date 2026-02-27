use std::path::Path;
use std::process::Command;

use super::git_config::{BranchGate, GateAction, GitConfig};

/// Result of a gate check for a git operation.
#[derive(Debug)]
pub enum GateResult {
    /// Operation is allowed without intervention.
    Allowed,
    /// Operation is blocked but can be overridden. Contains a human-readable
    /// warning message.
    BlockedWithOverride(String),
    /// Operation is unconditionally denied. Contains a human-readable error
    /// message.
    Denied(String),
}

/// Holds the merged gate rules for the current workspace. Per-repo overrides
/// take precedence over global config.
pub struct GitGateEnforcer {
    gates: Vec<BranchGate>,
}

impl GitGateEnforcer {
    /// Look up the gate rule for a specific branch. Returns `None` if no rule
    /// is configured for that branch (in which case the operation is allowed).
    fn gate_for(&self, branch: &str) -> Option<&BranchGate> {
        self.gates.iter().find(|g| g.branch == branch)
    }

    /// Check if a branch name is in the allowed list (has a gate defined).
    /// Returns `true` if the branch is allowed, `false` otherwise.
    pub fn is_branch_allowed(&self, branch: &str) -> bool {
        self.gates.iter().any(|g| g.branch == branch)
    }

    /// Get the list of allowed branch names.
    pub fn allowed_branches(&self) -> Vec<&str> {
        self.gates.iter().map(|g| g.branch.as_str()).collect()
    }
}

/// Load gate rules by merging global config (`~/.config/swebash/config.toml`)
/// with any per-repo overrides (`.swebash/git.toml` in the workspace root).
///
/// Per-repo gates override global gates on a per-branch basis.
pub fn load_gates(workspace_root: &Path) -> GitGateEnforcer {
    // 1. Load global config
    let global_config = load_global_git_config();

    // 2. Load per-repo config
    let repo_config = load_repo_git_config(workspace_root);

    // 3. Merge: start with global gates, override with repo gates
    let mut gates = global_config
        .map(|c| c.gates)
        .unwrap_or_default();

    if let Some(repo) = repo_config {
        for repo_gate in repo.gates {
            if let Some(existing) = gates.iter_mut().find(|g| g.branch == repo_gate.branch) {
                *existing = repo_gate;
            } else {
                gates.push(repo_gate);
            }
        }
    }

    GitGateEnforcer { gates }
}

/// Load the `[git]` section from the global swebash config.
fn load_global_git_config() -> Option<GitConfig> {
    let config_path = dirs::home_dir()
        .map(|h| h.join(".config").join("swebash").join("config.toml"))?;

    let contents = std::fs::read_to_string(&config_path).ok()?;

    // Parse the full config and extract the git section
    let config: super::config::SwebashConfig = toml::from_str(&contents).ok()?;
    config.git
}

/// Load a per-repo `GitConfig` from `.swebash/git.toml` under the given root.
fn load_repo_git_config(workspace_root: &Path) -> Option<GitConfig> {
    let git_toml = workspace_root.join(".swebash").join("git.toml");
    let contents = std::fs::read_to_string(git_toml).ok()?;
    toml::from_str::<GitConfig>(&contents).ok()
}

/// Get the remote URL for the given remote name (default: "origin").
///
/// Returns `None` if not in a git repo or remote doesn't exist.
pub fn current_remote(cwd: &Path, remote: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", remote])
        .current_dir(cwd)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

/// Get the current branch name by running `git rev-parse --abbrev-ref HEAD`.
///
/// Returns `None` if not in a git repo or in detached HEAD state.
pub fn current_branch(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if branch == "HEAD" {
        // Detached HEAD state
        None
    } else {
        Some(branch)
    }
}

/// Check whether a git operation (identified by parsed subcommand and args) is
/// allowed on the current branch.
///
/// `args` is the full argument list passed to `git` (e.g. `["commit", "-m", "msg"]`).
pub fn check_git_operation(
    enforcer: &GitGateEnforcer,
    args: &[&str],
    cwd: &Path,
) -> GateResult {
    if args.is_empty() {
        return GateResult::Allowed;
    }

    let branch = match current_branch(cwd) {
        Some(b) => b,
        None => return GateResult::Allowed,
    };

    check_operation_on_branch(enforcer, args, &branch)
}

/// Extract target branch name from branch creation commands.
/// Returns `Some(branch_name)` if this is a branch creation command.
fn extract_branch_creation_target<'a>(args: &[&'a str]) -> Option<&'a str> {
    if args.is_empty() {
        return None;
    }

    match args[0] {
        // git checkout -b <branch>
        "checkout" => {
            let mut iter = args.iter().skip(1);
            while let Some(arg) = iter.next() {
                if *arg == "-b" || *arg == "-B" {
                    return iter.next().copied();
                }
            }
            None
        }
        // git switch -c <branch> or git switch --create <branch>
        "switch" => {
            let mut iter = args.iter().skip(1);
            while let Some(arg) = iter.next() {
                if *arg == "-c" || *arg == "-C" || *arg == "--create" || *arg == "--force-create" {
                    return iter.next().copied();
                }
            }
            None
        }
        // git branch <branch> (creates a new branch)
        "branch" => {
            // Skip flags, find the first positional arg that isn't a flag
            let positional: Vec<_> = args.iter()
                .skip(1)
                .filter(|a| !a.starts_with('-'))
                .collect();
            // `git branch <new>` has 1 positional (creates branch)
            // `git branch -d <name>` is deletion, not creation
            if positional.len() == 1 && !args.iter().any(|a| *a == "-d" || *a == "-D" || *a == "--delete") {
                return Some(positional[0]);
            }
            None
        }
        _ => None,
    }
}

/// Core gate-check logic against a known branch name. Separated from
/// `check_git_operation` so it can be unit-tested without a real git repo.
fn check_operation_on_branch(
    enforcer: &GitGateEnforcer,
    args: &[&str],
    branch: &str,
) -> GateResult {
    if args.is_empty() {
        return GateResult::Allowed;
    }

    let subcommand = args[0];

    // Check for branch creation - target branch must be in allowed list
    if let Some(target_branch) = extract_branch_creation_target(args) {
        if !enforcer.is_branch_allowed(target_branch) {
            let allowed = enforcer.allowed_branches();
            return GateResult::Denied(format!(
                "\x1b[1;31merror:\x1b[0m Cannot create branch '\x1b[1m{target_branch}\x1b[0m'. \
                 Only configured branches are allowed.\n\
                 Allowed branches: {}\n\
                 Configure branches in ~/.config/swebash/config.toml or .swebash/git.toml",
                allowed.join(", ")
            ));
        }
    }

    let gate = match enforcer.gate_for(branch) {
        Some(g) => g,
        None => return GateResult::Allowed,
    };

    let (action, op_label) = match subcommand {
        "commit" => (gate.can_commit, "commit"),
        "push" => {
            let is_force = args.iter().any(|a| *a == "--force" || *a == "-f" || *a == "--force-with-lease");
            if is_force {
                (gate.can_force_push, "force-push")
            } else {
                (gate.can_push, "push")
            }
        }
        "merge" | "rebase" => (gate.can_merge, subcommand),
        _ => return GateResult::Allowed,
    };

    match action {
        GateAction::Allow => GateResult::Allowed,
        GateAction::BlockWithOverride => GateResult::BlockedWithOverride(format!(
            "\x1b[1;33mwarning:\x1b[0m git {op_label} on protected branch '\x1b[1m{branch}\x1b[0m' is restricted.\n\
             Type \x1b[1myes\x1b[0m to proceed, or anything else to cancel: "
        )),
        GateAction::Deny => GateResult::Denied(format!(
            "\x1b[1;31merror:\x1b[0m git {op_label} on protected branch '\x1b[1m{branch}\x1b[0m' is \x1b[1mdenied\x1b[0m by safety gates."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::git_config::{BranchPipeline, GitConfig};

    /// Build an enforcer from the default pipeline for testing.
    fn test_enforcer() -> GitGateEnforcer {
        let pipeline = BranchPipeline::default_for_user("alice");
        let gates = GitConfig::default_gates(&pipeline);
        GitGateEnforcer { gates }
    }

    // ── Empty / unknown args ────────────────────────────────────────────

    #[test]
    fn empty_args_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &[], "main");
        assert!(matches!(result, GateResult::Allowed));
    }

    #[test]
    fn unknown_subcommand_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["status"], "main");
        assert!(matches!(result, GateResult::Allowed));
    }

    #[test]
    fn unknown_branch_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["commit", "-m", "msg"], "feature/xyz");
        assert!(matches!(result, GateResult::Allowed));
    }

    // ── Commit ──────────────────────────────────────────────────────────

    #[test]
    fn commit_on_protected_branch_blocked_with_override() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["commit", "-m", "fix"], "main");
        assert!(matches!(result, GateResult::BlockedWithOverride(_)));
    }

    #[test]
    fn commit_on_open_branch_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["commit", "-m", "wip"], "dev_alice");
        assert!(matches!(result, GateResult::Allowed));
    }

    // ── Push ────────────────────────────────────────────────────────────

    #[test]
    fn push_on_protected_branch_blocked_with_override() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["push", "origin", "main"], "main");
        assert!(matches!(result, GateResult::BlockedWithOverride(_)));
    }

    #[test]
    fn push_on_open_branch_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["push"], "dev_alice");
        assert!(matches!(result, GateResult::Allowed));
    }

    // ── Force-push ──────────────────────────────────────────────────────

    #[test]
    fn force_push_on_protected_branch_denied() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["push", "--force"], "main");
        assert!(matches!(result, GateResult::Denied(_)));
    }

    #[test]
    fn force_push_short_flag_on_protected_branch_denied() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["push", "-f"], "main");
        assert!(matches!(result, GateResult::Denied(_)));
    }

    #[test]
    fn force_with_lease_on_protected_branch_denied() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(
            &enforcer,
            &["push", "--force-with-lease"],
            "main",
        );
        assert!(matches!(result, GateResult::Denied(_)));
    }

    #[test]
    fn force_push_on_open_branch_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["push", "--force"], "dev_alice");
        assert!(matches!(result, GateResult::Allowed));
    }

    // ── Merge / Rebase ──────────────────────────────────────────────────

    #[test]
    fn merge_on_protected_branch_blocked_with_override() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["merge", "dev_alice"], "main");
        assert!(matches!(result, GateResult::BlockedWithOverride(_)));
    }

    #[test]
    fn rebase_on_protected_branch_blocked_with_override() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["rebase", "dev_alice"], "main");
        assert!(matches!(result, GateResult::BlockedWithOverride(_)));
    }

    #[test]
    fn merge_on_open_branch_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["merge", "main"], "dev_alice");
        assert!(matches!(result, GateResult::Allowed));
    }

    // ── All protected branches ──────────────────────────────────────────

    #[test]
    fn all_protected_branches_block_commit() {
        let enforcer = test_enforcer();
        for branch in &["main", "test", "integration", "uat", "staging-prod"] {
            let result = check_operation_on_branch(&enforcer, &["commit", "-m", "x"], branch);
            assert!(
                matches!(result, GateResult::BlockedWithOverride(_)),
                "expected BlockedWithOverride for commit on {branch}"
            );
        }
    }

    #[test]
    fn all_protected_branches_deny_force_push() {
        let enforcer = test_enforcer();
        for branch in &["main", "test", "integration", "uat", "staging-prod"] {
            let result = check_operation_on_branch(&enforcer, &["push", "--force"], branch);
            assert!(
                matches!(result, GateResult::Denied(_)),
                "expected Denied for force-push on {branch}"
            );
        }
    }

    // ── Passthrough subcommands ─────────────────────────────────────────

    #[test]
    fn git_log_always_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["log", "--oneline"], "main");
        assert!(matches!(result, GateResult::Allowed));
    }

    #[test]
    fn git_diff_always_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["diff"], "main");
        assert!(matches!(result, GateResult::Allowed));
    }

    #[test]
    fn git_branch_always_allowed() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["branch", "-a"], "main");
        assert!(matches!(result, GateResult::Allowed));
    }

    // ── Enforcer construction ───────────────────────────────────────────

    #[test]
    fn enforcer_with_no_gates_allows_everything() {
        let enforcer = GitGateEnforcer { gates: vec![] };
        let result = check_operation_on_branch(&enforcer, &["commit", "-m", "x"], "main");
        assert!(matches!(result, GateResult::Allowed));
    }

    #[test]
    fn enforcer_gate_lookup() {
        let enforcer = test_enforcer();
        assert!(enforcer.gate_for("main").is_some());
        assert!(enforcer.gate_for("dev_alice").is_some());
        assert!(enforcer.gate_for("nonexistent").is_none());
    }

    // ── Message content ─────────────────────────────────────────────────

    #[test]
    fn blocked_message_contains_branch_name() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["commit", "-m", "x"], "main");
        if let GateResult::BlockedWithOverride(msg) = result {
            assert!(msg.contains("main"), "message should contain branch name");
            assert!(msg.contains("commit"), "message should contain operation");
        } else {
            panic!("expected BlockedWithOverride");
        }
    }

    #[test]
    fn denied_message_contains_branch_name() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["push", "--force"], "main");
        if let GateResult::Denied(msg) = result {
            assert!(msg.contains("main"), "message should contain branch name");
            assert!(msg.contains("force-push"), "message should contain operation");
            assert!(msg.contains("denied"), "message should contain 'denied'");
        } else {
            panic!("expected Denied");
        }
    }

    // ── Custom gate configurations ──────────────────────────────────────

    #[test]
    fn custom_deny_commit_on_branch() {
        let enforcer = GitGateEnforcer {
            gates: vec![BranchGate {
                branch: "release".to_string(),
                can_commit: GateAction::Deny,
                can_push: GateAction::Allow,
                can_merge: GateAction::Allow,
                can_force_push: GateAction::Deny,
            }],
        };
        let result = check_operation_on_branch(&enforcer, &["commit", "-m", "x"], "release");
        assert!(matches!(result, GateResult::Denied(_)));
    }

    #[test]
    fn custom_allow_force_push() {
        let enforcer = GitGateEnforcer {
            gates: vec![BranchGate {
                branch: "experiment".to_string(),
                can_commit: GateAction::Allow,
                can_push: GateAction::Allow,
                can_merge: GateAction::Allow,
                can_force_push: GateAction::Allow,
            }],
        };
        let result = check_operation_on_branch(&enforcer, &["push", "--force"], "experiment");
        assert!(matches!(result, GateResult::Allowed));
    }

    // ── Branch creation gating ──────────────────────────────────────────

    #[test]
    fn create_allowed_branch_succeeds() {
        let enforcer = test_enforcer();
        // "dev_alice" is in the default pipeline (underscore, not slash)
        let result = check_operation_on_branch(&enforcer, &["checkout", "-b", "dev_alice"], "main");
        assert!(matches!(result, GateResult::Allowed));
    }

    #[test]
    fn create_disallowed_branch_denied() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["checkout", "-b", "random-branch"], "main");
        assert!(matches!(result, GateResult::Denied(_)));
    }

    #[test]
    fn git_switch_create_disallowed_branch_denied() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["switch", "-c", "feature/random"], "main");
        assert!(matches!(result, GateResult::Denied(_)));
    }

    #[test]
    fn git_switch_create_allowed_branch_succeeds() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["switch", "--create", "main"], "dev/alice");
        assert!(matches!(result, GateResult::Allowed));
    }

    #[test]
    fn git_branch_create_disallowed_denied() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["branch", "my-feature"], "main");
        assert!(matches!(result, GateResult::Denied(_)));
    }

    #[test]
    fn git_branch_delete_allowed() {
        let enforcer = test_enforcer();
        // Deleting branches should not be blocked by branch creation gate
        let result = check_operation_on_branch(&enforcer, &["branch", "-d", "old-branch"], "main");
        assert!(matches!(result, GateResult::Allowed));
    }

    #[test]
    fn denied_branch_creation_lists_allowed_branches() {
        let enforcer = test_enforcer();
        let result = check_operation_on_branch(&enforcer, &["checkout", "-b", "nope"], "main");
        if let GateResult::Denied(msg) = result {
            assert!(msg.contains("nope"), "message should contain attempted branch");
            assert!(msg.contains("main"), "message should list allowed branches");
            assert!(msg.contains("dev_alice"), "message should list allowed branches");
        } else {
            panic!("expected Denied");
        }
    }

    #[test]
    fn extract_branch_checkout_b() {
        assert_eq!(extract_branch_creation_target(&["checkout", "-b", "feat"]), Some("feat"));
        assert_eq!(extract_branch_creation_target(&["checkout", "-B", "feat"]), Some("feat"));
        assert_eq!(extract_branch_creation_target(&["checkout", "main"]), None);
    }

    #[test]
    fn extract_branch_switch_c() {
        assert_eq!(extract_branch_creation_target(&["switch", "-c", "feat"]), Some("feat"));
        assert_eq!(extract_branch_creation_target(&["switch", "--create", "feat"]), Some("feat"));
        assert_eq!(extract_branch_creation_target(&["switch", "main"]), None);
    }

    #[test]
    fn extract_branch_branch_cmd() {
        assert_eq!(extract_branch_creation_target(&["branch", "new-branch"]), Some("new-branch"));
        assert_eq!(extract_branch_creation_target(&["branch", "-d", "old"]), None);
        assert_eq!(extract_branch_creation_target(&["branch", "-D", "old"]), None);
        assert_eq!(extract_branch_creation_target(&["branch"]), None);
    }
}
