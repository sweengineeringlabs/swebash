# swebash Development Setup Guide

## Prerequisites

- **Rust toolchain**: Install from [rustup.rs](https://rustup.rs)
- **WASM target**: `rustup target add wasm32-unknown-unknown`
- **Git**: For version control
- **Linux/WSL**: Recommended environment (Windows support via PowerShell)
- **Local Cargo registry**: The rustratify registry must be set up before building

## Quick Start

```bash
# One-time setup (installs WASM target, configures registry, creates .env)
./sbh setup

# Source the registry env var
source ~/.bashrc

# Build and run
./sbh run
```

## Environment Configuration

### Local Registry Setup

swebash depends on the `rustratify` workspace, which uses packages from a custom local Cargo registry. This registry must be configured before building.

#### 1. Verify Local Registry Exists

```bash
ls ~/.cargo/registry.local/index
```

If this directory doesn't exist, you need to set up the local registry first (contact team for registry setup instructions).

#### 2. Automated Setup

The recommended approach is to use the setup script:

```bash
./sbh setup
```

This will:
- Check that `rustup` and `cargo` are installed
- Install the `wasm32-unknown-unknown` target
- Detect your platform (WSL or native Linux)
- Locate the local Cargo registry (auto-detects Windows username on WSL)
- Persist `CARGO_REGISTRIES_LOCAL_INDEX` to `~/.bashrc`
- Copy `.env.example` to `.env` if needed
- Verify the registry is reachable

#### 3. Manual Setup (if needed)

If the automated setup doesn't work for your environment:

```bash
# Set the registry URL (replace with your actual path)
export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"

# Persist to ~/.bashrc
echo 'export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"' >> ~/.bashrc
```

On WSL, use the Windows path:

```bash
export CARGO_REGISTRIES_LOCAL_INDEX="file:///mnt/c/Users/$USER/.cargo/registry.local/index"
```

#### 4. Verify Configuration

```bash
echo $CARGO_REGISTRIES_LOCAL_INDEX
# Should output: file:///.../.cargo/registry.local/index
```

All `sbh` commands run a preflight check that verifies the registry is set and reachable before proceeding.

#### 5. Registry Configuration Files

The project has `.cargo/config.toml` that references the `local` registry:

```toml
# Local registry index is set via CARGO_REGISTRIES_LOCAL_INDEX env var
[registries.local]
```

The `[registries.local]` section is intentionally empty because the `index` field is provided via the environment variable.

## Project Structure

```
swebash/
├── features/
│   ├── shell/              Umbrella feature
│   │   ├── engine/         WASM shell engine (no_std, wasm32 target)
│   │   ├── host/           Thin binary — REPL loop + composition
│   │   └── readline/       Line editing + history
│   └── ai/                 LLM integration (depends on rustratify)
├── bin/                    Build/run/test scripts
├── lib/                    Shared script helpers
├── docs/                   Documentation
├── sbh                     Launcher (bash)
└── sbh.ps1                 Launcher (PowerShell)

Dependencies:
  └── rustratify/   (sibling workspace)
       └── Requires rustboot-* packages from local registry
```

## Building the Project

### Using sbh (Recommended)

```bash
# Release build (default)
./sbh build

# Debug build
./sbh build --debug
```

### Manual Build

```bash
# Step 1: Build the WASM engine
cargo build --manifest-path features/shell/engine/Cargo.toml \
  --target wasm32-unknown-unknown --release

# Step 2: Build the host binary
cargo build --manifest-path features/shell/host/Cargo.toml
```

## Running swebash

### Using sbh (Recommended)

```bash
# Debug mode (default)
./sbh run

# Release mode
./sbh run --release
```

### With AI Features

```bash
# Source .env (contains ANTHROPIC_API_KEY)
set -a && source .env && set +a
export LLM_PROVIDER=anthropic
./sbh run
```

## Running Tests

### Using sbh (Recommended)

```bash
# All tests
./sbh test

# Individual suites
./sbh test engine
./sbh test readline
./sbh test host
./sbh test ai
```

### Manual Test Commands

```bash
# Build engine.wasm first (required for integration tests)
cargo build --manifest-path features/shell/engine/Cargo.toml \
  --target wasm32-unknown-unknown --release

# Then run tests
cargo test --workspace
```

## API Configuration

### LLM Provider Setup

The AI features require an API key and a matching `LLM_PROVIDER` value.

| Provider | API Key Variable | LLM_PROVIDER |
|----------|-----------------|--------------|
| Anthropic | `ANTHROPIC_API_KEY` | `anthropic` |
| OpenAI | `OPENAI_API_KEY` | `openai` (default) |
| Gemini | `GEMINI_API_KEY` | `gemini` |

Create a `.env` file in the project root:

```bash
cp .env.example .env
# Edit .env and add your API key
```

Then source it before running:

```bash
set -a && source .env && set +a
export LLM_PROVIDER=anthropic   # must match the key you set
./sbh run
```

The shell will work without AI features if no key is configured.

## Common Issues and Troubleshooting

### Issue 1: "registry index was not found in any configuration: `local`"

**Cause**: `CARGO_REGISTRIES_LOCAL_INDEX` environment variable is not set.

**Solution**:
```bash
# Run setup
./sbh setup

# Or check/set manually
echo $CARGO_REGISTRIES_LOCAL_INDEX
source ~/.bashrc
```

### Issue 2: "engine.wasm not found" during tests

**Cause**: Integration tests require the compiled WASM module.

**Solution**:
```bash
# Use sbh (builds engine automatically before tests)
./sbh test

# Or build manually
cargo build --manifest-path features/shell/engine/Cargo.toml \
  --target wasm32-unknown-unknown --release
```

### Issue 3: Escape sequences displayed (`^[[A`, `^[[B`) when pressing arrow keys

**Cause**: Old build without readline support.

**Solution**:
```bash
./sbh build
./sbh run
```

### Issue 4: History not persisting across sessions

**Cause**: HOME directory not writable or wrong permissions.

**Solution**:
```bash
ls -la ~/.swebash_history
chmod 644 ~/.swebash_history
```

### Issue 5: AI commands not working

**Cause**: API key not configured or AI service failed to initialize.

**Solution**:
1. Check that `.env` file exists with your API key
2. Source it: `set -a && source .env && set +a`
3. Set provider: `export LLM_PROVIDER=anthropic`
4. The shell shows a warning on startup if AI is not configured

### Issue 6: Build fails on dependency updates

**Cause**: Cargo.lock out of sync or local registry packages need updating.

**Solution**:
```bash
cargo update
./sbh build
```

## Development Workflow

### Making Changes to Engine (WASM)

```bash
# Edit files in features/shell/engine/src/
./sbh build
./sbh test engine
./sbh run
```

### Making Changes to Host (REPL)

```bash
# Edit files in features/shell/host/src/
./sbh build --debug
./sbh test host
./sbh run
```

### Making Changes to Readline

```bash
# Edit files in features/shell/readline/src/
./sbh test readline
./sbh build --debug
./sbh run
```

### Making Changes to AI Module

```bash
# Edit files in features/ai/src/
./sbh test ai
./sbh build --debug
./sbh run
```

## IDE Setup

### VS Code

Recommended extensions:
- **rust-analyzer**: Rust language server
- **CodeLLDB**: Debugging support
- **Even Better TOML**: TOML syntax highlighting

Add to `.vscode/settings.json`:
```json
{
  "rust-analyzer.cargo.target": "wasm32-unknown-unknown",
  "rust-analyzer.check.allTargets": false,
  "rust-analyzer.cargo.allFeatures": true
}
```

## Quick Reference

```bash
# One-time setup
./sbh setup && source ~/.bashrc

# Build, test, run
./sbh build
./sbh test
./sbh run

# Check environment
echo $CARGO_REGISTRIES_LOCAL_INDEX
```
