# swebash Installation Guide

> **TLDR:** System requirements, build-from-source instructions, and AI feature setup for swebash.

**Audience**: Users, DevOps

## Table of Contents

- [System Requirements](#system-requirements)
- [Prerequisites](#prerequisites)
- [Building from Source](#building-from-source)
- [Installation](#installation)
- [Configuration](#configuration)
- [AI Features Setup](#ai-features-setup)
- [Running Tests](#running-tests)
- [Production Deployment Notes](#production-deployment-notes)
- [Uninstallation](#uninstallation)
- [Troubleshooting](#troubleshooting)


## System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| OS | Linux (kernel 5.x+) or WSL2 | Ubuntu 22.04+ / WSL2 on Windows 11 |
| Architecture | x86_64 | x86_64 |
| RAM | 512 MB | 2 GB |
| Disk | 200 MB (build artifacts) | 1 GB (includes toolchain) |
| Terminal | Any ANSI-compatible terminal | Windows Terminal, Alacritty, kitty |

## Prerequisites

### 1. Rust Toolchain

Install Rust via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verify the installation:

```bash
rustc --version    # 1.75+ required
cargo --version
```

### 2. WASM Target

The shell engine compiles to WebAssembly. Add the WASM compilation target:

```bash
rustup target add wasm32-unknown-unknown
```

### 3. Local Cargo Registry

swebash depends on packages from a custom local Cargo registry (`rustratify`). This registry must be present before building.

**Verify the registry exists:**

```bash
ls ~/.cargo/registry.local/index
```

If the directory does not exist, obtain the registry from your team before proceeding.

**Configure the registry index** by adding the following to `~/.bashrc` (or `~/.zshrc`):

```bash
export SWEBASH_PLATFORM="${SWEBASH_PLATFORM:-wsl}"
case "$SWEBASH_PLATFORM" in
  wsl)     export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index" ;;
  windows) export CARGO_REGISTRIES_LOCAL_INDEX="file:///C:/Users/$USER/.cargo/registry.local/index" ;;
esac
```

Then reload your shell:

```bash
source ~/.bashrc
```

**Verify:**

```bash
echo $CARGO_REGISTRIES_LOCAL_INDEX
# Expected: file:///home/<user>/.cargo/registry.local/index
```

### 4. Git

```bash
git --version   # Any recent version
```

## Building from Source

### Clone the Repository

```bash
git clone <repository-url> swebash
cd swebash
git checkout main    # or dev for latest development
```

### Step 1: Build the WASM Engine

The engine must be built first as the host binary embeds the compiled WASM module.

```bash
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release
```

Output: `target/wasm32-unknown-unknown/release/engine.wasm`

### Step 2: Build the Host Binary

**Debug build** (faster compilation, slower runtime):

```bash
cargo build --manifest-path host/Cargo.toml
```

Output: `target/debug/swebash`

**Release build** (optimized for size and performance):

```bash
cargo build --manifest-path host/Cargo.toml --release
```

Output: `target/release/swebash`

The release profile applies:
- Size optimization (`opt-level = "s"`)
- Link-time optimization (`lto = true`)
- Symbol stripping (`strip = true`)

### Full Build Script

For convenience, run both steps together:

```bash
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown --release && \
cargo build --manifest-path host/Cargo.toml --release
```

## Installation

### Option A: Local User Install

Copy the binary to a directory on your `$PATH`:

```bash
cp target/release/swebash ~/.local/bin/swebash
chmod +x ~/.local/bin/swebash
```

Ensure `~/.local/bin` is in your `$PATH`:

```bash
# Add to ~/.bashrc if not already present
export PATH="$HOME/.local/bin:$PATH"
```

### Option B: System-wide Install

```bash
sudo cp target/release/swebash /usr/local/bin/swebash
sudo chmod +x /usr/local/bin/swebash
```

### Option C: Run from Build Directory

No installation needed; run directly:

```bash
./target/release/swebash
# or
cargo run --release
```

### Verify Installation

```bash
swebash
# You should see the swebash prompt: swebash>
# Type 'exit' to quit
```

## Configuration

### Shell Configuration (~/.swebashrc)

Create the configuration file from the provided template:

```bash
cp .swebashrc.example ~/.swebashrc
```

**Available settings:**

```toml
[readline]
edit_mode = "emacs"           # Editing mode (emacs only, vi planned)
max_history_size = 1000       # Max commands in history
history_ignore_space = true   # Ignore commands starting with space
enable_completion = true      # Tab completion
enable_highlighting = true    # Syntax highlighting
enable_hints = true           # Inline history hints

[readline.colors]
builtin_command = "green"
external_command = "blue"
invalid_command = "red"
string = "yellow"
path = "cyan"
operator = "magenta"
hint = "gray"
```

### Command History

History is automatically saved to `~/.swebash_history` (up to 1000 entries by default). No configuration is required for basic history functionality.

## AI Features Setup

AI features are optional. The shell works fully without them.

### 1. Create the Environment File

```bash
cp .env.example .env
```

### 2. Configure Your LLM Provider

Edit `.env` and set your provider and API key:

**Anthropic (Claude):**

```bash
LLM_PROVIDER=anthropic
ANTHROPIC_API_KEY=sk-ant-api03-your-key-here
LLM_DEFAULT_MODEL=claude-sonnet-4-20250514
```

**OpenAI:**

```bash
LLM_PROVIDER=openai
OPENAI_API_KEY=sk-your-key-here
LLM_DEFAULT_MODEL=gpt-4o
```

**Google Gemini:**

```bash
LLM_PROVIDER=gemini
GEMINI_API_KEY=your-key-here
LLM_DEFAULT_MODEL=gemini-2.0-flash
```

### 3. Load Environment and Run

```bash
set -a && source .env && set +a
swebash
```

Or export variables directly:

```bash
export LLM_PROVIDER=anthropic
export ANTHROPIC_API_KEY="sk-ant-api03-..."
swebash
```

### 4. Verify AI Features

Inside swebash:

```
swebash> ai
ai> hello
# Should receive an AI response
ai> exit
swebash>
```

Quick AI queries without entering AI mode:

```
swebash> ? what is the current directory
swebash> ?? explain how pipes work in bash
```

### AI Tool Calling Configuration

Control which tools the AI can use via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_AI_TOOLS_FS` | `true` | File system read access |
| `SWEBASH_AI_TOOLS_EXEC` | `true` | Command execution |
| `SWEBASH_AI_TOOLS_WEB` | `true` | Web search via DuckDuckGo |
| `SWEBASH_AI_TOOLS_CONFIRM` | `true` | Require confirmation for dangerous ops |
| `SWEBASH_AI_TOOLS_MAX_ITER` | `10` | Max tool calling iterations |
| `SWEBASH_AI_FS_MAX_SIZE` | `1048576` | Max file read size (bytes) |
| `SWEBASH_AI_EXEC_TIMEOUT` | `30` | Command execution timeout (seconds) |

### Agent Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_AI_DEFAULT_AGENT` | `shell` | Default agent (shell, review, devops, git) |
| `SWEBASH_AI_AGENT_AUTO_DETECT` | `true` | Auto-detect best agent from input |
| `SWEBASH_AI_HISTORY_SIZE` | `20` | Chat history size |

### Security Profiles

**Minimal (chat only, no tools):**

```bash
SWEBASH_AI_TOOLS_FS=false
SWEBASH_AI_TOOLS_EXEC=false
SWEBASH_AI_TOOLS_WEB=false
```

**Safe (read-only, no command execution):**

```bash
SWEBASH_AI_TOOLS_EXEC=false
```

**Full access with higher limits:**

```bash
SWEBASH_AI_TOOLS_MAX_ITER=20
SWEBASH_AI_FS_MAX_SIZE=10485760
SWEBASH_AI_EXEC_TIMEOUT=60
```

## Running Tests

After building, verify the installation with the test suite:

```bash
# Engine unit tests
cargo test --manifest-path engine/Cargo.toml

# Host integration tests (requires engine.wasm)
cargo test --manifest-path host/Cargo.toml --test integration

# AI module tests
cargo test --manifest-path ai/Cargo.toml

# All workspace tests
cargo test --workspace
```

## Production Deployment Notes

For production environments:

- **Do not use `.env` files.** Set environment variables via your deployment platform or a secrets manager (AWS Secrets Manager, HashiCorp Vault, etc.).
- **Disable dangerous tools** by setting `SWEBASH_AI_TOOLS_EXEC=false` unless command execution is explicitly needed.
- **Keep confirmation enabled** (`SWEBASH_AI_TOOLS_CONFIRM=true`) for any environment where tool calling is active.
- **Never commit API keys** to version control. The `.env` file is listed in `.gitignore`.

### systemd Service Example

```ini
[Unit]
Description=swebash AI Shell
After=network.target

[Service]
Type=simple
User=swebash
Environment="LLM_PROVIDER=anthropic"
Environment="ANTHROPIC_API_KEY=sk-ant-..."
Environment="SWEBASH_AI_TOOLS_EXEC=false"
Environment="SWEBASH_AI_TOOLS_CONFIRM=true"
ExecStart=/usr/local/bin/swebash
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

## Uninstallation

### Remove the Binary

```bash
# Local install
rm ~/.local/bin/swebash

# System-wide install
sudo rm /usr/local/bin/swebash
```

### Remove User Data

```bash
rm ~/.swebash_history
rm ~/.swebashrc
```

### Remove Build Artifacts

```bash
cd /path/to/swebash
cargo clean
```

## Troubleshooting

### "registry index was not found in any configuration: `local`"

The `CARGO_REGISTRIES_LOCAL_INDEX` environment variable is not set. Source your shell profile and verify:

```bash
source ~/.bashrc
echo $CARGO_REGISTRIES_LOCAL_INDEX
```

### "engine.wasm not found" during tests or runtime

The WASM engine must be built before the host binary or tests:

```bash
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown --release
```

### Arrow keys display escape sequences (`^[[A`, `^[[B`)

Rebuild the host binary to pick up the crossterm-based line editor:

```bash
cargo build --manifest-path host/Cargo.toml
```

### AI commands return errors or do nothing

1. Verify AI is enabled: `echo $SWEBASH_AI_ENABLED` (should be `true` or unset).
2. Verify the API key is set: `echo $ANTHROPIC_API_KEY | head -c 20`.
3. Verify provider and key match: `echo $LLM_PROVIDER`.
4. Enable debug logging for diagnostics: `RUST_LOG=swebash_ai=debug swebash`.

### History not persisting across sessions

Check that `~/.swebash_history` is writable:

```bash
ls -la ~/.swebash_history
chmod 644 ~/.swebash_history
```

### Build fails after dependency updates

Clean the build cache and rebuild:

```bash
cargo clean
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown --release
cargo build --manifest-path host/Cargo.toml --release
```
