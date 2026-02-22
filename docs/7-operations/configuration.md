# Configuration

> **TLDR:** Environment variables, TOML config file, and the `workspace` command for AI providers, workspace sandbox, and feature toggles.

**Audience**: Users, DevOps

## Table of Contents

- [XDG Base Directory Specification](#xdg-base-directory-specification)
- [Workspace Sandbox](#workspace-sandbox)
- [Environment Variables](#environment-variables)
- [Quick Start](#quick-start)
- [Graceful Degradation](#graceful-degradation)

---

## XDG Base Directory Specification

swebash follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html), a Linux/Unix standard for where applications store files:

| Variable | Default | Purpose | swebash Usage |
|----------|---------|---------|---------------|
| `XDG_CONFIG_HOME` | `~/.config` | Configuration files | `~/.config/swebash/config.toml`, `agents.yaml`, `workspace/` |
| `XDG_DATA_HOME` | `~/.local/share` | Application data | *(not currently used)* |
| `XDG_STATE_HOME` | `~/.local/state` | Logs, history | `~/.local/state/swebash/history` |
| `XDG_CACHE_HOME` | `~/.cache` | Cache (disposable) | *(not currently used)* |

This separation ensures:
- **Config** (`~/.config/swebash/`) — settings, agents, and default workspace
- **State** — history files that are machine-specific

## Workspace Sandbox

The workspace sandbox controls which filesystem paths the shell can access. It defaults to `~/.config/swebash/workspace/` in read-only mode (XDG Base Directory compliant).

### Config File

Persistent workspace settings are stored in `~/.config/swebash/config.toml`:

```toml
[workspace]
root = "~/.config/swebash/workspace"  # Workspace root directory (XDG-compliant, supports ~ expansion)
mode = "ro"                                 # Default access mode: "ro" or "rw"
enabled = true                              # Whether sandbox enforcement is active

[[workspace.allow]]     # Additional allowed paths (repeatable)
path = "~/projects"
mode = "rw"
```

### `workspace` Command

Session-level overrides (do not persist across restarts):

| Command | Description |
|---------|-------------|
| `workspace` | Show sandbox status (root, mode, allowed paths) |
| `workspace rw` | Set workspace root to read-write |
| `workspace ro` | Set workspace root to read-only |
| `workspace allow PATH [ro\|rw]` | Add an allowed path (default: rw) |
| `workspace disable` | Turn off sandbox entirely |
| `workspace enable` | Turn on sandbox |

### Precedence

Workspace root is resolved in this order (first match wins):

1. `SWEBASH_WORKSPACE` environment variable
2. `root` in `~/.config/swebash/config.toml`
3. `~/.config/swebash/workspace/` (XDG-compliant default)

When `SWEBASH_WORKSPACE` is set via environment variable, the workspace defaults to **read-write** mode (the user explicitly chose the workspace).

### Access Classification

| Operation | Check |
|-----------|-------|
| `cat`, `ls`, `ls -l`, `head`, `tail` | Read |
| `touch`, `mkdir`, `rm`, `cp` (dst), `mv` (src+dst) | Write |
| `cp` (src) | Read |
| `cd` | Read (path must be in sandbox) |
| `pwd` | Always allowed |
| External commands (`host_spawn`) | CWD must be in sandbox |

Denied operations print to stderr: `sandbox: write access denied for '/path': read-only workspace`

## Environment Variables

Configuration is done via environment variables and the TOML config file.

### Workspace

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_WORKSPACE` | *(unset)* | Override workspace root directory. Defaults to RW mode when set. Use `.` to stay in the inherited working directory |

### AI Feature Control

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_AI_ENABLED` | `true` | Set to `false` or `0` to disable AI features entirely |
| `SWEBASH_AI_HISTORY_SIZE` | `20` | Maximum number of chat history messages to retain |

### LLM Provider Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_PROVIDER` | `openai` | LLM provider: `openai`, `anthropic`, `gemini` |
| `LLM_DEFAULT_MODEL` | *(per provider)* | Model to use for completions |

Default models per provider:
- `openai` → `gpt-4o`
- `anthropic` → `claude-sonnet-4-20250514`
- `gemini` → `gemini-2.0-flash`

### API Keys

| Variable | Provider |
|----------|----------|
| `OPENAI_API_KEY` | OpenAI |
| `ANTHROPIC_API_KEY` | Anthropic |
| `GEMINI_API_KEY` | Google Gemini |

## Quick Start

### OpenAI (default)
```bash
export OPENAI_API_KEY=sk-...
# That's it — OpenAI is the default provider
```

### Anthropic
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export LLM_PROVIDER=anthropic
```

### Gemini
```bash
export GEMINI_API_KEY=AI...
export LLM_PROVIDER=gemini
```

### Disable AI
```bash
export SWEBASH_AI_ENABLED=false
```

## Graceful Degradation

If no API key is set or AI is disabled:
- The shell works normally (all non-AI commands pass through to WASM)
- AI commands print a friendly "not configured" message with setup instructions
- `ai status` shows the current configuration state
- No errors, no crashes, no startup delays
