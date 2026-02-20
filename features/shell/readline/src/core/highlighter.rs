use swe_readline::{ColorConfig, Highlight};

/// Shell-specific syntax highlighter with builtin commands, paths, operators.
pub struct ShellHighlighter {
    builtin_commands: Vec<String>,
    colors: ColorConfig,
}

impl ShellHighlighter {
    pub fn new(colors: ColorConfig) -> Self {
        Self {
            builtin_commands: vec![
                "echo", "pwd", "cd", "ls", "cat", "mkdir", "rm",
                "cp", "mv", "touch", "env", "export", "head", "tail",
                "ai", "exit",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            colors,
        }
    }

    fn highlight_word(&self, word: &str, is_command: bool) -> String {
        if !is_command {
            if word.starts_with('/') || word.starts_with("./") || word.starts_with("~/") {
                format!("{}{}\x1b[0m", self.colors.path_ansi(), word)
            } else {
                word.to_string()
            }
        } else if self.builtin_commands.contains(&word.to_string()) {
            format!("{}{}\x1b[0m", self.colors.builtin_ansi(), word)
        } else {
            format!("{}{}\x1b[0m", self.colors.external_ansi(), word)
        }
    }
}

impl Highlight for ShellHighlighter {
    fn highlight(&self, line: &str) -> String {
        let mut result = String::new();
        let mut chars = line.chars().peekable();
        let mut in_string = false;
        let mut string_char = '\0';
        let mut current_word = String::new();
        let mut is_first_word = true;

        while let Some(ch) = chars.next() {
            match ch {
                '"' | '\'' if !in_string => {
                    if !current_word.is_empty() {
                        result.push_str(&self.highlight_word(&current_word, is_first_word));
                        current_word.clear();
                        is_first_word = false;
                    }
                    in_string = true;
                    string_char = ch;
                    result.push_str(self.colors.string_ansi());
                    result.push(ch);
                }
                '"' | '\'' if in_string && ch == string_char => {
                    result.push(ch);
                    result.push_str("\x1b[0m"); // Reset
                    in_string = false;
                }
                '|' | '>' | '<' | '&' | ';' if !in_string => {
                    if !current_word.is_empty() {
                        result.push_str(&self.highlight_word(&current_word, is_first_word));
                        current_word.clear();
                        is_first_word = false;
                    }
                    result.push_str(self.colors.operator_ansi());
                    result.push(ch);
                    result.push_str("\x1b[0m");
                }
                c if c.is_whitespace() && !in_string => {
                    if !current_word.is_empty() {
                        result.push_str(&self.highlight_word(&current_word, is_first_word));
                        current_word.clear();
                        is_first_word = false;
                    }
                    result.push(c);
                }
                _ => {
                    if in_string {
                        result.push(ch);
                    } else {
                        current_word.push(ch);
                    }
                }
            }
        }

        if !current_word.is_empty() {
            result.push_str(&self.highlight_word(&current_word, is_first_word));
        }

        // Reset at end
        result.push_str("\x1b[0m");
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_builtin() {
        let highlighter = ShellHighlighter::new(ColorConfig::default());
        let result = highlighter.highlight("echo hello");
        assert!(result.contains("\x1b["));
    }

    #[test]
    fn test_highlight_string() {
        let highlighter = ShellHighlighter::new(ColorConfig::default());
        let result = highlighter.highlight("echo \"hello world\"");
        assert!(result.contains("\x1b["));
    }
}
