/// Formatted AI output with colors.
use std::io::{self, Write};

// ANSI color codes
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Print an AI-prefixed informational message.
pub fn ai_info(msg: &str) {
    let _ = writeln!(io::stdout(), "{}{}[ai]{} {}", BOLD, CYAN, RESET, msg);
}

/// Print an AI-prefixed success message.
pub fn ai_success(msg: &str) {
    let _ = writeln!(io::stdout(), "{}{}[ai]{} {}", BOLD, GREEN, RESET, msg);
}

/// Print an AI-prefixed warning message.
pub fn ai_warn(msg: &str) {
    let _ = writeln!(io::stdout(), "{}{}[ai]{} {}", BOLD, YELLOW, RESET, msg);
}

/// Print an AI-prefixed error message.
pub fn ai_error(msg: &str) {
    let _ = writeln!(io::stderr(), "{}{}[ai]{} {}", BOLD, RED, RESET, msg);
}

/// Print a suggested command with highlight.
pub fn ai_command(cmd: &str) {
    let _ = writeln!(
        io::stdout(),
        "\n  {}{}{}{}",
        BOLD, GREEN, cmd, RESET
    );
}

/// Print an explanation block.
pub fn ai_explanation(text: &str) {
    let _ = writeln!(io::stdout());
    for line in text.lines() {
        let _ = writeln!(io::stdout(), "  {}", line);
    }
    let _ = writeln!(io::stdout());
}

/// Print a chat reply.
pub fn ai_reply(text: &str) {
    let _ = writeln!(io::stdout());
    for line in text.lines() {
        let _ = writeln!(io::stdout(), "  {}{}{}", CYAN, line, RESET);
    }
    let _ = writeln!(io::stdout());
}

/// Print autocomplete suggestions.
pub fn ai_suggestions(suggestions: &[String]) {
    let _ = writeln!(io::stdout(), "\n{}Suggestions:{}", BOLD, RESET);
    for (i, suggestion) in suggestions.iter().enumerate() {
        let _ = writeln!(
            io::stdout(),
            "  {}{}){} {}",
            DIM,
            i + 1,
            RESET,
            suggestion
        );
    }
    let _ = writeln!(io::stdout());
}

/// Print a "thinking..." indicator.
pub fn ai_thinking() {
    let _ = write!(io::stdout(), "{}{}[ai]{} thinking...", BOLD, CYAN, RESET);
    let _ = io::stdout().flush();
}

/// Clear the "thinking..." line.
pub fn ai_thinking_done() {
    // Move cursor to beginning and clear line
    let _ = write!(io::stdout(), "\r\x1b[K");
    let _ = io::stdout().flush();
}

/// Print the execute confirmation prompt.
pub fn ai_confirm_prompt() {
    let _ = write!(
        io::stdout(),
        "\n  {}Execute? [Y/n/e(dit)]{} ",
        DIM, RESET
    );
    let _ = io::stdout().flush();
}

/// Print AI status information.
pub fn ai_status(
    enabled: bool,
    provider: &str,
    model: &str,
    ready: bool,
) {
    let _ = writeln!(io::stdout(), "\n{}AI Status:{}", BOLD, RESET);
    let _ = writeln!(
        io::stdout(),
        "  Enabled:  {}{}{}",
        if enabled { GREEN } else { RED },
        if enabled { "yes" } else { "no" },
        RESET
    );
    let _ = writeln!(io::stdout(), "  Provider: {}", provider);
    let _ = writeln!(io::stdout(), "  Model:    {}", model);
    let _ = writeln!(
        io::stdout(),
        "  Ready:    {}{}{}",
        if ready { GREEN } else { RED },
        if ready { "yes" } else { "no" },
        RESET
    );
    let _ = writeln!(io::stdout());
}

/// Print a "not configured" friendly message.
pub fn ai_not_configured() {
    ai_warn("AI is not configured.");
    let _ = writeln!(io::stdout(), "  Set an API key to enable AI features:");
    let _ = writeln!(io::stdout(), "    export OPENAI_API_KEY=sk-...");
    let _ = writeln!(io::stdout(), "    export ANTHROPIC_API_KEY=sk-ant-...");
    let _ = writeln!(io::stdout(), "    export GEMINI_API_KEY=...");
    let _ = writeln!(
        io::stdout(),
        "  Then configure the provider: export LLM_PROVIDER=openai"
    );
    let _ = writeln!(io::stdout());
}
