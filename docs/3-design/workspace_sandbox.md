# Workspace Sandbox

> **TLDR:** Path-based access control layer in the host runtime that restricts filesystem operations to configured workspace paths.

**Audience**: Developers, architects

**WHAT**: A sandbox that controls which filesystem paths the shell can read from and write to
**WHY**: Prevents accidental modification of files outside the workspace; provides a configurable security boundary
**HOW**: Ordered path rules checked at the host import layer before any OS syscall

---

## Table of Contents

- [Overview](#overview)
- [Data Model](#data-model)
- [Configuration](#configuration)
- [Startup Sequence](#startup-sequence)
- [Access Classification](#access-classification)
- [Path Resolution](#path-resolution)
- [Runtime Overrides](#runtime-overrides)
- [Test Compatibility](#test-compatibility)
- [Module Layout](#module-layout)

## Overview

The workspace sandbox sits between the WASM engine's host imports and the OS filesystem. Every filesystem operation passes through `sandbox::check_path()` before reaching `std::fs`.

```
engine (WASM) ── host_read_file / host_write_file / ... ──▸ fs.rs
                                                              │
                                                     check_path(policy, path, Read|Write)
                                                              │
                                                     ┌───────┴────────┐
                                                     │  SandboxPolicy │
                                                     │  allowed_paths │
                                                     │  first match   │
                                                     └───────┬────────┘
                                                              │
                                                   allowed → std::fs::*
                                                   denied  → stderr msg, return -1
```

The WASM engine cannot bypass this layer — it has no direct OS access. All host imports go through the native host runtime where the sandbox policy is enforced.

## Data Model

```rust
// host/src/spi/state.rs

pub enum AccessMode { ReadOnly, ReadWrite }

pub struct PathRule {
    pub root: PathBuf,      // canonicalized
    pub mode: AccessMode,
}

pub struct SandboxPolicy {
    pub workspace_root: PathBuf,
    pub allowed_paths: Vec<PathRule>,  // first match wins; index 0 = workspace root
    pub enabled: bool,
}

pub struct HostState {
    pub response_buf_ptr: u32,
    pub response_buf_cap: u32,
    pub sandbox: SandboxPolicy,       // checked by every fs import
}
```

## Configuration

### Config File

`~/.config/swebash/config.toml`:

```toml
[workspace]
root = "~/.local/share/swebash/workspace"  # XDG-compliant default, supports ~ expansion
mode = "ro"                                 # "ro" or "rw"
enabled = true

[[workspace.allow]]
path = "~/projects"
mode = "rw"

[[workspace.allow]]
path = "/tmp"
mode = "rw"
```

Loaded by `config::load_config()` at startup. Falls back to defaults if the file is missing or malformed.

### Environment Variable

`SWEBASH_WORKSPACE` overrides the workspace root **and** defaults the mode to `ReadWrite`. This preserves backward compatibility with existing test infrastructure.

### Precedence

1. `SWEBASH_WORKSPACE` env var (if set, also forces RW)
2. `root` from config file
3. `~/.local/share/swebash/workspace/` (XDG-compliant default)

## Startup Sequence

In `main.rs`:

1. Load `.env` files (existing)
2. Load `~/.config/swebash/config.toml` via `config::load_config()`
3. Check for `SWEBASH_WORKSPACE` env var
4. Resolve workspace root (env > config > `~/.local/share/swebash/workspace/`)
5. Auto-create workspace directory if missing (`create_dir_all`)
6. Build `SandboxPolicy` via `config.into_policy()`
7. If `SWEBASH_WORKSPACE` was set via env var: override root + force RW mode
8. `set_current_dir(workspace_root)`
9. Pass policy to `runtime::setup(policy)`

## Access Classification

| Host Import | Check | Classification |
|-------------|-------|----------------|
| `host_read_file` | `check_path(path, Read)` | Read |
| `host_list_dir` | `check_path(path, Read)` | Read |
| `host_stat` | `check_path(path, Read)` | Read |
| `host_write_file` | `check_path(path, Write)` | Write |
| `host_remove` | `check_path(path, Write)` | Write |
| `host_mkdir` | `check_path(path, Write)` | Write |
| `host_copy` | `check_path(src, Read)` + `check_path(dst, Write)` | Read + Write |
| `host_rename` | `check_path(src, Write)` + `check_path(dst, Write)` | Write + Write |
| `host_set_cwd` | `check_path(path, Read)` | Read (must be in sandbox) |
| `host_get_cwd` | Always allowed | None |
| `host_spawn` | Verifies CWD is in sandbox | Read (CWD check) |

Denied operations print to stderr:
```
sandbox: write access denied for '/path': read-only workspace
sandbox: read access denied for '/path': outside workspace
```

## Path Resolution

`sandbox::resolve_path()` handles both absolute and relative paths:

1. If absolute: canonicalize (falls back to raw path if target doesn't exist yet)
2. If relative: join with CWD, then canonicalize

Canonicalization resolves symlinks and `..` components, preventing traversal attacks.

## Runtime Overrides

The `workspace` builtin communicates with the host via `host_workspace(cmd_ptr, cmd_len) -> i32`:

| Command | Effect |
|---------|--------|
| `workspace` / `workspace status` | Print policy status to response buffer |
| `workspace rw` | Set workspace root rule to `ReadWrite` |
| `workspace ro` | Set workspace root rule to `ReadOnly` |
| `workspace allow PATH [ro\|rw]` | Append a new `PathRule` |
| `workspace disable` | Set `policy.enabled = false` |
| `workspace enable` | Set `policy.enabled = true` |

Changes are session-scoped — they modify `HostState.sandbox` in-place and do not persist to the config file.

The `host_set_env` import warns if the user attempts to set `SWEBASH_WORKSPACE` at runtime, since the env var is only read during startup.

## Test Compatibility

All existing integration tests set `SWEBASH_WORKSPACE` via env var pointing at test directories. The key rule:

> When `SWEBASH_WORKSPACE` is set via env var, default that path to **RW mode**.

This ensures all existing write tests (`touch`, `cp`, `mv`, `rm`, `mkdir`) pass without modification. The env var signals explicit user intent, so full access is granted.

## Module Layout

```
host/src/spi/
├── config.rs       TOML config loading, SwebashConfig → SandboxPolicy
├── sandbox.rs      resolve_path(), check_access(), check_path()
├── state.rs        AccessMode, PathRule, SandboxPolicy, HostState
├── runtime.rs      setup(SandboxPolicy) → (Store, Instance)
├── imports/
│   ├── fs.rs       10 fs imports with check_path() guards
│   ├── env.rs      SWEBASH_WORKSPACE runtime warning
│   ├── process.rs  CWD sandbox check before host_spawn
│   ├── workspace.rs  host_workspace import handler
│   ├── io.rs       stdout/stderr (unchanged)
│   └── mod.rs      register_all()
└── mod.rs

engine/src/
├── builtins/
│   └── workspace.rs  workspace builtin → host_workspace FFI
├── dispatch.rs       "workspace" match arm
└── spi/host.rs       host_workspace extern declaration
```
