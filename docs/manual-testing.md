# Manual Testing Guide

## Prerequisites

1. Rust toolchain with `wasm32-unknown-unknown` target installed
2. An LLM API key for AI feature testing (Anthropic, OpenAI, or Gemini)
3. `.env` file with credentials (see `.env.example`)

## Running the Shell

```bash
# Without AI features
cargo run

# With AI features (Anthropic)
set -a && source .env && set +a
export LLM_PROVIDER=anthropic
cargo run
```

## Test Checklist

### 1. Shell Basics

| Test | Command | Expected |
|------|---------|----------|
| Startup banner | `cargo run` | Prints `wasm-shell v0.1.0` and prompt |
| Echo | `echo hello world` | Prints `hello world` |
| PWD | `pwd` | Prints current working directory |
| LS | `ls` | Lists files in current directory |
| LS path | `ls /tmp` | Lists files in /tmp |
| LS long | `ls -l` | Long-format listing |
| Exit | `exit` | Shell exits cleanly |

### 2. Directory Navigation

| Test | Command | Expected |
|------|---------|----------|
| CD absolute | `cd /tmp` | Prompt updates to /tmp |
| CD relative | `cd ..` | Moves up one directory |
| CD nonexistent | `cd /no/such/dir` | Prints error, stays in current dir |
| PWD after CD | `cd /tmp && pwd` | Prints `/tmp` |

### 3. File Operations

| Test | Command | Expected |
|------|---------|----------|
| Touch | `touch /tmp/test_manual.txt` | Creates empty file |
| Cat | `cat /tmp/test_manual.txt` | Shows file contents (empty) |
| Cat missing | `cat /tmp/no_such_file` | Prints error |
| Head | `head -5 <file>` | Shows first 5 lines |
| Tail | `tail -5 <file>` | Shows last 5 lines |
| Mkdir | `mkdir /tmp/test_dir` | Creates directory |
| Mkdir recursive | `mkdir -p /tmp/a/b/c` | Creates nested directories |
| CP | `cp /tmp/test_manual.txt /tmp/copy.txt` | Copies file |
| MV | `mv /tmp/copy.txt /tmp/moved.txt` | Renames file |
| RM | `rm /tmp/moved.txt` | Deletes file |
| RM recursive | `rm -r /tmp/test_dir` | Deletes directory tree |
| RM force | `rm -f /tmp/no_such_file` | No error for missing file |

### 4. Environment Variables

| Test | Command | Expected |
|------|---------|----------|
| Export | `export FOO=bar` | Sets variable |
| Env | `env` | Lists all env vars (FOO=bar visible) |

### 5. External Commands

| Test | Command | Expected |
|------|---------|----------|
| External echo | `/bin/echo test` | Runs host system echo |
| Unknown command | `notarealcommand` | Prints "not recognized" error |

### 6. History

| Test | Steps | Expected |
|------|-------|----------|
| History file | Run a few commands, then exit | `~/.swebash_history` file exists |
| History persistence | Restart shell | Previous commands available via arrow keys |

---

## AI Feature Tests

> Requires `ANTHROPIC_API_KEY` (or equivalent) and `LLM_PROVIDER` set.

### 7. AI Status

| Test | Command | Expected |
|------|---------|----------|
| Status | `ai status` | Shows provider, model, enabled=yes, ready=yes |

### 8. AI Ask (NL to Command)

| Test | Command | Expected |
|------|---------|----------|
| Ask via subcommand | `ai ask list all files` | Suggests a command (e.g. `ls -la`), prompts Execute? |
| Ask via shorthand | `? find rust files` | Same behavior as `ai ask` |
| Cancel execution | Press `n` at Execute? prompt | Prints "Cancelled", returns to shell |

### 9. AI Explain

| Test | Command | Expected |
|------|---------|----------|
| Explain via subcommand | `ai explain ls -la` | Natural language explanation of the command |
| Explain via shorthand | `?? ps aux \| grep rust` | Explains the pipeline |
| Explain simple | `ai explain echo test` | Short explanation, no leading/trailing whitespace |

### 10. AI Chat Mode

| Test | Steps | Expected |
|------|-------|----------|
| Enter mode | Type `ai` | Prints "Entered AI mode", prompt changes to `[AI Mode] >` |
| Basic chat | Type a question | Shows "thinking...", then a response |
| Multi-turn memory | Say "My name is Alice", then "What is my name?" | Second reply mentions Alice |
| Exit mode | Type `quit` or `exit` | Prints "Exited AI mode", prompt returns to normal |

### 11. AI Chat from Shell

| Test | Command | Expected |
|------|---------|----------|
| Direct chat | `ai chat what is Rust?` | Prints AI response inline without entering AI mode |

### 12. AI Suggest

| Test | Command | Expected |
|------|---------|----------|
| Suggest | `ai suggest` | Shows autocomplete suggestions based on recent commands |

---

## Automated Test Suites

For reference, the automated tests cover these areas:

```bash
# Unit + integration tests (no API key needed)
cargo test

# AI integration tests against real API
set -a && source .env && set +a
cargo test --manifest-path ai/Cargo.toml
```

| Suite | Location | Count |
|-------|----------|-------|
| Unit tests | `host/src/` | 82 |
| Shell integration | `tests/integration.rs` | 43 |
| Readline integration | `tests/readline_tests.rs` | 19 |
| AI integration (real API) | `ai/tests/integration.rs` | 52 |
