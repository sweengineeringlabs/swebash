use serde::{Deserialize, Serialize};

/// Action to take when a git operation is attempted on a gated branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateAction {
    /// Operation is allowed without any prompt.
    Allow,
    /// Operation is blocked but can be overridden with explicit confirmation.
    BlockWithOverride,
    /// Operation is unconditionally denied.
    Deny,
}

impl std::fmt::Display for GateAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateAction::Allow => write!(f, "allow"),
            GateAction::BlockWithOverride => write!(f, "block_with_override"),
            GateAction::Deny => write!(f, "deny"),
        }
    }
}

/// Role classification for a branch in the pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchRole {
    Main,
    Dev,
    Test,
    Integration,
    Uat,
    Staging,
    Custom,
}

impl std::fmt::Display for BranchRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchRole::Main => write!(f, "main"),
            BranchRole::Dev => write!(f, "dev"),
            BranchRole::Test => write!(f, "test"),
            BranchRole::Integration => write!(f, "integration"),
            BranchRole::Uat => write!(f, "uat"),
            BranchRole::Staging => write!(f, "staging"),
            BranchRole::Custom => write!(f, "custom"),
        }
    }
}

/// Definition of a single branch in the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDef {
    /// Branch name (e.g. `"main"`, `"dev_alice"`).
    pub name: String,
    /// Role of this branch in the pipeline.
    pub role: BranchRole,
    /// Whether this branch is considered protected.
    pub protected: bool,
}

/// Ordered list of branches forming the promotion pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BranchPipeline {
    pub branches: Vec<BranchDef>,
}

impl BranchPipeline {
    /// Generate the default 6-branch pipeline template for a given user id.
    ///
    /// Pipeline: `main` → `dev_{user_id}` → `test` → `integration` → `uat` → `staging-prod`
    pub fn default_for_user(user_id: &str) -> Self {
        Self {
            branches: vec![
                BranchDef {
                    name: "main".to_string(),
                    role: BranchRole::Main,
                    protected: true,
                },
                BranchDef {
                    name: format!("dev_{user_id}"),
                    role: BranchRole::Dev,
                    protected: false,
                },
                BranchDef {
                    name: "test".to_string(),
                    role: BranchRole::Test,
                    protected: true,
                },
                BranchDef {
                    name: "integration".to_string(),
                    role: BranchRole::Integration,
                    protected: true,
                },
                BranchDef {
                    name: "uat".to_string(),
                    role: BranchRole::Uat,
                    protected: true,
                },
                BranchDef {
                    name: "staging-prod".to_string(),
                    role: BranchRole::Staging,
                    protected: true,
                },
            ],
        }
    }
}

/// Per-branch gate rules controlling which git operations are allowed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchGate {
    /// Branch name this gate applies to.
    pub branch: String,
    /// Action for `git commit` on this branch.
    pub can_commit: GateAction,
    /// Action for `git push` to this branch.
    pub can_push: GateAction,
    /// Action for `git merge` / `git rebase` on this branch.
    pub can_merge: GateAction,
    /// Action for `git push --force` to this branch.
    pub can_force_push: GateAction,
}

/// Top-level `[git]` configuration section.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitConfig {
    /// User identifier (typically from `git config user.name`).
    #[serde(default)]
    pub user_id: String,
    /// Branch promotion pipeline.
    #[serde(default)]
    pub pipeline: BranchPipeline,
    /// Per-branch safety gates.
    #[serde(default)]
    pub gates: Vec<BranchGate>,
}

impl GitConfig {
    /// Generate default gate rules for a given pipeline.
    ///
    /// Protected branches get `BlockWithOverride` for commit/push/merge and
    /// `Deny` for force-push. Open branches get `Allow` for everything.
    pub fn default_gates(pipeline: &BranchPipeline) -> Vec<BranchGate> {
        pipeline
            .branches
            .iter()
            .map(|branch| {
                if branch.protected {
                    BranchGate {
                        branch: branch.name.clone(),
                        can_commit: GateAction::BlockWithOverride,
                        can_push: GateAction::BlockWithOverride,
                        can_merge: GateAction::BlockWithOverride,
                        can_force_push: GateAction::Deny,
                    }
                } else {
                    BranchGate {
                        branch: branch.name.clone(),
                        can_commit: GateAction::Allow,
                        can_push: GateAction::Allow,
                        can_merge: GateAction::Allow,
                        can_force_push: GateAction::Allow,
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pipeline_has_six_branches() {
        let pipeline = BranchPipeline::default_for_user("alice");
        assert_eq!(pipeline.branches.len(), 6);
    }

    #[test]
    fn default_pipeline_branch_names() {
        let pipeline = BranchPipeline::default_for_user("bob");
        let names: Vec<&str> = pipeline.branches.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["main", "dev_bob", "test", "integration", "uat", "staging-prod"]
        );
    }

    #[test]
    fn default_pipeline_dev_branch_uses_user_id() {
        let pipeline = BranchPipeline::default_for_user("charlie_123");
        assert_eq!(pipeline.branches[1].name, "dev_charlie_123");
        assert_eq!(pipeline.branches[1].role, BranchRole::Dev);
        assert!(!pipeline.branches[1].protected);
    }

    #[test]
    fn default_pipeline_roles() {
        let pipeline = BranchPipeline::default_for_user("x");
        let roles: Vec<&BranchRole> = pipeline.branches.iter().map(|b| &b.role).collect();
        assert_eq!(
            roles,
            vec![
                &BranchRole::Main,
                &BranchRole::Dev,
                &BranchRole::Test,
                &BranchRole::Integration,
                &BranchRole::Uat,
                &BranchRole::Staging,
            ]
        );
    }

    #[test]
    fn default_pipeline_protection_flags() {
        let pipeline = BranchPipeline::default_for_user("x");
        let protected: Vec<bool> = pipeline.branches.iter().map(|b| b.protected).collect();
        // main=true, dev=false, test=true, integration=true, uat=true, staging-prod=true
        assert_eq!(protected, vec![true, false, true, true, true, true]);
    }

    #[test]
    fn default_gates_protected_branch_blocks_commit_push_merge() {
        let pipeline = BranchPipeline::default_for_user("x");
        let gates = GitConfig::default_gates(&pipeline);

        // "main" is protected (index 0)
        let main_gate = &gates[0];
        assert_eq!(main_gate.branch, "main");
        assert_eq!(main_gate.can_commit, GateAction::BlockWithOverride);
        assert_eq!(main_gate.can_push, GateAction::BlockWithOverride);
        assert_eq!(main_gate.can_merge, GateAction::BlockWithOverride);
        assert_eq!(main_gate.can_force_push, GateAction::Deny);
    }

    #[test]
    fn default_gates_open_branch_allows_everything() {
        let pipeline = BranchPipeline::default_for_user("alice");
        let gates = GitConfig::default_gates(&pipeline);

        // "dev_alice" is not protected (index 1)
        let dev_gate = &gates[1];
        assert_eq!(dev_gate.branch, "dev_alice");
        assert_eq!(dev_gate.can_commit, GateAction::Allow);
        assert_eq!(dev_gate.can_push, GateAction::Allow);
        assert_eq!(dev_gate.can_merge, GateAction::Allow);
        assert_eq!(dev_gate.can_force_push, GateAction::Allow);
    }

    #[test]
    fn default_gates_count_matches_pipeline() {
        let pipeline = BranchPipeline::default_for_user("x");
        let gates = GitConfig::default_gates(&pipeline);
        assert_eq!(gates.len(), pipeline.branches.len());
    }

    #[test]
    fn gate_action_serde_roundtrip() {
        // TOML cannot serialize bare enums; wrap in a struct
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Wrapper {
            action: GateAction,
        }
        let actions = vec![GateAction::Allow, GateAction::BlockWithOverride, GateAction::Deny];
        for action in actions {
            let wrapper = Wrapper { action };
            let serialized = toml::to_string(&wrapper).unwrap();
            let deserialized: Wrapper = toml::from_str(&serialized).unwrap();
            assert_eq!(wrapper, deserialized);
        }
    }

    #[test]
    fn git_config_serde_roundtrip() {
        let pipeline = BranchPipeline::default_for_user("testuser");
        let gates = GitConfig::default_gates(&pipeline);
        let config = GitConfig {
            user_id: "testuser".to_string(),
            pipeline,
            gates,
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: GitConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.user_id, "testuser");
        assert_eq!(deserialized.pipeline.branches.len(), 6);
        assert_eq!(deserialized.gates.len(), 6);
        assert_eq!(deserialized.gates[0].branch, "main");
        assert_eq!(deserialized.gates[0].can_force_push, GateAction::Deny);
        assert_eq!(deserialized.gates[1].branch, "dev_testuser");
        assert_eq!(deserialized.gates[1].can_commit, GateAction::Allow);
    }

    #[test]
    fn gate_action_display() {
        assert_eq!(format!("{}", GateAction::Allow), "allow");
        assert_eq!(format!("{}", GateAction::BlockWithOverride), "block_with_override");
        assert_eq!(format!("{}", GateAction::Deny), "deny");
    }

    #[test]
    fn branch_role_display() {
        assert_eq!(format!("{}", BranchRole::Main), "main");
        assert_eq!(format!("{}", BranchRole::Dev), "dev");
        assert_eq!(format!("{}", BranchRole::Test), "test");
        assert_eq!(format!("{}", BranchRole::Integration), "integration");
        assert_eq!(format!("{}", BranchRole::Uat), "uat");
        assert_eq!(format!("{}", BranchRole::Staging), "staging");
        assert_eq!(format!("{}", BranchRole::Custom), "custom");
    }

    #[test]
    fn empty_pipeline_produces_no_gates() {
        let pipeline = BranchPipeline::default();
        let gates = GitConfig::default_gates(&pipeline);
        assert!(gates.is_empty());
    }

    #[test]
    fn gate_action_deserialized_from_snake_case() {
        let toml_str = r#"action = "block_with_override""#;
        #[derive(Deserialize)]
        struct Wrapper {
            action: GateAction,
        }
        let w: Wrapper = toml::from_str(toml_str).unwrap();
        assert_eq!(w.action, GateAction::BlockWithOverride);
    }
}
