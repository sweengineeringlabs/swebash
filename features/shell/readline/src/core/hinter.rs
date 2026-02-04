use super::config::ColorConfig;
use crate::core::history::History;

/// History-based hinter (fish-shell style)
pub struct Hinter {
    colors: ColorConfig,
}

impl Hinter {
    pub fn new(colors: ColorConfig) -> Self {
        Self { colors }
    }

    /// Get hint for the current line based on history
    pub fn hint(&self, line: &str, history: &History) -> Option<String> {
        if line.trim().is_empty() {
            return None;
        }

        // Find most recent matching history entry
        history
            .commands()
            .iter()
            .rev() // Most recent first
            .find(|entry| entry.starts_with(line) && entry.len() > line.len())
            .map(|entry| {
                // Return the completion part (grayed out)
                let hint_text = &entry[line.len()..];
                format!("{}{}\x1b[0m", self.colors.hint_ansi(), hint_text)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_hint_from_history() {
        let hinter = Hinter::new(ColorConfig::default());
        let mut history = History::new(100);
        history.add("echo hello world".to_string());
        history.add("echo test".to_string());

        let hint = hinter.hint("echo h", &history);
        assert!(hint.is_some());
        let hint_text = hint.unwrap();
        assert!(hint_text.contains("ello world") || hint_text.contains("world"));
    }

    #[test]
    fn test_no_hint_for_empty() {
        let hinter = Hinter::new(ColorConfig::default());
        let history = History::new(100);

        let hint = hinter.hint("", &history);
        assert!(hint.is_none());
    }

    #[test]
    fn test_no_hint_for_no_match() {
        let hinter = Hinter::new(ColorConfig::default());
        let mut history = History::new(100);
        history.add("echo test".to_string());

        let hint = hinter.hint("pwd", &history);
        assert!(hint.is_none());
    }
}
