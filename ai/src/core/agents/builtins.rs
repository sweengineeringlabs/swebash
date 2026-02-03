/// Built-in agent definitions.
///
/// Each agent has a dedicated system prompt, tool filter, and trigger keywords.
/// The `create_default_registry()` factory wires them into an `AgentRegistry`.
use std::sync::Arc;

use llm_provider::LlmService;

use crate::config::AiConfig;

use super::{Agent, AgentRegistry, ToolFilter};

// ── Shell Agent (default) ──────────────────────────────────────────

struct ShellAgent;

impl Agent for ShellAgent {
    fn id(&self) -> &str {
        "shell"
    }

    fn display_name(&self) -> &str {
        "Shell Assistant"
    }

    fn description(&self) -> &str {
        "General-purpose shell assistant with full tool access"
    }

    fn system_prompt(&self) -> String {
        crate::core::prompt::chat_system_prompt()
    }

    fn tool_filter(&self) -> ToolFilter {
        ToolFilter::All
    }

    // Default agent — no trigger keywords (selected when nothing else matches).
}

// ── Review Agent ───────────────────────────────────────────────────

struct ReviewAgent;

impl Agent for ReviewAgent {
    fn id(&self) -> &str {
        "review"
    }

    fn display_name(&self) -> &str {
        "Code Reviewer"
    }

    fn description(&self) -> &str {
        "Reviews code for bugs, style issues, and security concerns"
    }

    fn system_prompt(&self) -> String {
        r#"You are a code review assistant embedded in swebash, a Unix-like shell.

Your role is to review code for:
- Bugs and logic errors
- Security vulnerabilities (injection, XSS, buffer overflows, etc.)
- Style and readability issues
- Performance concerns
- Missing error handling

You have read-only file system access to examine source files.

Rules:
- Be specific: reference file names, line numbers, and code snippets.
- Categorize findings by severity: critical, warning, info.
- Suggest concrete fixes, not vague recommendations.
- Focus on actionable feedback the developer can act on immediately.
- When reviewing, read the files first using your tools before commenting."#
            .to_string()
    }

    fn tool_filter(&self) -> ToolFilter {
        ToolFilter::Only {
            fs: true,
            exec: false,
            web: false,
        }
    }

    fn trigger_keywords(&self) -> Vec<&str> {
        vec!["review", "audit"]
    }
}

// ── DevOps Agent ───────────────────────────────────────────────────

struct DevOpsAgent;

impl Agent for DevOpsAgent {
    fn id(&self) -> &str {
        "devops"
    }

    fn display_name(&self) -> &str {
        "DevOps Assistant"
    }

    fn description(&self) -> &str {
        "Helps with Docker, Kubernetes, Terraform, CI/CD, and deployments"
    }

    fn system_prompt(&self) -> String {
        r#"You are a DevOps assistant embedded in swebash, a Unix-like shell.

You specialize in:
- Docker: building images, managing containers, docker-compose
- Kubernetes: kubectl commands, manifests, debugging pods
- Terraform: infrastructure as code, plan/apply workflows
- CI/CD: pipeline configuration, deployment strategies
- Cloud infrastructure: AWS, GCP, Azure CLI operations

You have full tool access to read config files, execute commands, and look up docs.

Rules:
- Always explain infrastructure changes before executing them.
- Warn about destructive operations (deleting resources, force-pushing, etc.).
- Prefer declarative approaches (IaC) over imperative ad-hoc commands.
- Reference official documentation when suggesting configuration patterns.
- Be concise and direct — DevOps practitioners value precision."#
            .to_string()
    }

    fn tool_filter(&self) -> ToolFilter {
        ToolFilter::All
    }

    fn trigger_keywords(&self) -> Vec<&str> {
        vec!["docker", "k8s", "terraform", "deploy", "pipeline"]
    }
}

// ── Git Agent ──────────────────────────────────────────────────────

struct GitAgent;

impl Agent for GitAgent {
    fn id(&self) -> &str {
        "git"
    }

    fn display_name(&self) -> &str {
        "Git Assistant"
    }

    fn description(&self) -> &str {
        "Helps with Git operations, branching strategies, and repository management"
    }

    fn system_prompt(&self) -> String {
        r#"You are a Git assistant embedded in swebash, a Unix-like shell.

You specialize in:
- Git commands: commit, branch, merge, rebase, cherry-pick, stash
- Branching strategies: GitFlow, trunk-based, feature branches
- Conflict resolution and interactive rebase
- Repository history analysis (log, blame, bisect)
- Git hooks and automation

You have file system and command execution access to inspect repos and run git commands.

Rules:
- Always show the git command you're about to run and explain what it does.
- Warn before destructive operations (force push, reset --hard, etc.).
- Prefer safe defaults: merge over rebase for shared branches.
- When resolving conflicts, show the conflicting sections clearly.
- Be concise — git users expect precise, actionable guidance."#
            .to_string()
    }

    fn tool_filter(&self) -> ToolFilter {
        ToolFilter::Only {
            fs: true,
            exec: true,
            web: false,
        }
    }

    fn trigger_keywords(&self) -> Vec<&str> {
        vec!["git", "commit", "branch", "merge", "rebase"]
    }
}

// ── Factory ────────────────────────────────────────────────────────

/// Create the default agent registry with all built-in agents.
pub fn create_default_registry(llm: Arc<dyn LlmService>, config: AiConfig) -> AgentRegistry {
    let mut registry = AgentRegistry::new(llm, config);

    registry.register(Box::new(ShellAgent));
    registry.register(Box::new(ReviewAgent));
    registry.register(Box::new(DevOpsAgent));
    registry.register(Box::new(GitAgent));

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_agent() {
        let agent = ShellAgent;
        assert_eq!(agent.id(), "shell");
        assert!(agent.trigger_keywords().is_empty());
        assert!(matches!(agent.tool_filter(), ToolFilter::All));
    }

    #[test]
    fn test_review_agent() {
        let agent = ReviewAgent;
        assert_eq!(agent.id(), "review");
        assert!(agent.trigger_keywords().contains(&"review"));
        assert!(agent.trigger_keywords().contains(&"audit"));
        match agent.tool_filter() {
            ToolFilter::Only { fs, exec, web } => {
                assert!(fs);
                assert!(!exec);
                assert!(!web);
            }
            _ => panic!("Expected ToolFilter::Only"),
        }
    }

    #[test]
    fn test_devops_agent() {
        let agent = DevOpsAgent;
        assert_eq!(agent.id(), "devops");
        assert!(agent.trigger_keywords().contains(&"docker"));
        assert!(agent.trigger_keywords().contains(&"k8s"));
        assert!(matches!(agent.tool_filter(), ToolFilter::All));
    }

    #[test]
    fn test_git_agent() {
        let agent = GitAgent;
        assert_eq!(agent.id(), "git");
        assert!(agent.trigger_keywords().contains(&"git"));
        assert!(agent.trigger_keywords().contains(&"commit"));
        match agent.tool_filter() {
            ToolFilter::Only { fs, exec, web } => {
                assert!(fs);
                assert!(exec);
                assert!(!web);
            }
            _ => panic!("Expected ToolFilter::Only"),
        }
    }
}
