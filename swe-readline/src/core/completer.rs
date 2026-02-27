use std::path::PathBuf;

/// Completion candidate
#[derive(Debug, Clone)]
pub struct Completion {
    pub text: String,
    pub display: String,
}

/// Trait for providing tab completions.
///
/// Consumers implement this to supply domain-specific completions
/// (e.g. shell builtins, REPL commands).
pub trait Complete {
    fn complete(&self, line: &str, pos: usize) -> Vec<Completion>;
}

/// No-op completer for consumers that don't need completion.
pub struct NoComplete;

impl Complete for NoComplete {
    fn complete(&self, _line: &str, _pos: usize) -> Vec<Completion> {
        Vec::new()
    }
}

/// Reusable filesystem path completer.
///
/// Extracted from the original shell completer so any consumer can
/// offer path completion without reimplementing it.
pub struct PathCompleter;

impl PathCompleter {
    /// Complete a partial path extracted from the input line.
    pub fn complete_path(partial_path: &str) -> Vec<Completion> {
        // Expand ~ to home directory
        let expanded = if partial_path.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(&partial_path[2..]))
                .unwrap_or_else(|| PathBuf::from(partial_path))
        } else if partial_path == "~" {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
        } else {
            PathBuf::from(partial_path)
        };

        // Get parent directory and filename prefix
        let (dir, prefix) = if expanded.to_string_lossy().ends_with('/') {
            (expanded.clone(), String::new())
        } else if let Some(parent) = expanded.parent() {
            let filename = expanded
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent.to_path_buf(), filename)
        } else {
            (PathBuf::from("."), partial_path.to_string())
        };

        // Read directory and filter matches
        std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(Result::ok))
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&prefix)
            })
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let display = if entry.path().is_dir() {
                    format!("{}/", name)
                } else {
                    name.clone()
                };
                Completion {
                    text: name,
                    display,
                }
            })
            .collect()
    }
}

/// Get common prefix of all completions
pub fn common_prefix(completions: &[Completion]) -> String {
    if completions.is_empty() {
        return String::new();
    }

    if completions.len() == 1 {
        return completions[0].text.clone();
    }

    let first = &completions[0].text;
    let mut prefix_len = first.len();

    for comp in &completions[1..] {
        prefix_len = first
            .chars()
            .zip(comp.text.chars())
            .take(prefix_len)
            .take_while(|(a, b)| a == b)
            .count();
    }

    first.chars().take(prefix_len).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_complete() {
        let completer = NoComplete;
        let completions = completer.complete("anything", 8);
        assert!(completions.is_empty());
    }

    #[test]
    fn test_common_prefix() {
        let completions = vec![
            Completion {
                text: "echo".to_string(),
                display: "echo".to_string(),
            },
            Completion {
                text: "env".to_string(),
                display: "env".to_string(),
            },
        ];
        assert_eq!(common_prefix(&completions), "e");
    }

    #[test]
    fn test_common_prefix_single() {
        let completions = vec![Completion {
            text: "echo".to_string(),
            display: "echo".to_string(),
        }];
        assert_eq!(common_prefix(&completions), "echo");
    }

    #[test]
    fn test_common_prefix_empty() {
        let completions: Vec<Completion> = vec![];
        assert_eq!(common_prefix(&completions), "");
    }
}
