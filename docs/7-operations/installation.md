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

---

## System Requirements

| Requirement | Linux / WSL2 | Windows (native) |
|-------------|-------------|-----------------|
| OS | Linux kernel 5.x+ or WSL2 on Windows 11 | Windows 10 (1903+) or Windows 11 |
| Architecture | x86_64 | x86_64 |
| RAM | 512 MB minimum, 2 GB recommended | 512 MB minimum, 2 GB recommended |
| Disk | 1 GB (toolchain + build artifacts) | 1 GB (toolchain + build artifacts) |
| Shell | bash / zsh | PowerShell 5.1+ or PowerShell 7+ |
| Terminal | Any ANSI-compatible terminal | Windows Terminal (recommended) |

> **Recommended**: WSL2 on Windows 11 with Ubuntu 22.04 and Windows Terminal provides the best experience. Native Windows is fully supported via PowerShell.

---

## Prerequisites

### 1. Rust Toolchain

**Linux / WSL2**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**Windows (PowerShell)**

Download and run the installer from [rustup.rs](https://rustup.rs), or install via winget:

```powershell
winget install Rustlang.Rustup
```

After installation, restart your terminal and verify:

```powershell
rustc --version    # 1.75+ required
cargo --version
```

---

### 2. WASM Target

Required on both platforms:

**Linux / WSL2**
```bash
rustup target add wasm32-unknown-unknown
```

**Windows (PowerShell)**
```powershell
rustup target add wasm32-unknown-unknown
```

---

### 3. Local Cargo Registry

swebash depends on packages from a custom local Cargo registry (`rustratify`). This registry must be present before building.

**Linux / WSL2**

Verify the registry exists:
```bash
ls ~/.cargo/registry.local/index
```

If the directory does not exist, obtain the registry from your team.

Configure the registry index by adding to `~/.bashrc`:
```bash
export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"
```

Reload your shell:
```bash
source ~/.bashrc
```

Verify:
```bash
echo $CARGO_REGISTRIES_LOCAL_INDEX
# Expected: file:///home/<user>/.cargo/registry.local/index
```

> **WSL2 note**: `./sbh setup` auto-detects the Windows-side registry at `/mnt/c/Users/<user>/.cargo/registry.local/index` and copies it to the WSL home if needed.

---

**Windows (PowerShell)**

Verify the registry exists:
```powershell
Test-Path "$env:USERPROFILE\.cargo\registry.local\index"
```

If it does not exist, obtain the registry from your team.

Set the registry as a persistent user environment variable:
```powershell
$RegPath = "$env:USERPROFILE\.cargo\registry.local\index"
$RegUrl  = "file:///" + ($RegPath -replace '\\','/')
[Environment]::SetEnvironmentVariable("CARGO_REGISTRIES_LOCAL_INDEX", $RegUrl, "User")
$env:CARGO_REGISTRIES_LOCAL_INDEX = $RegUrl
```

Verify (in a new terminal):
```powershell
echo $env:CARGO_REGISTRIES_LOCAL_INDEX
# Expected: file:///C:/Users/<user>/.cargo/registry.local/index
```

> **Note**: `.\sbh setup` runs the above automatically.

---

### 4. Git

**Linux / WSL2**
```bash
git --version   # Any recent version
```

**Windows (PowerShell)**
```powershell
winget install Git.Git   # if not already installed
git --version
```

---

## Building from Source

### Clone the Repository

**Linux / WSL2**
```bash
git clone <repository-url> swebash
cd swebash
git checkout main    # or dev for latest development
```

**Windows (PowerShell)**
```powershell
git clone <repository-url> swebash
cd swebash
git checkout main
```

---

### Using sbh (Recommended)

`sbh` is the project launcher. Run one-time setup first, then build.

**Linux / WSL2**
```bash
./sbh setup
source ~/.bashrc
./sbh build             # release (default)
./sbh build --debug     # debug build
```

**Windows (PowerShell)**
```powershell
.\sbh setup
.\sbh build             # release (default)
.\sbh build -Debug      # debug build
```

**Windows (Command Prompt)**
```cmd
sbh.cmd setup
sbh.cmd build
```

---

### Manual Build

If you prefer to invoke cargo directly:

**Step 1 — Build the WASM engine** (must be built first):

**Linux / WSL2**
```bash
cargo build --manifest-path features/shell/engine/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release
```

**Windows (PowerShell)**
```powershell
cargo build --manifest-path features\shell\engine\Cargo.toml `
  --target wasm32-unknown-unknown `
  --release
```

Output: `target/wasm32-unknown-unknown/release/engine.wasm`

---

**Step 2 — Build the host binary**:

**Linux / WSL2**
```bash
# Debug
cargo build --manifest-path features/shell/host/Cargo.toml

# Release
cargo build --manifest-path features/shell/host/Cargo.toml --release
```

**Windows (PowerShell)**
```powershell
# Debug
cargo build --manifest-path features\shell\host\Cargo.toml

# Release
cargo build --manifest-path features\shell\host\Cargo.toml --release
```

Output:
- Linux: `target/release/swebash`
- Windows: `target\release\swebash.exe`

The release profile applies size optimization (`opt-level = "s"`), link-time optimization (`lto = true`), and symbol stripping (`strip = true`).

---

## Installation

### Option A: Local User Install

**Linux / WSL2**
```bash
cp target/release/swebash ~/.local/bin/swebash
chmod +x ~/.local/bin/swebash

# Ensure ~/.local/bin is on $PATH (add to ~/.bashrc if needed)
export PATH="$HOME/.local/bin:$PATH"
```

**Windows (PowerShell)**
```powershell
# Copy to a directory already on %PATH%, e.g. a personal bin folder
$BinDir = "$env:USERPROFILE\bin"
New-Item -ItemType Directory -Force $BinDir | Out-Null
Copy-Item target\release\swebash.exe "$BinDir\swebash.exe"

# Add to PATH (one-time, persists across sessions)
$CurrentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($CurrentPath -notlike "*$BinDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$CurrentPath;$BinDir", "User")
}
```

---

### Option B: System-wide Install

**Linux / WSL2**
```bash
sudo cp target/release/swebash /usr/local/bin/swebash
sudo chmod +x /usr/local/bin/swebash
```

**Windows (PowerShell — run as Administrator)**
```powershell
Copy-Item target\release\swebash.exe "C:\Windows\System32\swebash.exe"
```

---

### Option C: Run from Build Directory

No installation needed; run directly from the repo:

**Linux / WSL2**
```bash
./sbh run
# or
./target/release/swebash
```

**Windows (PowerShell)**
```powershell
.\sbh run
# or
.\target\release\swebash.exe
```

---

### Verify Installation

**Linux / WSL2**
```bash
swebash
# You should see: swebash>
# Type 'exit' to quit
```

**Windows (PowerShell)**
```powershell
swebash
# You should see: swebash>
# Type 'exit' to quit
```

---

## Configuration

### Shell Configuration

**Linux / WSL2**
```bash
cp .swebashrc.example ~/.swebashrc
```

**Windows (PowerShell)**
```powershell
Copy-Item .swebashrc.example "$env:USERPROFILE\.swebashrc"
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

History is saved automatically:
- **Linux / WSL2**: `~/.swebash_history`
- **Windows**: `%USERPROFILE%\.swebash_history`

---

## AI Features Setup

AI features are optional. The shell works fully without them.

### 1. Create the Environment File

**Linux / WSL2**
```bash
cp .env.example .env
```

**Windows (PowerShell)**
```powershell
Copy-Item .env.example .env
```

### 2. Configure Your LLM Provider

Edit `.env` and set your provider and API key:

**Anthropic (Claude):**
```
LLM_PROVIDER=anthropic
ANTHROPIC_API_KEY=sk-ant-api03-your-key-here
LLM_DEFAULT_MODEL=claude-sonnet-4-20250514
```

**OpenAI:**
```
LLM_PROVIDER=openai
OPENAI_API_KEY=sk-your-key-here
LLM_DEFAULT_MODEL=gpt-4o
```

**Google Gemini:**
```
LLM_PROVIDER=gemini
GEMINI_API_KEY=your-key-here
LLM_DEFAULT_MODEL=gemini-2.0-flash
```

### 3. Load Environment and Run

**Linux / WSL2**
```bash
set -a && source .env && set +a
swebash
```

Or export directly:
```bash
export LLM_PROVIDER=anthropic
export ANTHROPIC_API_KEY="sk-ant-api03-..."
swebash
```

**Windows (PowerShell)**
```powershell
# sbh run loads .env automatically
.\sbh run

# Or set variables manually in the current session
$env:LLM_PROVIDER = "anthropic"
$env:ANTHROPIC_API_KEY = "sk-ant-api03-..."
.\target\release\swebash.exe
```

### 4. Verify AI Features

Inside swebash (both platforms):

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
```
SWEBASH_AI_TOOLS_FS=false
SWEBASH_AI_TOOLS_EXEC=false
SWEBASH_AI_TOOLS_WEB=false
```

**Safe (read-only, no command execution):**
```
SWEBASH_AI_TOOLS_EXEC=false
```

**Full access with higher limits:**
```
SWEBASH_AI_TOOLS_MAX_ITER=20
SWEBASH_AI_FS_MAX_SIZE=10485760
SWEBASH_AI_EXEC_TIMEOUT=60
```

---

## Running Tests

### Using sbh (Recommended)

**Linux / WSL2**
```bash
./sbh test          # all suites
./sbh test engine
./sbh test readline
./sbh test host
./sbh test ai
```

**Windows (PowerShell)**
```powershell
.\sbh test
.\sbh test engine
.\sbh test readline
.\sbh test host
.\sbh test ai
```

### Manual

Build the engine first (required for integration tests), then run:

**Linux / WSL2**
```bash
cargo build --manifest-path features/shell/engine/Cargo.toml \
  --target wasm32-unknown-unknown --release

cargo test --manifest-path features/shell/engine/Cargo.toml
cargo test --manifest-path features/shell/host/Cargo.toml --test integration
cargo test --manifest-path features/ai/Cargo.toml
cargo test --workspace
```

**Windows (PowerShell)**
```powershell
cargo build --manifest-path features\shell\engine\Cargo.toml `
  --target wasm32-unknown-unknown --release

cargo test --manifest-path features\shell\engine\Cargo.toml
cargo test --manifest-path features\shell\host\Cargo.toml --test integration
cargo test --manifest-path features\ai\Cargo.toml
cargo test --workspace
```

---

## Production Deployment Notes

- **Do not use `.env` files.** Set environment variables via your deployment platform or a secrets manager (AWS Secrets Manager, HashiCorp Vault, etc.).
- **Disable dangerous tools** by setting `SWEBASH_AI_TOOLS_EXEC=false` unless command execution is explicitly needed.
- **Keep confirmation enabled** (`SWEBASH_AI_TOOLS_CONFIRM=true`) for any environment where tool calling is active.
- **Never commit API keys** to version control. The `.env` file is listed in `.gitignore`.

### systemd Service (Linux)

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

---

## Uninstallation

### Remove the Binary

**Linux / WSL2**
```bash
rm ~/.local/bin/swebash          # local install
sudo rm /usr/local/bin/swebash   # system-wide install
```

**Windows (PowerShell)**
```powershell
Remove-Item "$env:USERPROFILE\bin\swebash.exe"   # local install
# or for system-wide (run as Administrator):
Remove-Item "C:\Windows\System32\swebash.exe"
```

### Remove User Data

**Linux / WSL2**
```bash
rm ~/.swebash_history
rm ~/.swebashrc
```

**Windows (PowerShell)**
```powershell
Remove-Item "$env:USERPROFILE\.swebash_history"
Remove-Item "$env:USERPROFILE\.swebashrc"
```

### Remove Build Artifacts

```bash
cargo clean    # Linux / WSL2
```
```powershell
cargo clean    # Windows
```

---

## Troubleshooting

### "registry index was not found in any configuration: `local`"

`CARGO_REGISTRIES_LOCAL_INDEX` is not set.

**Linux / WSL2**
```bash
./sbh setup
source ~/.bashrc
echo $CARGO_REGISTRIES_LOCAL_INDEX
```

**Windows (PowerShell)**
```powershell
.\sbh setup
# Open a new terminal, then:
echo $env:CARGO_REGISTRIES_LOCAL_INDEX
```

---

### "engine.wasm not found" during tests or runtime

Build the WASM engine before running tests or the host binary:

**Linux / WSL2**
```bash
cargo build --manifest-path features/shell/engine/Cargo.toml \
  --target wasm32-unknown-unknown --release
```

**Windows (PowerShell)**
```powershell
cargo build --manifest-path features\shell\engine\Cargo.toml `
  --target wasm32-unknown-unknown --release
```

---

### Arrow keys display escape sequences (`^[[A`, `^[[B`)

Rebuild the host binary:

**Linux / WSL2**
```bash
./sbh build && ./sbh run
```

**Windows (PowerShell)**
```powershell
.\sbh build
.\sbh run
```

---

### AI commands return errors or do nothing

1. Verify the API key is set: `echo $ANTHROPIC_API_KEY` (Linux) / `echo $env:ANTHROPIC_API_KEY` (Windows).
2. Verify provider matches the key: `echo $LLM_PROVIDER` / `echo $env:LLM_PROVIDER`.
3. Enable debug logging:

**Linux / WSL2**
```bash
RUST_LOG=swebash_ai=debug swebash
```

**Windows (PowerShell)**
```powershell
$env:RUST_LOG = "swebash_ai=debug"
.\target\release\swebash.exe
```

---

### History not persisting across sessions

**Linux / WSL2**
```bash
ls -la ~/.swebash_history
chmod 644 ~/.swebash_history
```

**Windows (PowerShell)**
```powershell
Get-Acl "$env:USERPROFILE\.swebash_history"
```

---

### Build fails after dependency updates

**Linux / WSL2**
```bash
cargo clean
cargo build --manifest-path features/shell/engine/Cargo.toml \
  --target wasm32-unknown-unknown --release
cargo build --manifest-path features/shell/host/Cargo.toml --release
```

**Windows (PowerShell)**
```powershell
cargo clean
cargo build --manifest-path features\shell\engine\Cargo.toml `
  --target wasm32-unknown-unknown --release
cargo build --manifest-path features\shell\host\Cargo.toml --release
```

---

### PowerShell execution policy blocks scripts

If `.\sbh.ps1` is blocked by execution policy, use the `.cmd` wrapper which bypasses it automatically:

```cmd
sbh.cmd setup
sbh.cmd build
sbh.cmd run
```

Or allow scripts in the current session only:
```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
.\sbh setup
```
