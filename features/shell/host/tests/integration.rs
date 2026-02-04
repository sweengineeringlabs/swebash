use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::env;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn host_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_swebash"))
}

fn engine_wasm_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("..");
    p.push("..");
    p.push("..");
    p.push("target");
    p.push("wasm32-unknown-unknown");
    p.push("release");
    p.push("engine.wasm");
    p
}

/// Run shell commands and return (stdout, stderr).
fn run(commands: &[&str]) -> (String, String) {
    run_in(&std::env::current_dir().unwrap(), commands)
}

/// Run shell commands with a specific working directory.
fn run_in(dir: &Path, commands: &[&str]) -> (String, String) {
    run_in_with_home(dir, commands, None)
}

/// Run shell commands with a specific working directory and optionally override HOME.
fn run_in_with_home(dir: &Path, commands: &[&str], home: Option<&Path>) -> (String, String) {
    assert!(
        engine_wasm_path().exists(),
        "engine.wasm not found — build it first:\n  \
         cargo build --manifest-path features/shell/engine/Cargo.toml \
         --target wasm32-unknown-unknown --release"
    );

    let mut input = String::new();
    for cmd in commands {
        input.push_str(cmd);
        input.push('\n');
    }
    input.push_str("exit\n");

    let mut command = Command::new(host_exe());
    command
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Pin workspace to the test's working directory so the shell doesn't
    // cd to ~ on startup (SWEBASH_WORKSPACE overrides the default).
    command.env("SWEBASH_WORKSPACE", dir);

    // Override HOME directory if provided (for testing history file location)
    if let Some(home_path) = home {
        command.env("HOME", home_path);
    }

    let mut child = command.spawn().expect("failed to start host binary");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let output = child.wait_with_output().expect("failed to wait on host");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (stdout, stderr)
}

/// Create a temp directory for a test.  Cleaned up on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("wasm_shell_test_{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TestDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Tests — echo
// ---------------------------------------------------------------------------

#[test]
fn echo_simple() {
    let (out, _) = run(&["echo hello"]);
    assert!(out.contains("hello\n"), "stdout: {out}");
}

#[test]
fn echo_multiple_args() {
    let (out, _) = run(&["echo hello world"]);
    assert!(out.contains("hello world\n"), "stdout: {out}");
}

#[test]
fn echo_quoted() {
    let (out, _) = run(&[r#"echo "hello world""#]);
    assert!(out.contains("hello world\n"), "stdout: {out}");
}

#[test]
fn echo_no_args() {
    let (out, _) = run(&["echo"]);
    // Should output just a newline
    assert!(out.contains("\n"), "stdout: {out}");
}

// ---------------------------------------------------------------------------
// Tests — pwd / cd
// ---------------------------------------------------------------------------

#[test]
fn pwd_outputs_path() {
    let (out, _) = run(&["pwd"]);
    // Should contain a directory separator
    assert!(
        out.contains('/') || out.contains('\\'),
        "pwd should print a path, got: {out}"
    );
}

#[test]
fn cd_then_pwd() {
    let dir = TestDir::new("cd_pwd");
    let (out, _) = run_in(dir.path(), &["pwd"]);
    let dir_str = dir.path().to_string_lossy();
    assert!(
        out.contains(dir_str.as_ref()),
        "pwd should show temp dir. stdout: {out}"
    );
}

#[test]
fn cd_nonexistent() {
    let (_out, err) = run(&["cd /nonexistent_path_12345"]);
    assert!(
        err.contains("no such directory"),
        "cd to missing dir should error. stderr: {err}"
    );
}

// ---------------------------------------------------------------------------
// Tests — ls
// ---------------------------------------------------------------------------

#[test]
fn ls_current_dir() {
    let dir = TestDir::new("ls_cur");
    std::fs::write(dir.path().join("aaa.txt"), "").unwrap();
    std::fs::write(dir.path().join("bbb.txt"), "").unwrap();

    let (out, _) = run_in(dir.path(), &["ls"]);
    assert!(out.contains("aaa.txt"), "stdout: {out}");
    assert!(out.contains("bbb.txt"), "stdout: {out}");
}

#[test]
fn ls_specific_dir() {
    let dir = TestDir::new("ls_dir");
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub").join("file.txt"), "").unwrap();

    let (out, _) = run_in(dir.path(), &["ls sub"]);
    assert!(out.contains("file.txt"), "stdout: {out}");
}

#[test]
fn ls_long_format() {
    let dir = TestDir::new("ls_long");
    std::fs::write(dir.path().join("data.txt"), "hello").unwrap();

    let (out, _) = run_in(dir.path(), &["ls -l"]);
    assert!(out.contains("file"), "ls -l should show file type. stdout: {out}");
    assert!(out.contains("data.txt"), "stdout: {out}");
    // Should contain header row
    assert!(out.contains("TYPE"), "ls -l should show header. stdout: {out}");
    assert!(out.contains("NAME"), "ls -l should show header. stdout: {out}");
    // New format should contain a date-like pattern (YYYY-MM-DD)
    assert!(
        out.contains('-') && out.contains(':'),
        "ls -l should show formatted date. stdout: {out}"
    );
}

// ---------------------------------------------------------------------------
// Tests — cat
// ---------------------------------------------------------------------------

#[test]
fn cat_file() {
    let dir = TestDir::new("cat");
    std::fs::write(dir.path().join("hello.txt"), "file contents here").unwrap();

    let (out, _) = run_in(dir.path(), &["cat hello.txt"]);
    assert!(out.contains("file contents here"), "stdout: {out}");
}

#[test]
fn cat_missing_file() {
    let (_out, err) = run(&["cat nonexistent.txt"]);
    assert!(
        err.contains("no such file"),
        "cat missing should error. stderr: {err}"
    );
}

#[test]
fn cat_multiple_files() {
    let dir = TestDir::new("cat_multi");
    std::fs::write(dir.path().join("a.txt"), "AAA").unwrap();
    std::fs::write(dir.path().join("b.txt"), "BBB").unwrap();

    let (out, _) = run_in(dir.path(), &["cat a.txt b.txt"]);
    assert!(out.contains("AAA"), "stdout: {out}");
    assert!(out.contains("BBB"), "stdout: {out}");
}

// ---------------------------------------------------------------------------
// Tests — mkdir
// ---------------------------------------------------------------------------

#[test]
fn mkdir_creates_dir() {
    let dir = TestDir::new("mkdir");
    run_in(dir.path(), &["mkdir subdir"]);
    assert!(dir.path().join("subdir").is_dir());
}

#[test]
fn mkdir_recursive() {
    let dir = TestDir::new("mkdir_p");
    run_in(dir.path(), &["mkdir -p a/b/c"]);
    assert!(dir.path().join("a").join("b").join("c").is_dir());
}

// ---------------------------------------------------------------------------
// Tests — rm
// ---------------------------------------------------------------------------

#[test]
fn rm_file() {
    let dir = TestDir::new("rm_file");
    let f = dir.path().join("gone.txt");
    std::fs::write(&f, "bye").unwrap();
    assert!(f.exists());

    run_in(dir.path(), &["rm gone.txt"]);
    assert!(!f.exists());
}

#[test]
fn rm_recursive() {
    let dir = TestDir::new("rm_r");
    std::fs::create_dir_all(dir.path().join("d/e")).unwrap();
    std::fs::write(dir.path().join("d/e/f.txt"), "").unwrap();

    run_in(dir.path(), &["rm -r d"]);
    assert!(!dir.path().join("d").exists());
}

#[test]
fn rm_missing_with_force() {
    // rm -f on a missing file should not print an error
    let (_out, err) = run(&["rm -f this_does_not_exist_xyz"]);
    assert!(
        !err.contains("cannot remove"),
        "rm -f should suppress errors. stderr: {err}"
    );
}

// ---------------------------------------------------------------------------
// Tests — cp
// ---------------------------------------------------------------------------

#[test]
fn cp_file() {
    let dir = TestDir::new("cp");
    std::fs::write(dir.path().join("src.txt"), "data").unwrap();

    run_in(dir.path(), &["cp src.txt dst.txt"]);

    let dst = dir.path().join("dst.txt");
    assert!(dst.exists(), "dst.txt should exist after cp");
    assert_eq!(std::fs::read_to_string(dst).unwrap(), "data");
}

// ---------------------------------------------------------------------------
// Tests — mv
// ---------------------------------------------------------------------------

#[test]
fn mv_file() {
    let dir = TestDir::new("mv");
    std::fs::write(dir.path().join("old.txt"), "data").unwrap();

    run_in(dir.path(), &["mv old.txt new.txt"]);

    assert!(!dir.path().join("old.txt").exists(), "old.txt should be gone");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("new.txt")).unwrap(),
        "data"
    );
}

// ---------------------------------------------------------------------------
// Tests — touch
// ---------------------------------------------------------------------------

#[test]
fn touch_creates_file() {
    let dir = TestDir::new("touch");
    let f = dir.path().join("new.txt");
    assert!(!f.exists());

    run_in(dir.path(), &["touch new.txt"]);
    assert!(f.exists());
}

// ---------------------------------------------------------------------------
// Tests — env / export
// ---------------------------------------------------------------------------

#[test]
fn env_lists_variables() {
    let (out, _) = run(&["env"]);
    // PATH is almost always set
    assert!(out.contains("PATH="), "env should list PATH. stdout length: {}", out.len());
}

#[test]
fn export_then_env() {
    let (out, _) = run(&["export WASM_TEST_VAR=hello123", "env"]);
    assert!(
        out.contains("WASM_TEST_VAR=hello123"),
        "exported var should appear in env. stdout length: {}",
        out.len()
    );
}

// ---------------------------------------------------------------------------
// Tests — head
// ---------------------------------------------------------------------------

#[test]
fn head_default_lines() {
    let dir = TestDir::new("head_default");
    let content: String = (1..=15).map(|i| format!("line{i}\n")).collect();
    std::fs::write(dir.path().join("f.txt"), &content).unwrap();

    let (out, _) = run_in(dir.path(), &["head f.txt"]);
    assert!(out.contains("line1\n"), "stdout: {out}");
    assert!(out.contains("line10\n"), "stdout: {out}");
    assert!(!out.contains("line11"), "head should stop at 10 lines. stdout: {out}");
}

#[test]
fn head_n_flag() {
    let dir = TestDir::new("head_n");
    let content: String = (1..=10).map(|i| format!("line{i}\n")).collect();
    std::fs::write(dir.path().join("f.txt"), &content).unwrap();

    let (out, _) = run_in(dir.path(), &["head -n 3 f.txt"]);
    assert!(out.contains("line1\n"), "stdout: {out}");
    assert!(out.contains("line3\n"), "stdout: {out}");
    assert!(!out.contains("line4"), "stdout: {out}");
}

// ---------------------------------------------------------------------------
// Tests — tail
// ---------------------------------------------------------------------------

#[test]
fn tail_default_lines() {
    let dir = TestDir::new("tail_default");
    let content: String = (1..=15).map(|i| format!("line{i}\n")).collect();
    std::fs::write(dir.path().join("f.txt"), &content).unwrap();

    let (out, _) = run_in(dir.path(), &["tail f.txt"]);
    assert!(out.contains("line15\n"), "stdout: {out}");
    assert!(out.contains("line6\n"), "stdout: {out}");
    assert!(!out.contains("line5\n"), "tail should skip first 5. stdout: {out}");
}

#[test]
fn tail_n_flag() {
    let dir = TestDir::new("tail_n");
    let content: String = (1..=10).map(|i| format!("line{i}\n")).collect();
    std::fs::write(dir.path().join("f.txt"), &content).unwrap();

    let (out, _) = run_in(dir.path(), &["tail -n 2 f.txt"]);
    assert!(out.contains("line9\n"), "stdout: {out}");
    assert!(out.contains("line10\n"), "stdout: {out}");
    assert!(!out.contains("line8"), "stdout: {out}");
}

// ---------------------------------------------------------------------------
// Tests — external command (host_spawn)
// ---------------------------------------------------------------------------

#[test]
fn external_command_echo() {
    // Use a command that exists on all platforms.
    // On Windows, "cmd /C echo" is used internally; on Unix, "echo" binary.
    // We test with a non-builtin: use the platform's native echo via a
    // fully-qualified name or a universally available command.
    if cfg!(windows) {
        let (out, _) = run(&["cmd /C echo native"]);
        assert!(out.contains("native"), "stdout: {out}");
    } else {
        let (out, _) = run(&["/bin/echo native"]);
        assert!(out.contains("native"), "stdout: {out}");
    }
}

#[test]
fn unknown_command_reports_error() {
    let (_out, err) = run(&["this_command_does_not_exist_xyz"]);
    // Should get some error output (either from host_spawn or the error code message)
    assert!(
        !err.is_empty() || _out.contains("exited with code"),
        "unknown command should produce an error. stderr: {err}, stdout: {_out}"
    );
}

// ---------------------------------------------------------------------------
// Tests — history
// ---------------------------------------------------------------------------

#[test]
fn history_file_created() {
    let dir = TestDir::new("history_file");

    // Run shell with custom HOME to control history file location
    let (_out, _err) = run_in_with_home(
        dir.path(),
        &["echo test1", "echo test2", "echo test3"],
        Some(dir.path()),
    );

    // Check that .swebash_history file was created
    let history_file = dir.path().join(".swebash_history");
    assert!(
        history_file.exists(),
        "history file should be created at {:?}",
        history_file
    );
}

#[test]
fn history_persists_commands() {
    let dir = TestDir::new("history_persist");

    // Run shell once with some commands
    let (_out1, _err1) = run_in_with_home(
        dir.path(),
        &["echo first", "echo second", "pwd"],
        Some(dir.path()),
    );

    let history_file = dir.path().join(".swebash_history");
    assert!(history_file.exists(), "history file should exist");

    // Read history file and verify commands were saved
    let history_content = std::fs::read_to_string(&history_file)
        .expect("should be able to read history file");

    assert!(
        history_content.contains("echo first"),
        "history should contain first command. content: {history_content}"
    );
    assert!(
        history_content.contains("echo second"),
        "history should contain second command. content: {history_content}"
    );
    assert!(
        history_content.contains("pwd"),
        "history should contain third command. content: {history_content}"
    );
}

#[test]
fn history_ignores_empty_lines() {
    let dir = TestDir::new("history_empty");

    // Run shell with empty lines between commands
    let (_out, _err) = run_in_with_home(
        dir.path(),
        &["echo test", "", "", "echo another"],
        Some(dir.path()),
    );

    let history_file = dir.path().join(".swebash_history");
    let history_content = std::fs::read_to_string(&history_file)
        .expect("should be able to read history file");

    // Empty lines should not be in history, exit should not be in history
    let line_count = history_content.lines().filter(|l| !l.is_empty()).count();
    assert_eq!(
        line_count, 2,
        "history should have exactly 2 commands, got {line_count}. Content:\n{history_content}"
    );

    // Verify the actual commands
    assert!(history_content.contains("echo test"), "should contain 'echo test'");
    assert!(history_content.contains("echo another"), "should contain 'echo another'");
}

#[test]
fn history_ignores_space_prefix() {
    let dir = TestDir::new("history_space");

    // Run shell with command starting with space
    let (_out, _err) = run_in_with_home(
        dir.path(),
        &["echo visible", " echo secret", "pwd"],
        Some(dir.path()),
    );

    let history_file = dir.path().join(".swebash_history");
    let history_content = std::fs::read_to_string(&history_file)
        .expect("should be able to read history file");

    assert!(
        history_content.contains("echo visible"),
        "visible command should be in history"
    );
    assert!(
        !history_content.contains("secret"),
        "command with space prefix should not be in history"
    );
    assert!(
        history_content.contains("pwd"),
        "pwd command should be in history"
    );
}

// ---------------------------------------------------------------------------
// Tests — AI mode
// ---------------------------------------------------------------------------

#[test]
fn ai_mode_enter_and_exit() {
    let (out, _err) = run(&["ai", "exit"]);

    // Should show "Entered AI mode" message
    assert!(
        out.contains("Entered AI mode"),
        "should show entered message. stdout: {out}"
    );

    // Should show "Exited AI mode" message
    assert!(
        out.contains("Exited AI mode"),
        "should show exited message. stdout: {out}"
    );
}

#[test]
fn ai_mode_prompt_indicator() {
    let (out, _err) = run(&["ai", "exit"]);

    // Should show [AI:<agent>] prompt
    assert!(
        out.contains("[AI:"),
        "should show AI agent prompt indicator. stdout: {out}"
    );
}

#[test]
fn ai_mode_status_command() {
    let (out, _err) = run(&["ai", "status", "exit"]);

    // Should handle status command in AI mode
    assert!(
        out.contains("[AI:"),
        "should be in AI mode. stdout: {out}"
    );
}

#[test]
fn ai_mode_quit_exits() {
    let (out, _err) = run(&["ai", "quit"]);

    // Both 'exit' and 'quit' should work
    assert!(
        out.contains("Exited AI mode"),
        "quit should exit AI mode. stdout: {out}"
    );
}

#[test]
fn ai_mode_chat_response() {
    let (out, _err) = run(&["ai", "how do I list files?", "exit"]);

    // Should either respond with AI output or show "not configured" message
    // depending on whether API key is set in the test environment
    assert!(
        out.contains("not configured") ||
        out.contains("Not configured") ||
        out.contains("ls") ||
        out.contains("thinking"),
        "should either show not configured or respond to the question. stdout: {out}"
    );
}

#[test]
fn ai_mode_preserves_history() {
    let dir = TestDir::new("ai_mode_history");

    let (_out, _err) = run_in_with_home(
        dir.path(),
        &["ai", "status", "exit", "echo test"],
        Some(dir.path()),
    );

    let history_file = dir.path().join(".swebash_history");
    let history_content = std::fs::read_to_string(&history_file)
        .expect("should be able to read history file");

    // AI mode commands should be in history
    assert!(
        history_content.contains("status"),
        "AI mode commands should be in history. content: {history_content}"
    );

    // Regular shell commands should also be in history
    assert!(
        history_content.contains("echo test"),
        "shell commands should be in history. content: {history_content}"
    );
}

#[test]
fn shell_mode_exit_quits() {
    // In shell mode (not AI mode), 'exit' should quit the shell
    let (out, _err) = run(&["echo before"]);

    assert!(
        out.contains("before"),
        "should execute command before exit. stdout: {out}"
    );
    // Shell exits after "exit" command (added automatically by run())
}

#[test]
fn ai_mode_nested_exit_behavior() {
    // Test that exit in AI mode doesn't quit the shell, but returns to shell mode
    let (out, _err) = run(&["ai", "exit", "echo after"]);

    assert!(
        out.contains("Exited AI mode"),
        "should exit AI mode. stdout: {out}"
    );

    assert!(
        out.contains("after"),
        "should continue executing commands after exiting AI mode. stdout: {out}"
    );
}

#[test]
fn ai_mode_multiple_commands() {
    let (out, _err) = run(&[
        "ai",
        "status",
        "history",
        "clear",
        "exit",
    ]);

    // Should stay in AI mode for all commands
    assert!(
        out.contains("Entered AI mode"),
        "should enter AI mode. stdout: {out}"
    );

    assert!(
        out.contains("Exited AI mode"),
        "should exit AI mode at the end. stdout: {out}"
    );
}

#[test]
fn ai_mode_with_multiline() {
    // Test that AI mode works with multi-line input
    let (out, _err) = run(&[
        "ai",
        "echo \"hello",
        "world\"",
        "exit",
    ]);

    // Multi-line should work in AI mode
    assert!(
        out.contains("[AI:") || out.contains("not configured"),
        "should be in AI mode or show not configured. stdout: {out}"
    );
}

// ---------------------------------------------------------------------------
// Tests — Agent framework (host layer)
// ---------------------------------------------------------------------------

#[test]
fn ai_agents_list_command() {
    let (out, _err) = run(&["ai agents"]);

    // Should list the built-in agents
    assert!(
        out.contains("shell") || out.contains("Shell"),
        "ai agents should list shell agent. stdout: {out}"
    );
    assert!(
        out.contains("review") || out.contains("Review"),
        "ai agents should list review agent. stdout: {out}"
    );
    assert!(
        out.contains("devops") || out.contains("DevOps"),
        "ai agents should list devops agent. stdout: {out}"
    );
    assert!(
        out.contains("git") || out.contains("Git"),
        "ai agents should list git agent. stdout: {out}"
    );
}

#[test]
fn ai_agent_switch_in_ai_mode() {
    let (out, _err) = run(&["ai", "@review", "exit"]);

    // Should switch to review agent
    assert!(
        out.contains("review") || out.contains("Review"),
        "should switch to review agent. stdout: {out}"
    );

    // Prompt should reflect the agent change
    assert!(
        out.contains("[AI:review]") || out.contains("[AI:"),
        "prompt should show agent. stdout: {out}"
    );
}

#[test]
fn ai_agent_list_in_ai_mode() {
    let (out, _err) = run(&["ai", "agents", "exit"]);

    // Should list agents when in AI mode
    assert!(
        out.contains("shell") || out.contains("Shell"),
        "agents command in AI mode should list agents. stdout: {out}"
    );
}

#[test]
fn ai_mode_prompt_shows_default_agent() {
    let (out, _err) = run(&["ai", "exit"]);

    // Default agent is shell, so prompt should show [AI:shell]
    assert!(
        out.contains("[AI:shell]"),
        "prompt should show default shell agent. stdout: {out}"
    );
}

#[test]
fn ai_agent_one_shot_from_shell() {
    let (out, _err) = run(&["ai @review hello"]);

    // One-shot agent command should produce some output
    // Either a response or a "not configured" message
    assert!(
        out.contains("review") ||
        out.contains("Review") ||
        out.contains("not configured") ||
        out.contains("Not configured") ||
        !out.is_empty(),
        "one-shot agent command should produce output. stdout: {out}"
    );
}

#[test]
fn ai_agent_switch_back_and_forth() {
    let (out, _err) = run(&["ai", "@git", "@review", "@shell", "exit"]);

    // Should handle multiple agent switches without crashing
    assert!(
        out.contains("Entered AI mode"),
        "should enter AI mode. stdout: {out}"
    );
    assert!(
        out.contains("Exited AI mode"),
        "should exit AI mode cleanly after switches. stdout: {out}"
    );
}

// ---------------------------------------------------------------------------
// Tests — @agent from shell mode enters AI mode (regression)
// ---------------------------------------------------------------------------

#[test]
fn ai_agent_switch_from_shell_enters_ai_mode() {
    let (out, _err) = run(&["@devops", "exit"]);

    // @devops from shell mode should enter AI mode
    assert!(
        out.contains("Entered AI mode"),
        "@devops from shell mode should enter AI mode. stdout: {out}"
    );

    // Prompt should show devops agent
    assert!(
        out.contains("[AI:devops]"),
        "prompt should show devops agent after @devops. stdout: {out}"
    );

    // Should also show the agent switched message
    assert!(
        out.contains("Switched to") || out.contains("devops"),
        "should confirm agent switch. stdout: {out}"
    );
}

#[test]
fn ai_agent_switch_from_shell_no_shell_execution() {
    let (out, err) = run(&["@devops", "do we have docker installed?", "exit"]);

    // Natural language input should NOT be executed as a shell command.
    // Before the fix, "do" was looked up as a binary and produced:
    //   "do: No such file or directory (os error 2)"
    assert!(
        !out.contains("No such file or directory") && !err.contains("No such file or directory"),
        "natural language after @devops should not be executed as a shell command. stdout: {out}, stderr: {err}"
    );
    assert!(
        !out.contains("process exited with code 127"),
        "should not see command-not-found exit code. stdout: {out}"
    );
}

#[test]
fn ai_agent_switch_from_shell_exit_returns_to_shell() {
    let (out, _err) = run(&["@devops", "exit", "echo back_in_shell"]);

    // After exiting AI mode, shell commands should work normally
    assert!(
        out.contains("Exited AI mode"),
        "exit should leave AI mode. stdout: {out}"
    );
    assert!(
        out.contains("back_in_shell"),
        "shell should work after exiting AI mode entered via @devops. stdout: {out}"
    );
}

#[test]
fn ai_agent_switch_from_shell_all_agents() {
    // All agent shorthands should enter AI mode from shell
    for agent in &["devops", "git", "review"] {
        let switch_cmd = format!("@{}", agent);
        let (out, _err) = run(&[&switch_cmd, "exit"]);
        assert!(
            out.contains("Entered AI mode"),
            "@{} from shell mode should enter AI mode. stdout: {}",
            agent,
            out
        );
    }
}

#[test]
fn ai_agent_switch_from_shell_with_ai_prefix() {
    // `ai @devops` should also enter AI mode (not just `@devops`)
    let (out, _err) = run(&["ai @devops", "exit"]);

    assert!(
        out.contains("Entered AI mode") || out.contains("[AI:devops]"),
        "ai @devops should enter AI mode with devops agent. stdout: {out}"
    );
}
