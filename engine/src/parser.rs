// ---------------------------------------------------------------------------
// Command-line parser: tokenizes input into command name + arguments
// ---------------------------------------------------------------------------

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

pub struct ParsedCommand {
    pub name: String,
    pub args: Vec<String>,
}

/// Parse input into a command name and argument list.
///
/// Handles:
///   - Simple tokens separated by whitespace
///   - Double-quoted strings: "hello world"
///   - Single-quoted strings: 'hello world'
///   - Backslash escapes: hello\ world
///   - Empty quoted strings: echo "" produces ["echo", ""]
pub fn parse(input: &str) -> Option<ParsedCommand> {
    let tokens = tokenize(input);
    if tokens.is_empty() {
        return None;
    }
    let name = tokens[0].clone();
    let args = tokens[1..].to_vec();
    Some(ParsedCommand { name, args })
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_token = false; // tracks whether we've started a token (including via quotes)

    while let Some(&ch) = chars.peek() {
        if in_single_quote {
            chars.next();
            if ch == '\'' {
                in_single_quote = false;
            } else {
                current.push(ch);
            }
        } else if in_double_quote {
            chars.next();
            if ch == '"' {
                in_double_quote = false;
            } else if ch == '\\' {
                if let Some(&next) = chars.peek() {
                    chars.next();
                    current.push(next);
                }
            } else {
                current.push(ch);
            }
        } else {
            match ch {
                '\'' => {
                    chars.next();
                    in_single_quote = true;
                    in_token = true;
                }
                '"' => {
                    chars.next();
                    in_double_quote = true;
                    in_token = true;
                }
                '\\' => {
                    chars.next();
                    in_token = true;
                    if let Some(&next) = chars.peek() {
                        chars.next();
                        current.push(next);
                    }
                }
                ' ' | '\t' => {
                    chars.next();
                    if in_token {
                        tokens.push(core::mem::take(&mut current));
                        in_token = false;
                    }
                }
                _ => {
                    chars.next();
                    current.push(ch);
                    in_token = true;
                }
            }
        }
    }

    if in_token {
        tokens.push(current);
    }

    tokens
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn names_and_args(input: &str) -> Option<(String, Vec<String>)> {
        parse(input).map(|p| (p.name, p.args))
    }

    // -- basic commands -------------------------------------------------------

    #[test]
    fn simple_command() {
        let (name, args) = names_and_args("ls").unwrap();
        assert_eq!(name, "ls");
        assert!(args.is_empty());
    }

    #[test]
    fn command_with_args() {
        let (name, args) = names_and_args("ls -la /tmp").unwrap();
        assert_eq!(name, "ls");
        assert_eq!(args, vec!["-la", "/tmp"]);
    }

    #[test]
    fn multiple_args() {
        let (name, args) = names_and_args("cp src.txt dst.txt").unwrap();
        assert_eq!(name, "cp");
        assert_eq!(args, vec!["src.txt", "dst.txt"]);
    }

    // -- quoting --------------------------------------------------------------

    #[test]
    fn double_quotes() {
        let (name, args) = names_and_args(r#"echo "hello world""#).unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec!["hello world"]);
    }

    #[test]
    fn single_quotes() {
        let (name, args) = names_and_args("echo 'hello world'").unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec!["hello world"]);
    }

    #[test]
    fn empty_double_quotes() {
        let (name, args) = names_and_args(r#"echo """#).unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec![""]);
    }

    #[test]
    fn empty_single_quotes() {
        let (name, args) = names_and_args("echo ''").unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec![""]);
    }

    #[test]
    fn adjacent_quoted_segments() {
        let (name, args) = names_and_args(r#"echo "hello"" world""#).unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec!["hello world"]);
    }

    #[test]
    fn mixed_quote_styles() {
        let (name, args) =
            names_and_args(r#"cmd "arg one" 'arg two' plain"#).unwrap();
        assert_eq!(name, "cmd");
        assert_eq!(args, vec!["arg one", "arg two", "plain"]);
    }

    // -- escaping -------------------------------------------------------------

    #[test]
    fn backslash_escape_space() {
        let (name, args) = names_and_args(r"echo hello\ world").unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec!["hello world"]);
    }

    #[test]
    fn escape_inside_double_quotes() {
        let (name, args) = names_and_args(r#"echo "hello \"world\"""#).unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec![r#"hello "world""#]);
    }

    #[test]
    fn single_quotes_preserve_backslash() {
        let (name, args) = names_and_args(r"echo 'hello\nworld'").unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec![r"hello\nworld"]);
    }

    // -- whitespace handling --------------------------------------------------

    #[test]
    fn empty_input() {
        assert!(parse("").is_none());
    }

    #[test]
    fn whitespace_only() {
        assert!(parse("   ").is_none());
    }

    #[test]
    fn tabs_and_extra_spaces() {
        let (name, args) = names_and_args("  echo\t hello  ").unwrap();
        assert_eq!(name, "echo");
        assert_eq!(args, vec!["hello"]);
    }

    #[test]
    fn leading_trailing_whitespace() {
        let (name, args) = names_and_args("   ls -l   ").unwrap();
        assert_eq!(name, "ls");
        assert_eq!(args, vec!["-l"]);
    }

    // -- realistic commands ---------------------------------------------------

    #[test]
    fn export_key_value() {
        let (name, args) = names_and_args("export FOO=bar").unwrap();
        assert_eq!(name, "export");
        assert_eq!(args, vec!["FOO=bar"]);
    }

    #[test]
    fn head_with_flag() {
        let (name, args) = names_and_args("head -n 5 file.txt").unwrap();
        assert_eq!(name, "head");
        assert_eq!(args, vec!["-n", "5", "file.txt"]);
    }

    #[test]
    fn path_with_spaces() {
        let (name, args) =
            names_and_args(r#"cat "my documents/file.txt""#).unwrap();
        assert_eq!(name, "cat");
        assert_eq!(args, vec!["my documents/file.txt"]);
    }

    #[test]
    fn git_commit_message() {
        let (name, args) =
            names_and_args(r#"git commit -m "initial commit""#).unwrap();
        assert_eq!(name, "git");
        assert_eq!(args, vec!["commit", "-m", "initial commit"]);
    }
}
