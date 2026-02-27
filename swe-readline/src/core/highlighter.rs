/// Trait for syntax highlighting input lines.
///
/// Consumers implement this to supply domain-specific highlighting
/// (e.g. shell keywords, REPL commands).
pub trait Highlight {
    /// Return the input `line` with ANSI color codes inserted.
    fn highlight(&self, line: &str) -> String;
}

/// No-op highlighter — returns the line unchanged.
pub struct NoHighlight;

impl Highlight for NoHighlight {
    fn highlight(&self, line: &str) -> String {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_highlight() {
        let hl = NoHighlight;
        assert_eq!(hl.highlight("echo hello"), "echo hello");
    }
}
