# Rustyline Enhancements - Technical Design

**Audience**: Developers

## Overview

This document describes the technical design for implementing advanced rustyline features in swebash. These enhancements will provide a modern, user-friendly shell experience comparable to fish, zsh, or nushell.

## Architecture

All rustyline enhancements will be implemented in the host crate, as rustyline integration happens in the native REPL layer, not in the WASM engine.

```
host/src/
  ├── main.rs                 (rustyline Editor setup)
  └── readline/               (new module for rustyline features)
      ├── mod.rs             (module exports)
      ├── completer.rs       (Completer trait impl)
      ├── highlighter.rs     (Highlighter trait impl)
      ├── hinter.rs          (Hinter trait impl)
      ├── validator.rs       (Validator trait impl)
      ├── helper.rs          (Combined Helper impl)
      └── config.rs          (Configuration types)
```

## Phase 7: Tab Completion

### Design

Implement the `rustyline::completion::Completer` trait to provide context-aware completion.

### Implementation

```rust
// host/src/readline/completer.rs

use rustyline::completion::{Completer, Pair};
use rustyline::Context;
use std::path::PathBuf;

pub struct SwebashCompleter {
    builtin_commands: Vec<String>,
    history: Vec<String>,
}

impl SwebashCompleter {
    pub fn new() -> Self {
        Self {
            builtin_commands: vec![
                "echo", "pwd", "cd", "ls", "cat", "mkdir", "rm",
                "cp", "mv", "touch", "env", "export", "head", "tail",
                "ai", "exit"
            ].into_iter().map(String::from).collect(),
            history: Vec::new(),
        }
    }

    fn complete_command(&self, line: &str) -> Vec<Pair> {
        // Complete builtin commands
        self.builtin_commands.iter()
            .filter(|cmd| cmd.starts_with(line))
            .map(|cmd| Pair {
                display: cmd.clone(),
                replacement: cmd.clone(),
            })
            .collect()
    }

    fn complete_path(&self, line: &str, pos: usize) -> Vec<Pair> {
        // Extract the path component to complete
        let path_start = line[..pos].rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        let partial_path = &line[path_start..pos];

        // Expand ~ to home directory
        let expanded = if partial_path.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(&partial_path[2..]))
                .unwrap_or_else(|| PathBuf::from(partial_path))
        } else {
            PathBuf::from(partial_path)
        };

        // Get parent directory and filename prefix
        let (dir, prefix) = if let Some(parent) = expanded.parent() {
            (parent.to_path_buf(), expanded.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default())
        } else {
            (PathBuf::from("."), partial_path.to_string())
        };

        // Read directory and filter matches
        std::fs::read_dir(&dir)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(Result::ok))
            .filter(|entry| {
                entry.file_name()
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
                Pair {
                    display,
                    replacement: name,
                }
            })
            .collect()
    }
}

impl Completer for SwebashCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        // Determine what to complete based on position
        let before_cursor = &line[..pos];

        if before_cursor.trim().is_empty() || !before_cursor.contains(char::is_whitespace) {
            // Complete command
            Ok((0, self.complete_command(before_cursor)))
        } else {
            // Complete path/argument
            let start = before_cursor.rfind(char::is_whitespace)
                .map(|i| i + 1)
                .unwrap_or(0);
            Ok((start, self.complete_path(line, pos)))
        }
    }
}
```

### Integration

```rust
// host/src/main.rs

use readline::completer::SwebashCompleter;

let mut rl = Editor::<SwebashCompleter, _>::with_config(config)?;
rl.set_helper(Some(SwebashCompleter::new()));
```

### Testing

```rust
// host/tests/completer.rs

#[test]
fn test_command_completion() {
    let completer = SwebashCompleter::new();
    let (start, candidates) = completer.complete("ec", 2, &Context::new()).unwrap();
    assert_eq!(start, 0);
    assert!(candidates.iter().any(|c| c.display == "echo"));
}

#[test]
fn test_path_completion() {
    // Create temp dir with test files
    let dir = TestDir::new("completion");
    std::fs::write(dir.path().join("test.txt"), "").unwrap();
    std::fs::write(dir.path().join("test2.txt"), "").unwrap();

    let completer = SwebashCompleter::new();
    let line = format!("cat {}/test", dir.path().display());
    let (start, candidates) = completer.complete(&line, line.len(), &Context::new()).unwrap();

    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().any(|c| c.display.starts_with("test.txt")));
}
```

## Phase 8: Syntax Highlighting

### Design

Implement the `rustyline::highlight::Highlighter` trait to colorize commands in real-time.

### Color Scheme

```
Builtin commands:  Green (existing prompt color)
External commands: Blue
Invalid commands:  Red
Strings/quotes:    Yellow
File paths:        Cyan
Operators:         Magenta (|, >, <, &&, ||, ;)
```

### Implementation

```rust
// host/src/readline/highlighter.rs

use rustyline::highlight::Highlighter;
use std::borrow::Cow;

pub struct SwebashHighlighter {
    builtin_commands: Vec<String>,
}

impl SwebashHighlighter {
    pub fn new() -> Self {
        Self {
            builtin_commands: vec![
                "echo", "pwd", "cd", "ls", "cat", "mkdir", "rm",
                "cp", "mv", "touch", "env", "export", "head", "tail",
                "ai", "exit"
            ].into_iter().map(String::from).collect(),
        }
    }

    fn is_builtin(&self, word: &str) -> bool {
        self.builtin_commands.iter().any(|cmd| cmd == word)
    }

    fn is_valid_external(&self, word: &str) -> bool {
        // Check if command exists in PATH
        which::which(word).is_ok()
    }
}

impl Highlighter for SwebashHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
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
                    result.push_str("\x1b[33m"); // Yellow
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
                    result.push_str("\x1b[35m"); // Magenta
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

        Cow::Owned(result)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: bool) -> bool {
        true // Enable real-time highlighting
    }
}

impl SwebashHighlighter {
    fn highlight_word(&self, word: &str, is_command: bool) -> String {
        if !is_command {
            // Might be a file path or argument
            if word.starts_with('/') || word.starts_with("./") || word.starts_with("~/") {
                format!("\x1b[36m{}\x1b[0m", word) // Cyan for paths
            } else {
                word.to_string() // No color for regular arguments
            }
        } else if self.is_builtin(word) {
            format!("\x1b[32m{}\x1b[0m", word) // Green for builtins
        } else if self.is_valid_external(word) {
            format!("\x1b[34m{}\x1b[0m", word) // Blue for external commands
        } else {
            format!("\x1b[31m{}\x1b[0m", word) // Red for invalid
        }
    }
}
```

### Integration

```rust
// Combine with completer using Helper
use rustyline::Helper;

#[derive(Helper)]
pub struct SwebashHelper {
    completer: SwebashCompleter,
    highlighter: SwebashHighlighter,
}

impl Completer for SwebashHelper {
    type Candidate = Pair;
    fn complete(&self, line: &str, pos: usize, ctx: &Context)
        -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        self.completer.complete(line, pos, ctx)
    }
}

impl Highlighter for SwebashHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }
    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}
```

## Phase 9: History Hints

### Design

Implement the `rustyline::hint::Hinter` trait to show suggestions as you type.

### Implementation

```rust
// host/src/readline/hinter.rs

use rustyline::hint::Hinter;
use rustyline::Context;

pub struct SwebashHinter {
    history: Vec<String>,
}

impl SwebashHinter {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    pub fn update_history(&mut self, history: Vec<String>) {
        self.history = history;
    }
}

impl Hinter for SwebashHinter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        if pos < line.len() {
            return None; // Only hint at end of line
        }

        // Find most recent matching history entry
        self.history.iter()
            .rev() // Most recent first
            .find(|entry| entry.starts_with(line) && entry.len() > line.len())
            .map(|entry| {
                // Return the completion part (grayed out)
                format!("\x1b[90m{}\x1b[0m", &entry[line.len()..])
            })
    }
}
```

### User Experience

```bash
~/swebash/> echo he█
                  llo world  # Gray hint from history
# Press Right arrow or Ctrl-F to accept hint
~/swebash/> echo hello world█
```

## Phase 10: Vi Mode

### Design

Configure rustyline's built-in Vi mode support.

### Implementation

```rust
// host/src/readline/config.rs

use rustyline::{Config, EditMode};

pub struct SwebashConfig {
    pub edit_mode: EditMode,
    pub auto_add_history: bool,
    pub history_ignore_space: bool,
    pub max_history_size: usize,
}

impl Default for SwebashConfig {
    fn default() -> Self {
        Self {
            edit_mode: EditMode::Emacs,
            auto_add_history: true,
            history_ignore_space: true,
            max_history_size: 1000,
        }
    }
}

impl SwebashConfig {
    pub fn load() -> Self {
        // Load from ~/.swebashrc or use defaults
        Self::default()
    }

    pub fn to_rustyline_config(&self) -> Config {
        Config::builder()
            .edit_mode(self.edit_mode)
            .auto_add_history(self.auto_add_history)
            .history_ignore_space(self.history_ignore_space)
            .max_history_size(self.max_history_size)
            .build()
    }
}
```

### Prompt Mode Indicator

```rust
// Show mode in prompt for Vi users
let mode_indicator = match rl.helper() {
    Some(h) if h.config().edit_mode == EditMode::Vi => {
        if rl.is_in_vi_insert_mode() {
            " [I]"
        } else {
            " [N]"
        }
    }
    _ => ""
};
let prompt = format!("\x1b[1;32m{}\x1b[0m{}/> ", display_cwd, mode_indicator);
```

## Phase 11: Multi-line Editing

### Design

Implement the `rustyline::validate::Validator` trait to detect incomplete commands.

### Implementation

```rust
// host/src/readline/validator.rs

use rustyline::validate::{ValidationResult, ValidationContext, Validator};

pub struct SwebashValidator;

impl Validator for SwebashValidator {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        let input = ctx.input();

        // Check for trailing backslash
        if input.trim_end().ends_with('\\') {
            return Ok(ValidationResult::Incomplete);
        }

        // Check for unclosed quotes
        let mut in_quote = false;
        let mut quote_char = '\0';
        for ch in input.chars() {
            match ch {
                '"' | '\'' if !in_quote => {
                    in_quote = true;
                    quote_char = ch;
                }
                c if in_quote && c == quote_char => {
                    in_quote = false;
                }
                _ => {}
            }
        }

        if in_quote {
            return Ok(ValidationResult::Incomplete);
        }

        // Check for unclosed brackets (simple heuristic)
        let open_parens = input.chars().filter(|&c| c == '(').count();
        let close_parens = input.chars().filter(|&c| c == ')').count();
        if open_parens != close_parens {
            return Ok(ValidationResult::Incomplete);
        }

        Ok(ValidationResult::Valid(None))
    }
}
```

### User Experience

```bash
~/swebash/> echo "hello \
... world"
hello world

~/swebash/> echo (pwd | \
... head -1)
/home/user/swebash
```

## Phase 12: Configuration System

### Design

TOML-based configuration file at `~/.swebashrc`.

### Configuration Format

```toml
# ~/.swebashrc - swebash configuration

[readline]
# Editing mode: "emacs" or "vi"
edit_mode = "emacs"

# History settings
max_history_size = 1000
history_ignore_space = true
history_ignore_dups = true

# Features
enable_completion = true
enable_highlighting = true
enable_hints = true

# Color theme: "default", "solarized", "nord", etc.
color_theme = "default"

[readline.colors]
builtin_command = "green"
external_command = "blue"
invalid_command = "red"
string = "yellow"
path = "cyan"
operator = "magenta"
hint = "gray"

[readline.keybindings]
# Custom keybindings (GNU readline format)
# "\C-x\C-e" = "edit-and-execute-command"

[ai]
# AI-related settings
provider = "anthropic"
model = "claude-sonnet-4"
```

### Implementation

```rust
// host/src/readline/config.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
pub struct SwebashRcFile {
    #[serde(default)]
    pub readline: ReadlineConfig,
    #[serde(default)]
    pub ai: AiConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadlineConfig {
    #[serde(default = "default_edit_mode")]
    pub edit_mode: String,
    #[serde(default = "default_max_history")]
    pub max_history_size: usize,
    #[serde(default = "default_true")]
    pub history_ignore_space: bool,
    #[serde(default = "default_true")]
    pub enable_completion: bool,
    #[serde(default = "default_true")]
    pub enable_highlighting: bool,
    #[serde(default = "default_true")]
    pub enable_hints: bool,
    #[serde(default)]
    pub colors: ColorConfig,
}

impl Default for ReadlineConfig {
    fn default() -> Self {
        Self {
            edit_mode: default_edit_mode(),
            max_history_size: default_max_history(),
            history_ignore_space: true,
            enable_completion: true,
            enable_highlighting: true,
            enable_hints: true,
            colors: ColorConfig::default(),
        }
    }
}

impl SwebashRcFile {
    pub fn load() -> Self {
        let config_path = dirs::home_dir()
            .map(|h| h.join(".swebashrc"))
            .unwrap_or_else(|| PathBuf::from(".swebashrc"));

        if let Ok(content) = std::fs::read_to_string(&config_path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }
}
```

## Testing Strategy

### Unit Tests
- Test each trait implementation independently
- Mock history, file system, and command availability
- Test edge cases (empty input, special characters, etc.)

### Integration Tests
- Test combined helper with all features enabled
- Test configuration loading and application
- Test keybinding customization

### Manual Testing
- Create test scenarios document
- Test on different terminals (alacritty, kitty, gnome-terminal, etc.)
- Test Vi mode thoroughly (if Vi user available)
- Test multi-line with complex scenarios

## Performance Considerations

1. **Completion**: Cache directory listings, limit results
2. **Highlighting**: Only highlight visible portion in very long lines
3. **Hints**: Limit history search to most recent N entries
4. **Path Checking**: Use async or caching for command validation

## Compatibility

- Maintain compatibility with non-TTY mode (piped input)
- Graceful degradation when features unavailable
- Cross-platform support (Unix + Windows)

## Documentation Requirements

For each phase:
1. Update user guide with new features
2. Add examples and screenshots
3. Document configuration options
4. Add troubleshooting section

## Success Criteria

Each phase is complete when:
- [ ] All subtasks implemented
- [ ] Unit tests passing
- [ ] Integration tests passing
- [ ] Manual testing completed
- [ ] Documentation updated
- [ ] Code reviewed
