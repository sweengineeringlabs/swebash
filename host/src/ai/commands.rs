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
        _ => {}
    }

    None
}
