use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn host_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_host"))
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

/// Run shell commands and return (stdout, stderr).
fn run(commands: &[&str]) -> (String, String) {
    run_in(&std::env::current_dir().unwrap(), commands)
}

/// Run shell commands with a specific working directory.
fn run_in(dir: &Path, commands: &[&str]) -> (String, String) {
    assert!(
        engine_wasm_path().exists(),
        "engine.wasm not found — build it first:\n  \
         cargo build --manifest-path engine/Cargo.toml \
         --target wasm32-unknown-unknown --release"
    );

    let mut input = String::new();
    for cmd in commands {
        input.push_str(cmd);
        input.push('\n');
    }
    input.push_str("exit\n");

    let mut child = Command::new(host_exe())
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start host binary");

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
