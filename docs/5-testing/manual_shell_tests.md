# Manual Shell Tests

> **TLDR:** Manual test checklist for shell basics: commands, file ops, environment, history, and workspace sandbox.

**Audience**: Developers, QA

**WHAT**: Manual test procedures for core shell features
**WHY**: Validates fundamental shell operations that underpin all higher-level features
**HOW**: Step-by-step test tables with expected outcomes

---

## Table of Contents

- [Shell Basics](#1-shell-basics)
- [Directory Navigation](#2-directory-navigation)
- [File Operations](#3-file-operations)
- [Environment Variables](#4-environment-variables)
- [External Commands](#5-external-commands)
- [History](#6-history)
- [Workspace Sandbox](#6b-workspace-sandbox)

---

## 1. Shell Basics

| Test | Command | Expected |
|------|---------|----------|
| Startup banner | `./sbh run` | Prints `wasm-shell v0.1.0` and prompt |
| Echo | `echo hello world` | Prints `hello world` |
| PWD | `pwd` | Prints current working directory |
| LS | `ls` | Lists files in current directory |
| LS path | `ls /tmp` | Lists files in /tmp |
| LS long | `ls -l` | Long-format listing |
| Exit | `exit` | Shell exits cleanly |

## 2. Directory Navigation

| Test | Command | Expected |
|------|---------|----------|
| CD absolute | `cd /tmp` | Prompt updates to /tmp |
| CD relative | `cd ..` | Moves up one directory |
| CD nonexistent | `cd /no/such/dir` | Prints error, stays in current dir |
| PWD after CD | `cd /tmp && pwd` | Prints `/tmp` |

## 3. File Operations

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

## 4. Environment Variables

| Test | Command | Expected |
|------|---------|----------|
| Export | `export FOO=bar` | Sets variable |
| Env | `env` | Lists all env vars (FOO=bar visible) |

## 5. External Commands

| Test | Command | Expected |
|------|---------|----------|
| External echo | `/bin/echo test` | Runs host system echo |
| Unknown command | `notarealcommand` | Prints "not recognized" error |

## 6. History

| Test | Steps | Expected |
|------|-------|----------|
| History file | Run a few commands, then exit | `~/.swebash_history` file exists |
| History persistence | Restart shell | Previous commands available via arrow keys |

## 6b. Workspace Sandbox

| Test | Command | Expected |
|------|---------|----------|
| Default workspace | `./sbh run` then `pwd` | Shows `~/workspace/` (auto-created if missing) |
| Sandbox status | `workspace` | Shows enabled, root path, allowed paths with modes |
| Read-only default | `touch foo` | Denied: `sandbox: write access denied for '...'` |
| Switch to RW | `workspace rw` then `touch foo` | File created successfully |
| Switch back to RO | `workspace ro` then `touch bar` | Denied |
| Allow path | `workspace allow /tmp rw` then `cd /tmp` then `touch test` | Allowed — /tmp added as RW |
| Deny outside | `cd /etc` | Denied: `sandbox: read access denied for '/etc': outside workspace` |
| Disable sandbox | `workspace disable` then `cd /etc` | Allowed — sandbox turned off |
| Re-enable sandbox | `workspace enable` then `cd /etc` | Denied again |
| Env var override | `SWEBASH_WORKSPACE=/tmp ./sbh run` then `pwd` | Shows `/tmp`; writes are allowed (RW mode) |
| Config file | Create `~/.config/swebash/config.toml` with `mode = "rw"`, restart | Workspace is read-write by default |
| LS in sandbox | `ls` | Works (read operation in workspace) |
| Cat in sandbox | `cat <file-in-workspace>` | Works (read operation in workspace) |

---

## See Also

- [Manual Testing Hub](manual_testing.md) — prerequisites and setup
- [Manual AI Tests](manual_ai_tests.md) — AI feature tests
- [Workspace Sandbox Design](../3-design/workspace_sandbox.md) — sandbox architecture
