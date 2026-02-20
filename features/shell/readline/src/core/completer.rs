use swe_readline::{Complete, Completion, PathCompleter};

/// Shell-specific completer with builtin commands and path completion.
pub struct ShellCompleter {
    builtin_commands: Vec<String>,
}

impl ShellCompleter {
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
        let parts: Vec<&str> = line.split_whitespace().collect();
        let partial_path = parts.last().unwrap_or(&"");
        PathCompleter::complete_path(partial_path)
    }

    /// Complete the input line at the cursor position.
    ///
    /// This inherent method provides backward compatibility so callers
    /// don't need to import the `Complete` trait.
    pub fn complete(&self, line: &str, pos: usize) -> Vec<Completion> {
        <Self as Complete>::complete(self, line, pos)
    }
}

impl Complete for ShellCompleter {
    fn complete(&self, line: &str, pos: usize) -> Vec<Completion> {
        let before_cursor = &line[..pos];

        if before_cursor.trim().is_empty() || !before_cursor.contains(char::is_whitespace) {
            self.complete_command(before_cursor)
        } else {
            self.complete_path(before_cursor)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_command() {
        let completer = ShellCompleter::new();
        let completions = completer.complete("ec", 2);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].text, "echo");
    }

    #[test]
    fn test_complete_multiple_commands() {
        let completer = ShellCompleter::new();
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
        assert_eq!(swe_readline::common_prefix(&completions), "e");
    }

    #[test]
    fn test_common_prefix_single() {
        let completions = vec![Completion {
            text: "echo".to_string(),
            display: "echo".to_string(),
        }];
        assert_eq!(swe_readline::common_prefix(&completions), "echo");
    }
}
