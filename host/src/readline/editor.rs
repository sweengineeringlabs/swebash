use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, ClearType},
};
use std::io::{self, Write};

use crate::history::History;
use super::{Hinter, ReadlineConfig};

/// Control flow for key event handling
enum ControlFlow {
    Continue,
    Submit,
    Eof,
}

/// Calculate the visible width of a string, excluding ANSI escape sequences.
///
/// ANSI codes like `\x1b[1;32m` (colors, bold, etc.) don't take up space on the terminal,
/// but are counted by `.chars().count()`. This function strips them to get the actual
/// display width.
fn visible_width(s: &str) -> usize {
    let mut count = 0;
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip ANSI escape sequence
            // Format: ESC [ <params> <command>
            // or: ESC <command> (for simpler sequences)
            if chars.as_str().starts_with('[') {
                // CSI sequence: skip until we hit a letter (the command)
                chars.next(); // consume '['
                while let Some(c) = chars.next() {
                    if c.is_ascii_alphabetic() || c == 'm' {
                        break;
                    }
                }
            } else {
                // Simple escape sequence, skip next char
                chars.next();
            }
        } else {
            // Regular visible character
            count += 1;
        }
    }

    count
}

/// Line editor with arrow key support
pub struct LineEditor {
    buffer: String,
    cursor: usize,
    history_pos: Option<usize>,
    saved_buffer: Option<String>,
    config: ReadlineConfig,
    hinter: Hinter,
}

impl LineEditor {
    pub fn new(config: ReadlineConfig, hinter: Hinter) -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            history_pos: None,
            saved_buffer: None,
            config,
            hinter,
        }
    }

    /// Read a line with full readline support
    pub fn read_line(&mut self, prompt: &str, history: &History) -> Result<Option<String>> {
        // Check if stdin is a terminal (interactive mode)
        if crossterm::tty::IsTty::is_tty(&std::io::stdin()) {
            // Interactive mode: use raw terminal
            terminal::enable_raw_mode()?;
            let result = self.read_line_raw(prompt, history);
            let _ = terminal::disable_raw_mode();
            result
        } else {
            // Non-interactive mode: use simple line reading
            self.read_line_simple(prompt)
        }
    }

    /// Simple line reading for non-interactive mode (pipes, tests)
    fn read_line_simple(&mut self, prompt: &str) -> Result<Option<String>> {
        use std::io::{self, BufRead};

        print!("{}", prompt);
        io::stdout().flush()?;

        let stdin = io::stdin();
        let mut line = String::new();
        let n = stdin.lock().read_line(&mut line)?;

        if n == 0 {
            return Ok(None);
        }

        // Trim newline but preserve leading/trailing spaces
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        Ok(Some(line))
    }

    fn read_line_raw(&mut self, prompt: &str, history: &History) -> Result<Option<String>> {
        // Initialize state
        self.buffer.clear();
        self.cursor = 0;
        self.history_pos = None;
        self.saved_buffer = None;

        // Initial render
        self.render(prompt, history)?;

        loop {
            // Read key event
            if let Event::Key(key_event) = event::read()? {
                match self.handle_key(key_event, history)? {
                    ControlFlow::Continue => {
                        self.render(prompt, history)?;
                    }
                    ControlFlow::Submit => {
                        // Move to new line (use \r\n for raw mode)
                        print!("\r\n");
                        io::stdout().flush()?;
                        return Ok(Some(self.buffer.clone()));
                    }
                    ControlFlow::Eof => {
                        // Move to new line (use \r\n for raw mode)
                        print!("\r\n");
                        io::stdout().flush()?;
                        return Ok(None);
                    }
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent, history: &History) -> Result<ControlFlow> {
        match (key.code, key.modifiers) {
            // Enter - submit line
            (KeyCode::Enter, _) => Ok(ControlFlow::Submit),

            // Ctrl-C - clear line or EOF if empty
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if self.buffer.is_empty() {
                    Ok(ControlFlow::Eof)
                } else {
                    self.buffer.clear();
                    self.cursor = 0;
                    self.history_pos = None;
                    Ok(ControlFlow::Continue)
                }
            }

            // Ctrl-D - EOF if empty, else delete char at cursor
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if self.buffer.is_empty() {
                    Ok(ControlFlow::Eof)
                } else if self.cursor < self.buffer.len() {
                    self.buffer.remove(self.cursor);
                    Ok(ControlFlow::Continue)
                } else {
                    Ok(ControlFlow::Continue)
                }
            }

            // Ctrl-A - move to start of line
            (KeyCode::Char('a'), KeyModifiers::CONTROL) | (KeyCode::Home, _) => {
                self.cursor = 0;
                Ok(ControlFlow::Continue)
            }

            // Ctrl-E - move to end of line
            (KeyCode::Char('e'), KeyModifiers::CONTROL) | (KeyCode::End, _) => {
                self.cursor = self.buffer.len();
                Ok(ControlFlow::Continue)
            }

            // Ctrl-U - clear line before cursor
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.buffer.drain(..self.cursor);
                self.cursor = 0;
                Ok(ControlFlow::Continue)
            }

            // Ctrl-K - clear line after cursor
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                self.buffer.truncate(self.cursor);
                Ok(ControlFlow::Continue)
            }

            // Ctrl-W - delete word before cursor
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                if self.cursor > 0 {
                    // Find start of current word
                    let mut pos = self.cursor;

                    // Skip trailing whitespace
                    while pos > 0 && self.buffer.chars().nth(pos - 1).map_or(false, |c| c.is_whitespace()) {
                        pos -= 1;
                    }

                    // Delete word
                    while pos > 0 && !self.buffer.chars().nth(pos - 1).map_or(false, |c| c.is_whitespace()) {
                        pos -= 1;
                    }

                    self.buffer.drain(pos..self.cursor);
                    self.cursor = pos;
                }
                Ok(ControlFlow::Continue)
            }

            // Arrow Up - previous history
            (KeyCode::Up, _) => {
                self.history_prev(history);
                Ok(ControlFlow::Continue)
            }

            // Arrow Down - next history
            (KeyCode::Down, _) => {
                self.history_next(history);
                Ok(ControlFlow::Continue)
            }

            // Arrow Left - move cursor left
            (KeyCode::Left, _) => {
                self.move_cursor_left();
                Ok(ControlFlow::Continue)
            }

            // Arrow Right - move cursor right
            (KeyCode::Right, _) => {
                self.move_cursor_right();
                Ok(ControlFlow::Continue)
            }

            // Backspace - delete char before cursor
            (KeyCode::Backspace, _) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.buffer.remove(self.cursor);
                }
                Ok(ControlFlow::Continue)
            }

            // Delete - delete char at cursor
            (KeyCode::Delete, _) => {
                if self.cursor < self.buffer.len() {
                    self.buffer.remove(self.cursor);
                }
                Ok(ControlFlow::Continue)
            }

            // Tab - accept hint if available, otherwise insert tab
            (KeyCode::Tab, _) => {
                // Check if there's a hint to accept (only at end of line)
                if self.cursor == self.buffer.len() {
                    if let Some(hint) = self.hinter.hint(&self.buffer, history) {
                        // Strip ANSI codes from hint to get the actual text
                        let hint_text = Self::strip_ansi(&hint);
                        // Append hint to buffer
                        self.buffer.push_str(&hint_text);
                        self.cursor = self.buffer.len();
                        return Ok(ControlFlow::Continue);
                    }
                }
                // No hint available, insert tab character
                self.buffer.insert(self.cursor, '\t');
                self.cursor += 1;
                Ok(ControlFlow::Continue)
            }

            // Regular character - insert at cursor
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.buffer.insert(self.cursor, c);
                self.cursor += 1;
                Ok(ControlFlow::Continue)
            }

            // Ignore other key combinations
            _ => Ok(ControlFlow::Continue),
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
        }
    }

    fn history_prev(&mut self, history: &History) {
        if history.is_empty() {
            return;
        }

        // Save current buffer on first history navigation
        if self.history_pos.is_none() {
            self.saved_buffer = Some(self.buffer.clone());
        }

        let new_pos = match self.history_pos {
            None => history.len() - 1,
            Some(pos) if pos > 0 => pos - 1,
            Some(_) => return, // Already at oldest
        };

        self.history_pos = Some(new_pos);
        if let Some(cmd) = history.get(new_pos) {
            self.buffer = cmd.clone();
            self.cursor = self.buffer.len();
        }
    }

    fn history_next(&mut self, history: &History) {
        match self.history_pos {
            None => return, // Not in history navigation
            Some(pos) if pos < history.len() - 1 => {
                // Move to next history entry
                let new_pos = pos + 1;
                self.history_pos = Some(new_pos);
                if let Some(cmd) = history.get(new_pos) {
                    self.buffer = cmd.clone();
                    self.cursor = self.buffer.len();
                }
            }
            Some(_) => {
                // Reached newest, restore saved buffer
                self.history_pos = None;
                if let Some(saved) = self.saved_buffer.take() {
                    self.buffer = saved;
                    self.cursor = self.buffer.len();
                }
            }
        }
    }

    /// Strip ANSI escape sequences from a string
    fn strip_ansi(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars();

        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // Skip ANSI escape sequence
                if chars.as_str().starts_with('[') {
                    // CSI sequence: skip until we hit a letter (the command)
                    chars.next(); // consume '['
                    while let Some(c) = chars.next() {
                        if c.is_ascii_alphabetic() || c == 'm' {
                            break;
                        }
                    }
                } else {
                    // Simple escape sequence, skip next char
                    chars.next();
                }
            } else {
                // Regular character
                result.push(ch);
            }
        }

        result
    }

    fn render(&self, prompt: &str, history: &History) -> Result<()> {
        let mut stdout = io::stdout();

        // Clear current line
        queue!(
            stdout,
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::CurrentLine),
        )?;

        // Print prompt
        queue!(stdout, Print(prompt))?;

        // Print buffer
        queue!(stdout, Print(&self.buffer))?;

        // Print hint if enabled and available
        if self.config.enable_hints && self.cursor == self.buffer.len() {
            if let Some(hint) = self.hinter.hint(&self.buffer, history) {
                queue!(stdout, Print(&hint))?;
            }
        }

        // Move cursor to correct position
        // Use visible width (excluding ANSI codes) for proper cursor positioning
        let cursor_col = visible_width(prompt) + self.cursor;
        queue!(stdout, cursor::MoveToColumn(cursor_col as u16))?;

        stdout.flush()?;
        Ok(())
    }
}

impl Drop for LineEditor {
    fn drop(&mut self) {
        // Ensure raw mode is disabled
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::History;
    use crate::readline::config::ColorConfig;

    fn create_test_editor() -> LineEditor {
        let config = ReadlineConfig::default();
        let hinter = Hinter::new(ColorConfig::default());
        LineEditor::new(config, hinter)
    }

    fn create_test_history() -> History {
        let mut history = History::new(100);
        history.add("echo first".to_string());
        history.add("echo second".to_string());
        history.add("echo third".to_string());
        history
    }

    #[test]
    fn test_editor_initialization() {
        let editor = create_test_editor();
        assert_eq!(editor.buffer, "");
        assert_eq!(editor.cursor, 0);
        assert_eq!(editor.history_pos, None);
        assert_eq!(editor.saved_buffer, None);
    }

    #[test]
    fn test_cursor_movement_left() {
        let mut editor = create_test_editor();
        editor.buffer = "hello".to_string();
        editor.cursor = 5;

        editor.move_cursor_left();
        assert_eq!(editor.cursor, 4);

        editor.move_cursor_left();
        assert_eq!(editor.cursor, 3);

        // Move to start
        editor.move_cursor_left();
        editor.move_cursor_left();
        editor.move_cursor_left();
        assert_eq!(editor.cursor, 0);

        // Should not go below 0
        editor.move_cursor_left();
        assert_eq!(editor.cursor, 0);
    }

    #[test]
    fn test_cursor_movement_right() {
        let mut editor = create_test_editor();
        editor.buffer = "hello".to_string();
        editor.cursor = 0;

        editor.move_cursor_right();
        assert_eq!(editor.cursor, 1);

        editor.move_cursor_right();
        assert_eq!(editor.cursor, 2);

        // Move to end
        editor.move_cursor_right();
        editor.move_cursor_right();
        editor.move_cursor_right();
        assert_eq!(editor.cursor, 5);

        // Should not go beyond buffer length
        editor.move_cursor_right();
        assert_eq!(editor.cursor, 5);
    }

    #[test]
    fn test_history_prev_navigation() {
        let mut editor = create_test_editor();
        let history = create_test_history();

        // Start with empty buffer
        assert_eq!(editor.buffer, "");
        assert_eq!(editor.history_pos, None);

        // Move to most recent (third)
        editor.history_prev(&history);
        assert_eq!(editor.buffer, "echo third");
        assert_eq!(editor.history_pos, Some(2));
        assert_eq!(editor.cursor, 10); // Cursor at end

        // Move to second
        editor.history_prev(&history);
        assert_eq!(editor.buffer, "echo second");
        assert_eq!(editor.history_pos, Some(1));

        // Move to first
        editor.history_prev(&history);
        assert_eq!(editor.buffer, "echo first");
        assert_eq!(editor.history_pos, Some(0));

        // Should not go below 0
        editor.history_prev(&history);
        assert_eq!(editor.buffer, "echo first");
        assert_eq!(editor.history_pos, Some(0));
    }

    #[test]
    fn test_history_next_navigation() {
        let mut editor = create_test_editor();
        let history = create_test_history();

        // Navigate to oldest first
        editor.history_prev(&history);
        editor.history_prev(&history);
        editor.history_prev(&history);
        assert_eq!(editor.buffer, "echo first");
        assert_eq!(editor.history_pos, Some(0));

        // Move forward
        editor.history_next(&history);
        assert_eq!(editor.buffer, "echo second");
        assert_eq!(editor.history_pos, Some(1));

        editor.history_next(&history);
        assert_eq!(editor.buffer, "echo third");
        assert_eq!(editor.history_pos, Some(2));

        // Move beyond newest should reset
        editor.history_next(&history);
        assert_eq!(editor.buffer, "");
        assert_eq!(editor.history_pos, None);
    }

    #[test]
    fn test_history_saves_current_buffer() {
        let mut editor = create_test_editor();
        let history = create_test_history();

        // Type something
        editor.buffer = "incomplete command".to_string();
        editor.cursor = editor.buffer.len();

        // Navigate history (should save buffer)
        editor.history_prev(&history);
        assert_eq!(editor.saved_buffer, Some("incomplete command".to_string()));
        assert_eq!(editor.buffer, "echo third");

        // Navigate back should restore
        editor.history_next(&history);
        editor.history_next(&history);
        assert_eq!(editor.buffer, "incomplete command");
        assert_eq!(editor.history_pos, None);
    }

    #[test]
    fn test_history_with_empty_history() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        // Should do nothing with empty history
        editor.history_prev(&history);
        assert_eq!(editor.buffer, "");
        assert_eq!(editor.history_pos, None);

        editor.history_next(&history);
        assert_eq!(editor.buffer, "");
        assert_eq!(editor.history_pos, None);
    }

    #[test]
    fn test_handle_key_enter() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "test command".to_string();

        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let result = editor.handle_key(key, &history).unwrap();

        match result {
            ControlFlow::Submit => (),
            _ => panic!("Expected Submit"),
        }
    }

    #[test]
    fn test_handle_key_ctrl_c_clears_buffer() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "test command".to_string();
        editor.cursor = 5;

        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let result = editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "");
        assert_eq!(editor.cursor, 0);
        match result {
            ControlFlow::Continue => (),
            _ => panic!("Expected Continue"),
        }
    }

    #[test]
    fn test_handle_key_ctrl_c_on_empty_is_eof() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "".to_string();

        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let result = editor.handle_key(key, &history).unwrap();

        match result {
            ControlFlow::Eof => (),
            _ => panic!("Expected Eof"),
        }
    }

    #[test]
    fn test_handle_key_ctrl_d_on_empty_is_eof() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let result = editor.handle_key(key, &history).unwrap();

        match result {
            ControlFlow::Eof => (),
            _ => panic!("Expected Eof"),
        }
    }

    #[test]
    fn test_handle_key_ctrl_d_deletes_at_cursor() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello".to_string();
        editor.cursor = 2; // At 'l'

        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "helo");
        assert_eq!(editor.cursor, 2);
    }

    #[test]
    fn test_handle_key_ctrl_a_home() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello".to_string();
        editor.cursor = 5;

        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.cursor, 0);
    }

    #[test]
    fn test_handle_key_ctrl_e_end() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello".to_string();
        editor.cursor = 0;

        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.cursor, 5);
    }

    #[test]
    fn test_handle_key_ctrl_u_clear_before() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello world".to_string();
        editor.cursor = 6; // After "hello "

        let key = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "world");
        assert_eq!(editor.cursor, 0);
    }

    #[test]
    fn test_handle_key_ctrl_k_clear_after() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello world".to_string();
        editor.cursor = 5; // After "hello"

        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "hello");
        assert_eq!(editor.cursor, 5);
    }

    #[test]
    fn test_handle_key_ctrl_w_delete_word() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "echo hello world".to_string();
        editor.cursor = 16; // At end

        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "echo hello ");
        assert_eq!(editor.cursor, 11);
    }

    #[test]
    fn test_handle_key_ctrl_w_with_spaces() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "echo test   ".to_string();
        editor.cursor = 12; // At end

        // Should delete trailing spaces
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "echo ");
        assert_eq!(editor.cursor, 5);
    }

    #[test]
    fn test_handle_key_backspace() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello".to_string();
        editor.cursor = 5;

        let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "hell");
        assert_eq!(editor.cursor, 4);
    }

    #[test]
    fn test_handle_key_backspace_at_start() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello".to_string();
        editor.cursor = 0;

        let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "hello"); // No change
        assert_eq!(editor.cursor, 0);
    }

    #[test]
    fn test_handle_key_delete() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello".to_string();
        editor.cursor = 0;

        let key = KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "ello");
        assert_eq!(editor.cursor, 0);
    }

    #[test]
    fn test_handle_key_char_insert() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hllo".to_string();
        editor.cursor = 1;

        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "hello");
        assert_eq!(editor.cursor, 2);
    }

    #[test]
    fn test_handle_key_char_append() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hell".to_string();
        editor.cursor = 4;

        let key = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "hello");
        assert_eq!(editor.cursor, 5);
    }

    #[test]
    fn test_handle_key_home() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello".to_string();
        editor.cursor = 5;

        let key = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.cursor, 0);
    }

    #[test]
    fn test_handle_key_end() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "hello".to_string();
        editor.cursor = 0;

        let key = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.cursor, 5);
    }

    #[test]
    fn test_handle_key_tab_with_hint() {
        let mut editor = create_test_editor();
        let mut history = History::new(100);

        // Add "ai" to history so it becomes a hint
        history.add("ai".to_string());

        // Type "a"
        editor.buffer = "a".to_string();
        editor.cursor = 1;

        // Press Tab - should accept hint and complete to "ai"
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "ai");
        assert_eq!(editor.cursor, 2);
    }

    #[test]
    fn test_handle_key_tab_without_hint() {
        let mut editor = create_test_editor();
        let history = History::new(100);

        editor.buffer = "test".to_string();
        editor.cursor = 4;

        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        editor.handle_key(key, &history).unwrap();

        assert_eq!(editor.buffer, "test\t");
        assert_eq!(editor.cursor, 5);
    }

    #[test]
    fn test_strip_ansi() {
        assert_eq!(LineEditor::strip_ansi("hello"), "hello");
        assert_eq!(LineEditor::strip_ansi("\x1b[1;32mhello\x1b[0m"), "hello");
        assert_eq!(LineEditor::strip_ansi("\x1b[36mai\x1b[0m"), "ai");
    }

    #[test]
    fn test_cursor_movement_with_unicode() {
        let mut editor = create_test_editor();
        editor.buffer = "hello 世界".to_string();
        editor.cursor = editor.buffer.len();

        // Move left through unicode
        editor.move_cursor_left();
        assert!(editor.cursor < editor.buffer.len());

        // Move right
        editor.move_cursor_right();
        assert_eq!(editor.cursor, editor.buffer.len());
    }

    #[test]
    fn test_visible_width_plain_text() {
        assert_eq!(visible_width("hello"), 5);
        assert_eq!(visible_width("~/swebash/> "), 12);
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn test_visible_width_with_ansi_codes() {
        // Green color: \x1b[1;32m, reset: \x1b[0m
        assert_eq!(visible_width("\x1b[1;32mhello\x1b[0m"), 5);

        // Actual shell prompt: green path + reset + "/> "
        assert_eq!(visible_width("\x1b[1;32m~/swebash\x1b[0m/> "), 12);

        // AI mode prompt: cyan [AI Mode] + reset + " > "
        assert_eq!(visible_width("\x1b[1;36m[AI Mode]\x1b[0m > "), 12);

        // Multi-line continuation prompt
        assert_eq!(visible_width("\x1b[1;32m...\x1b[0m> "), 5);
    }

    #[test]
    fn test_visible_width_multiple_ansi_codes() {
        // Multiple ANSI codes in sequence
        assert_eq!(visible_width("\x1b[1m\x1b[32mhello\x1b[0m"), 5);

        // ANSI codes with spaces
        assert_eq!(visible_width("\x1b[1;31merror:\x1b[0m test"), 11);
    }

    #[test]
    fn test_visible_width_empty_ansi() {
        // Just ANSI codes, no visible text
        assert_eq!(visible_width("\x1b[1;32m\x1b[0m"), 0);
    }
}
