/// Parse AI command triggers from user input.

/// Recognized AI command types.
#[derive(Debug)]
pub enum AiCommand {
    /// `ai ask <text>` or `? <text>` — translate NL to shell command.
    Ask(String),
    /// `ai explain <cmd>` or `?? <cmd>` — explain a command.
    Explain(String),
    /// `ai chat <text>` — conversational assistant.
    Chat(String),
    /// `ai suggest` — autocomplete suggestions.
    Suggest,
    /// `ai status` — show AI configuration status.
    Status,
    /// `ai history` — show chat history.
    History,
    /// `ai clear` — clear chat history.
    Clear,
    /// `ai` alone — enter AI mode.
    EnterMode,
    /// `exit` or `quit` — exit AI mode (only valid in AI mode).
    ExitMode,
}

/// Try to parse the input as an AI command.
///
/// Returns `Some(AiCommand)` if the input matches an AI trigger,
/// `None` if it should be passed to the WASM engine.
pub fn parse_ai_command(input: &str) -> Option<AiCommand> {
    let trimmed = input.trim();

    // Shorthand: `?? <cmd>` — explain
    if let Some(rest) = trimmed.strip_prefix("??") {
        let text = rest.trim();
        if !text.is_empty() {
            return Some(AiCommand::Explain(text.to_string()));
        }
    }

    // Shorthand: `? <text>` — ask (must check after `??`)
    if let Some(rest) = trimmed.strip_prefix('?') {
        let text = rest.trim();
        if !text.is_empty() {
            return Some(AiCommand::Ask(text.to_string()));
        }
    }

    // `ai <subcommand> [args...]`
    if let Some(rest) = trimmed.strip_prefix("ai ").or_else(|| trimmed.strip_prefix("ai\t")) {
        let rest = rest.trim();
        if let Some(text) = rest.strip_prefix("ask ").or_else(|| rest.strip_prefix("ask\t")) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(AiCommand::Ask(text.to_string()));
            }
        } else if let Some(text) =
            rest.strip_prefix("explain ").or_else(|| rest.strip_prefix("explain\t"))
        {
            let text = text.trim();
            if !text.is_empty() {
                return Some(AiCommand::Explain(text.to_string()));
            }
        } else if let Some(text) =
            rest.strip_prefix("chat ").or_else(|| rest.strip_prefix("chat\t"))
        {
            let text = text.trim();
            if !text.is_empty() {
                return Some(AiCommand::Chat(text.to_string()));
            }
        } else if rest == "suggest" {
            return Some(AiCommand::Suggest);
        } else if rest == "status" {
            return Some(AiCommand::Status);
        } else if rest == "history" {
            return Some(AiCommand::History);
        } else if rest == "clear" {
            return Some(AiCommand::Clear);
        }
    }

    // Exact match: `ai status`, etc. (without trailing space parsed above)
    match trimmed {
        "ai suggest" => return Some(AiCommand::Suggest),
        "ai status" => return Some(AiCommand::Status),
        "ai history" => return Some(AiCommand::History),
        "ai clear" => return Some(AiCommand::Clear),
        "ai" => return Some(AiCommand::EnterMode),
        _ => {}
    }

    None
}

/// Parse input in AI mode with smart detection.
///
/// This is called when already in AI mode. It uses smart detection to automatically
/// route input to the appropriate AI command based on patterns.
///
/// Priority:
/// 1. Explicit subcommands (chat, ask, explain, etc.)
/// 2. Exit commands (exit, quit)
/// 3. Smart detection (command patterns, action verbs, questions)
/// 4. Default to chat (conversational fallback)
pub fn parse_ai_mode_command(input: &str) -> AiCommand {
    let trimmed = input.trim();

    // 1. Exit commands
    if trimmed == "exit" || trimmed == "quit" {
        return AiCommand::ExitMode;
    }

    // 2. Explicit subcommands take precedence
    if let Some(text) = trimmed.strip_prefix("ask ").or_else(|| trimmed.strip_prefix("ask\t")) {
        let text = text.trim();
        if !text.is_empty() {
            return AiCommand::Ask(text.to_string());
        }
    }

    if let Some(text) = trimmed
        .strip_prefix("explain ")
        .or_else(|| trimmed.strip_prefix("explain\t"))
    {
        let text = text.trim();
        if !text.is_empty() {
            return AiCommand::Explain(text.to_string());
        }
    }

    if let Some(text) = trimmed.strip_prefix("chat ").or_else(|| trimmed.strip_prefix("chat\t")) {
        let text = text.trim();
        if !text.is_empty() {
            return AiCommand::Chat(text.to_string());
        }
    }

    // Exact match subcommands
    match trimmed {
        "suggest" => return AiCommand::Suggest,
        "status" => return AiCommand::Status,
        "history" => return AiCommand::History,
        "clear" => return AiCommand::Clear,
        _ => {}
    }

    // 3. Smart detection
    if looks_like_command(trimmed) {
        return AiCommand::Explain(trimmed.to_string());
    }

    if is_action_request(trimmed) {
        return AiCommand::Ask(trimmed.to_string());
    }

    // 4. Default to chat (handles questions and everything else)
    AiCommand::Chat(trimmed.to_string())
}

/// Check if input looks like a shell command to explain.
///
/// Detects:
/// - Known command names with flags/arguments that look like command syntax
/// - Command flags (-x, --flag)
/// - Pipes and redirects (|, >, <)
///
/// Ambiguous cases (e.g., "find large files") are NOT treated as commands
/// unless they have clear command syntax (flags, pipes, paths).
fn looks_like_command(input: &str) -> bool {
    // Has flags (- or --)
    if input.contains(" -") || input.contains(" --") {
        return true;
    }

    // Has pipes or redirects
    if input.contains('|') || input.contains(" > ") || input.contains(" < ") {
        return true;
    }

    let first_word = input.split_whitespace().next().unwrap_or("");
    let rest = input[first_word.len()..].trim();

    // Unambiguous shell commands (not also common English words)
    let unambiguous_commands = [
        "ls", "cd", "pwd", "mkdir", "rm", "cp", "mv", "cat", "grep", "sed", "awk",
        "tar", "gzip", "gunzip", "zip", "unzip", "curl", "wget", "ssh", "scp", "rsync", "chmod",
        "chown", "ps", "top", "df", "du", "mount", "umount", "apt", "yum", "brew",
        "git", "docker", "kubectl", "cargo", "npm", "pip", "make", "printf", "head",
        "tail", "sort", "uniq", "wc", "diff", "patch", "vim", "nano", "emacs", "less", "more",
    ];

    if unambiguous_commands.contains(&first_word) {
        return true;
    }

    // Ambiguous words that could be commands OR natural language
    // Only treat as command if followed by path-like or file-like arguments
    let ambiguous_commands = ["find", "kill", "echo", "show"];
    if ambiguous_commands.contains(&first_word) {
        // Check if rest looks like file paths or command arguments
        if rest.starts_with('/') || rest.starts_with("./") || rest.starts_with("~/") {
            return true;
        }
        // Check if it has file extensions
        if rest.contains(".txt") || rest.contains(".log") || rest.contains(".sh") {
            return true;
        }
        // Otherwise, treat as natural language
        return false;
    }

    false
}

/// Check if input is an action request to translate to a command.
///
/// Detects action verbs like "find", "list", "show", "delete", etc.
fn is_action_request(input: &str) -> bool {
    let lower = input.to_lowercase();
    let first_word = lower.split_whitespace().next().unwrap_or("");

    let action_verbs = [
        "find", "list", "show", "get", "delete", "remove", "create", "make", "move", "copy",
        "search", "count", "display", "print", "download", "upload", "start", "stop", "restart",
        "kill", "compress", "extract", "archive", "backup", "restore",
    ];

    action_verbs.contains(&first_word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_enter_mode() {
        assert!(matches!(
            parse_ai_command("ai"),
            Some(AiCommand::EnterMode)
        ));
    }

    #[test]
    fn test_parse_mode_exit() {
        assert!(matches!(
            parse_ai_mode_command("exit"),
            AiCommand::ExitMode
        ));
        assert!(matches!(
            parse_ai_mode_command("quit"),
            AiCommand::ExitMode
        ));
    }

    #[test]
    fn test_parse_mode_explicit_subcommands() {
        assert!(matches!(
            parse_ai_mode_command("ask find large files"),
            AiCommand::Ask(_)
        ));
        assert!(matches!(
            parse_ai_mode_command("explain tar -xzf file.tar.gz"),
            AiCommand::Explain(_)
        ));
        assert!(matches!(
            parse_ai_mode_command("chat how do I compress?"),
            AiCommand::Chat(_)
        ));
        assert!(matches!(
            parse_ai_mode_command("status"),
            AiCommand::Status
        ));
    }

    #[test]
    fn test_looks_like_command_known() {
        assert!(looks_like_command("ls -la"));
        assert!(looks_like_command("grep pattern file.txt"));
        assert!(looks_like_command("tar -xzf archive.tar.gz"));
        assert!(looks_like_command("docker ps"));
    }

    #[test]
    fn test_looks_like_command_flags() {
        assert!(looks_like_command("unknown --help"));
        assert!(looks_like_command("something -v"));
    }

    #[test]
    fn test_looks_like_command_pipes() {
        assert!(looks_like_command("ps aux | grep node"));
        assert!(looks_like_command("cat file.txt > output.txt"));
        assert!(looks_like_command("echo test < input.txt"));
    }

    #[test]
    fn test_looks_like_command_negative() {
        assert!(!looks_like_command("how do I list files"));
        assert!(!looks_like_command("find large log files"));
        assert!(!looks_like_command("what is tar"));
    }

    #[test]
    fn test_is_action_request() {
        assert!(is_action_request("find files larger than 100MB"));
        assert!(is_action_request("list running processes"));
        assert!(is_action_request("show disk usage"));
        assert!(is_action_request("delete old logs"));
    }

    #[test]
    fn test_is_action_request_negative() {
        assert!(!is_action_request("ls -la"));
        assert!(!is_action_request("how do I compress files"));
        assert!(!is_action_request("what is grep"));
    }

    #[test]
    fn test_smart_detection_command() {
        // Known command → explain
        assert!(matches!(
            parse_ai_mode_command("tar -xzf archive.tar.gz"),
            AiCommand::Explain(_)
        ));
        // Pipe → explain
        assert!(matches!(
            parse_ai_mode_command("ps aux | grep node"),
            AiCommand::Explain(_)
        ));
    }

    #[test]
    fn test_smart_detection_action() {
        // Action verb → ask
        assert!(matches!(
            parse_ai_mode_command("find files larger than 100MB"),
            AiCommand::Ask(_)
        ));
        assert!(matches!(
            parse_ai_mode_command("list docker containers"),
            AiCommand::Ask(_)
        ));
    }

    #[test]
    fn test_smart_detection_chat_fallback() {
        // Question → chat
        assert!(matches!(
            parse_ai_mode_command("how do I compress a directory?"),
            AiCommand::Chat(_)
        ));
        // Conversational → chat
        assert!(matches!(
            parse_ai_mode_command("that's helpful, thanks"),
            AiCommand::Chat(_)
        ));
        // Unknown → chat
        assert!(matches!(
            parse_ai_mode_command("tell me about pipes"),
            AiCommand::Chat(_)
        ));
    }

    #[test]
    fn test_explicit_override() {
        // Explicit "chat" overrides command detection
        assert!(matches!(
            parse_ai_mode_command("chat find files"),
            AiCommand::Chat(_)
        ));
        // Explicit "explain" overrides action detection
        assert!(matches!(
            parse_ai_mode_command("explain how pipes work"),
            AiCommand::Explain(_)
        ));
    }
}
