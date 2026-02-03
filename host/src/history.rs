use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Simple command history manager
pub struct History {
    commands: Vec<String>,
    max_size: usize,
    file_path: Option<PathBuf>,
}

impl History {
    pub fn new(max_size: usize) -> Self {
        Self {
            commands: Vec::new(),
            max_size,
            file_path: None,
        }
    }

    /// Create history with file persistence
    pub fn with_file(max_size: usize, file_path: PathBuf) -> Self {
        let mut history = Self {
            commands: Vec::new(),
            max_size,
            file_path: Some(file_path.clone()),
        };

        // Load existing history
        if let Err(e) = history.load_from_file(&file_path) {
            eprintln!("[history] failed to load history: {}", e);
        }

        history
    }

    /// Add a command to history
    pub fn add(&mut self, command: String) {
        // Don't add empty commands or commands that start with space
        if command.trim().is_empty() || command.starts_with(' ') {
            return;
        }

        // Don't add duplicates of the last command
        if let Some(last) = self.commands.last() {
            if last == &command {
                return;
            }
        }

        self.commands.push(command);

        // Enforce max size
        if self.commands.len() > self.max_size {
            self.commands.remove(0);
        }
    }

    /// Get command by index (0 = oldest, len-1 = newest)
    pub fn get(&self, index: usize) -> Option<&String> {
        self.commands.get(index)
    }

    /// Get the number of commands in history
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get all commands as a slice
    pub fn commands(&self) -> &[String] {
        &self.commands
    }

    /// Load history from file
    fn load_from_file(&mut self, path: &PathBuf) -> std::io::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            if let Ok(cmd) = line {
                if !cmd.trim().is_empty() {
                    self.commands.push(cmd);
                }
            }
        }

        // Enforce max size after loading
        while self.commands.len() > self.max_size {
            self.commands.remove(0);
        }

        Ok(())
    }

    /// Save history to file
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(ref path) = self.file_path {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?;

            for cmd in &self.commands {
                writeln!(file, "{}", cmd)?;
            }

            file.flush()?;
        }
        Ok(())
    }
}

impl Drop for History {
    fn drop(&mut self) {
        // Auto-save on drop
        if let Err(e) = self.save() {
            eprintln!("[history] failed to save history: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_add_command() {
        let mut history = History::new(100);
        history.add("echo test".to_string());
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0), Some(&"echo test".to_string()));
    }

    #[test]
    fn test_ignore_empty() {
        let mut history = History::new(100);
        history.add("".to_string());
        history.add("   ".to_string());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_ignore_space_prefix() {
        let mut history = History::new(100);
        history.add(" secret command".to_string());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_ignore_duplicate_last() {
        let mut history = History::new(100);
        history.add("echo test".to_string());
        history.add("echo test".to_string());
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_max_size() {
        let mut history = History::new(3);
        history.add("cmd1".to_string());
        history.add("cmd2".to_string());
        history.add("cmd3".to_string());
        history.add("cmd4".to_string());
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0), Some(&"cmd2".to_string()));
        assert_eq!(history.get(2), Some(&"cmd4".to_string()));
    }

    #[test]
    fn test_persistence() {
        let temp_dir = std::env::temp_dir();
        let history_file = temp_dir.join(format!("test_swebash_history_{}", std::process::id()));

        // Remove old file if exists
        let _ = fs::remove_file(&history_file);

        // Create history and add commands
        {
            let mut history = History::with_file(100, history_file.clone());
            history.add("echo first".to_string());
            history.add("echo second".to_string());
            history.add("pwd".to_string());
        } // Drop saves history

        // Load in new instance
        let history = History::with_file(100, history_file.clone());
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0), Some(&"echo first".to_string()));
        assert_eq!(history.get(1), Some(&"echo second".to_string()));
        assert_eq!(history.get(2), Some(&"pwd".to_string()));

        // Cleanup
        let _ = fs::remove_file(history_file);
    }
}
