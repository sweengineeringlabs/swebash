use std::path::PathBuf;

/// Completion candidate
#[derive(Debug, Clone)]
pub struct Completion {
    pub text: String,
    pub display: String,
}

/// Tab completion handler
pub struct Completer {
    builtin_commands: Vec<String>,
}

impl Completer {
    pub fn new() -> Self {
        Self {
            builtin_commands: vec![
                "echo", "pwd", "cd", "ls", "cat", "mkdir", "rm",
                "cp", "mv", "touch", "env", "export", "head", "tail",
                "ai", "exit",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }

    /// Complete the input line at the cursor position
    pub fn complete(&self, line: &str, pos: usize) -> Vec<Completion> {
        let before_cursor = &line[..pos];

        // Determine what to complete
        if before_cursor.trim().is_empty() || !before_cursor.contains(char::is_whitespace) {
            // Complete command
            self.complete_command(before_cursor)
        } else {
            // Complete path/argument
            self.complete_path(before_cursor)
        }
    }

    fn complete_command(&self, prefix: &str) -> Vec<Completion> {
        self.builtin_commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|cmd| Completion {
                text: cmd.clone(),
                display: cmd.clone(),
            })
            .collect()
    }

    fn complete_path(&self, line: &str) -> Vec<Completion> {
        // Extract the path component to complete
        let parts: Vec<&str> = line.split_whitespace().collect();
        let partial_path = parts.last().unwrap_or(&"");

        // Expand ~ to home directory
        let expanded = if partial_path.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(&partial_path[2..]))
                .unwrap_or_else(|| PathBuf::from(partial_path))
        } else if *partial_path == "~" {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_command() {
        let completer = Completer::new();
        let completions = completer.complete("ec", 2);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].text, "echo");
    }

    #[test]
    fn test_complete_multiple_commands() {
        let completer = Completer::new();
        let completions = completer.complete("e", 1);
        assert!(completions.len() >= 2); // echo, env, export, exit
        assert!(completions.iter().any(|c| c.text == "echo"));
        assert!(completions.iter().any(|c| c.text == "env"));
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
        assert_eq!(Completer::common_prefix(&completions), "e");
    }

    #[test]
    fn test_common_prefix_single() {
        let completions = vec![Completion {
            text: "echo".to_string(),
            display: "echo".to_string(),
        }];
        assert_eq!(Completer::common_prefix(&completions), "echo");
    }
}
