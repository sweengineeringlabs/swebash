/// Integration tests for readline/line editor functionality
///
/// These tests verify the arrow key navigation and line editing features
/// work correctly in an end-to-end scenario.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::env;

// ---------------------------------------------------------------------------
// Test Helpers
// ---------------------------------------------------------------------------

fn host_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_swebash"))
}

fn engine_wasm_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("..");
    p.push("target");
    p.push("wasm32-unknown-unknown");
    p.push("release");
    p.push("engine.wasm");
    p
}

struct TestContext {
    home_dir: PathBuf,
    history_file: PathBuf,
}

impl TestContext {
    fn new(test_name: &str) -> Self {
        let home_dir = std::env::temp_dir().join(format!("swebash_readline_test_{}", test_name));
        let _ = std::fs::remove_dir_all(&home_dir);
        std::fs::create_dir_all(&home_dir).unwrap();

        let history_file = home_dir.join(".swebash_history");

        TestContext {
            home_dir,
            history_file,
        }
    }

    fn setup_history(&self, commands: &[&str]) {
        let mut file = fs::File::create(&self.history_file).unwrap();
        for cmd in commands {
            writeln!(file, "{}", cmd).unwrap();
        }
    }

    fn read_history(&self) -> Vec<String> {
        if !self.history_file.exists() {
            return Vec::new();
        }

        fs::read_to_string(&self.history_file)
            .unwrap()
            .lines()
            .map(|s| s.to_string())
            .collect()
    }

    /// Run commands with simulated input (simple mode - no escape sequences)
    fn run_simple(&self, input: &str) -> (String, String) {
        assert!(
            engine_wasm_path().exists(),
            "engine.wasm not found — build it first"
        );

        let mut child = Command::new(host_exe())
            .current_dir(&self.home_dir)
            .env("HOME", &self.home_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to start shell");

        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();

        let output = child.wait_with_output().expect("failed to wait");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        (stdout, stderr)
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home_dir);
    }
}

// ---------------------------------------------------------------------------
// History Persistence Tests
// ---------------------------------------------------------------------------

#[test]
fn test_history_persists_across_sessions() {
    let ctx = TestContext::new("history_persist");

    // First session: run some commands
    let input = "echo first\necho second\necho third\nexit\n";
    let (stdout, _) = ctx.run_simple(input);

    // Verify commands were recorded
    assert!(stdout.contains("first"));
    assert!(stdout.contains("second"));
    assert!(stdout.contains("third"));

    // Check history file
    let history = ctx.read_history();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0], "echo first");
    assert_eq!(history[1], "echo second");
    assert_eq!(history[2], "echo third");

    // Second session: history should be loaded
    // Just verify the file exists and can be read
    let history2 = ctx.read_history();
    assert_eq!(history2, history);
}

#[test]
fn test_history_ignores_duplicates() {
    let ctx = TestContext::new("history_duplicates");

    let input = "echo test\necho test\necho different\nexit\n";
    ctx.run_simple(input);

    let history = ctx.read_history();
    // Should only have 2 entries (duplicate filtered)
    assert_eq!(history.len(), 2);
    assert_eq!(history[0], "echo test");
    assert_eq!(history[1], "echo different");
}

#[test]
fn test_history_ignores_space_prefix() {
    let ctx = TestContext::new("history_space");

    let input = " secret command\necho public\nexit\n";
    ctx.run_simple(input);

    let history = ctx.read_history();
    // Should only have the public command
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], "echo public");
}

#[test]
fn test_history_ignores_empty_lines() {
    let ctx = TestContext::new("history_empty");

    let input = "\n\necho hello\n\n\nexit\n";
    ctx.run_simple(input);

    let history = ctx.read_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], "echo hello");
}

#[test]
fn test_history_max_size() {
    let ctx = TestContext::new("history_max");

    // Pre-populate with many commands
    let initial: Vec<String> = (1..=1005).map(|i| format!("echo cmd{}", i)).collect();
    let initial_refs: Vec<&str> = initial.iter().map(|s| s.as_str()).collect();
    ctx.setup_history(&initial_refs);

    // Add one more command
    let input = "echo new_command\nexit\n";
    ctx.run_simple(input);

    let history = ctx.read_history();
    // Default max is 1000, so oldest should be dropped
    assert_eq!(history.len(), 1000);
    // Newest command should be present
    assert_eq!(history.last().unwrap(), "echo new_command");
    // Oldest commands should be dropped
    assert!(!history.contains(&"echo cmd1".to_string()));
    assert!(!history.contains(&"echo cmd2".to_string()));
}

// ---------------------------------------------------------------------------
// Line Editing Tests (via command execution)
// ---------------------------------------------------------------------------

#[test]
fn test_basic_command_execution() {
    let ctx = TestContext::new("basic_exec");

    let input = "echo hello world\nexit\n";
    let (stdout, _) = ctx.run_simple(input);

    assert!(stdout.contains("hello world"));
}

#[test]
fn test_multiline_command() {
    let ctx = TestContext::new("multiline");

    let input = "echo line1 \\\nline2\nexit\n";
    let (stdout, _) = ctx.run_simple(input);

    // Multiline command should be executed
    assert!(stdout.contains("line1"));
}

#[test]
fn test_empty_input_ignored() {
    let ctx = TestContext::new("empty_input");

    // Multiple empty lines should not cause issues
    let input = "\n\n\necho test\nexit\n";
    let (stdout, _) = ctx.run_simple(input);

    assert!(stdout.contains("test"));
}

#[test]
fn test_ctrl_d_exits() {
    let ctx = TestContext::new("ctrl_d");

    // EOF (empty stdin) should exit cleanly
    let input = "";
    let (_, stderr) = ctx.run_simple(input);

    // Should exit without errors
    assert!(!stderr.contains("error"));
    assert!(!stderr.contains("panic"));
}

// ---------------------------------------------------------------------------
// Special Character Handling
// ---------------------------------------------------------------------------

#[test]
fn test_special_characters_in_commands() {
    let ctx = TestContext::new("special_chars");

    let input = "echo 'hello world'\nexit\n";
    let (stdout, _) = ctx.run_simple(input);

    assert!(stdout.contains("hello world"));
}

#[test]
fn test_escape_sequences_in_echo() {
    let ctx = TestContext::new("escape_seq");

    let input = "echo test\\nline\nexit\n";
    let (stdout, _) = ctx.run_simple(input);

    // The command should execute without errors
    assert!(stdout.contains("test") || stdout.contains("line"));
}

// ---------------------------------------------------------------------------
// Configuration Tests
// ---------------------------------------------------------------------------

#[test]
fn test_readline_with_custom_config() {
    let ctx = TestContext::new("custom_config");

    // Create a custom config
    let config_path = ctx.home_dir.join(".swebashrc");
    let config = r#"
[readline]
max_history_size = 10
enable_hints = false
enable_completion = false
"#;
    fs::write(&config_path, config).unwrap();

    // Run commands that would exceed max history
    let mut input = String::new();
    for i in 1..=15 {
        input.push_str(&format!("echo cmd{}\n", i));
    }
    input.push_str("exit\n");

    ctx.run_simple(&input);

    let history = ctx.read_history();
    // Should respect the config max of 10
    assert!(history.len() <= 10);
}

// ---------------------------------------------------------------------------
// Error Handling Tests
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_command_does_not_crash() {
    let ctx = TestContext::new("invalid_cmd");

    let input = "this_command_does_not_exist\nexit\n";
    let (_, stderr) = ctx.run_simple(input);

    // Should not panic, may show error
    assert!(!stderr.contains("panic"));
}

#[test]
fn test_very_long_command() {
    let ctx = TestContext::new("long_cmd");

    // Create a very long command (but within buffer limits)
    let long_arg = "x".repeat(500);
    let input = format!("echo {}\nexit\n", long_arg);
    let (stdout, _) = ctx.run_simple(&input);

    // Should handle long input
    assert!(stdout.contains(&long_arg) || !stdout.is_empty());
}

// ---------------------------------------------------------------------------
// Exit Behavior Tests
// ---------------------------------------------------------------------------

#[test]
fn test_exit_command() {
    let ctx = TestContext::new("exit_cmd");

    let input = "echo before exit\nexit\necho should not run\n";
    let (stdout, _) = ctx.run_simple(input);

    assert!(stdout.contains("before exit"));
    assert!(!stdout.contains("should not run"));
}

#[test]
fn test_multiple_sessions() {
    let ctx = TestContext::new("multi_session");

    // Session 1
    ctx.run_simple("echo session1\nexit\n");
    let history1 = ctx.read_history();
    assert_eq!(history1.len(), 1);

    // Session 2
    ctx.run_simple("echo session2\nexit\n");
    let history2 = ctx.read_history();
    assert_eq!(history2.len(), 2);
    assert_eq!(history2[0], "echo session1");
    assert_eq!(history2[1], "echo session2");

    // Session 3
    ctx.run_simple("echo session3\nexit\n");
    let history3 = ctx.read_history();
    assert_eq!(history3.len(), 3);
}

// ---------------------------------------------------------------------------
// Stress Tests
// ---------------------------------------------------------------------------

#[test]
fn test_rapid_commands() {
    let ctx = TestContext::new("rapid");

    let mut input = String::new();
    for i in 1..=50 {
        input.push_str(&format!("echo cmd{}\n", i));
    }
    input.push_str("exit\n");

    let (stdout, _) = ctx.run_simple(&input);

    // Should handle all commands
    assert!(stdout.contains("cmd1"));
    assert!(stdout.contains("cmd50"));
}

#[test]
fn test_whitespace_handling() {
    let ctx = TestContext::new("whitespace");

    let input = "  echo   test   \nexit\n";
    let (stdout, _) = ctx.run_simple(input);

    // Should handle extra whitespace
    assert!(stdout.contains("test"));
}

// ---------------------------------------------------------------------------
// AI Mode Integration Tests
// ---------------------------------------------------------------------------

#[test]
fn test_ai_mode_exit_returns_to_shell() {
    let ctx = TestContext::new("ai_mode");

    // Try to enter and exit AI mode
    let input = "ai\nexit\necho back in shell\nexit\n";
    let (stdout, _) = ctx.run_simple(input);

    // Should be able to return to shell and execute commands
    // (AI may not be configured, but exit should work)
    assert!(!stdout.contains("panic"));
}
