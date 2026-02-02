# swebash Development Setup Guide

## Prerequisites

- **Rust toolchain**: Install from [rustup.rs](https://rustup.rs)
- **WASM target**: `rustup target add wasm32-unknown-unknown`
- **Git**: For version control
- **Linux/WSL**: Recommended environment (Windows support via WSL)

## Environment Configuration

### Local Registry Setup

swebash depends on the `rustratify` workspace, which uses packages from a custom local Cargo registry. This registry must be configured before building.

#### 1. Verify Local Registry Exists

```bash
ls ~/.cargo/registry.local/index
```

If this directory doesn't exist, you need to set up the local registry first (contact team for registry setup instructions).

#### 2. Configure Environment Variables

The project requires `CARGO_REGISTRIES_LOCAL_INDEX` to point to your local registry. This is configured in `~/.bashrc`:

```bash
# Cargo local registry — toggle SWEBASH_PLATFORM to switch between environments
# Values: "wsl" (default), "windows"
export SWEBASH_PLATFORM="${SWEBASH_PLATFORM:-wsl}"
case "$SWEBASH_PLATFORM" in
  wsl)     export CARGO_REGISTRIES_LOCAL_INDEX="file:///home/adentic/.cargo/registry.local/index" ;;
  windows) export CARGO_REGISTRIES_LOCAL_INDEX="file:///C:/Users/elvis/.cargo/registry.local/index" ;;
esac
```

**Important**: If you're working in a new shell session, you must source your bashrc:

```bash
source ~/.bashrc
```

Or start a new shell:

```bash
exec bash
```

#### 3. Verify Configuration

Check that the environment variable is set:

```bash
echo $CARGO_REGISTRIES_LOCAL_INDEX
# Should output: file:///home/adentic/.cargo/registry.local/index (or similar)
```

#### 4. Registry Configuration Files

The project has `.cargo/config.toml` files that reference the `local` registry:

**swebash/.cargo/config.toml**:
```toml
# Local registry index is set via CARGO_REGISTRIES_LOCAL_INDEX env var
# Toggle with: SWEBASH_PLATFORM=wsl|windows (see ~/.bashrc)
[registries.local]
```

**rustratify/.cargo/config.toml**: Same configuration

The `[registries.local]` section is intentionally empty because the `index` field is provided via the environment variable.

## Project Structure

```
swebash/
  ├── engine/       WASM shell engine (no_std, wasm32 target)
  ├── host/         Native REPL, WASM runtime, host imports
  ├── ai/           LLM integration (depends on rustratify)
  └── docs/         Documentation

Dependencies:
  └── rustratify/   (sibling workspace at ../rustratify)
       └── Requires rustboot-* packages from local registry
```

## Building the Project

### Step 1: Build the WASM Engine

```bash
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release
```

This produces `target/wasm32-unknown-unknown/release/engine.wasm`.

### Step 2: Build the Host Binary

```bash
cargo build --manifest-path host/Cargo.toml
```

Or for release:

```bash
cargo build --manifest-path host/Cargo.toml --release
```

### Step 3: Build Everything

From the root directory:

```bash
# Build engine first
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown --release

# Then build host
cargo build --manifest-path host/Cargo.toml
```

## Running swebash

### Development Build

```bash
./target/debug/swebash
```

### Release Build

```bash
./target/release/swebash
```

## Running Tests

### Engine Tests

```bash
cargo test --manifest-path engine/Cargo.toml
```

### Host Integration Tests

**Important**: Integration tests require `engine.wasm` to exist first!

```bash
# Build engine.wasm first
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown --release

# Run all integration tests
cargo test --manifest-path host/Cargo.toml --test integration

# Run specific test category
cargo test --manifest-path host/Cargo.toml --test integration history
cargo test --manifest-path host/Cargo.toml --test integration echo
```

### AI Module Tests

```bash
cargo test --manifest-path ai/Cargo.toml
```

## API Configuration

### Anthropic API Key

The AI features require an Anthropic API key. Create a `.env` file in the project root:

```bash
cp .env.example .env
# Edit .env and add your API key
```

Or export it:

```bash
export ANTHROPIC_API_KEY="sk-ant-api03-..."
```

The shell will work without AI features if the key is not configured.

## Common Issues and Troubleshooting

### Issue 1: "registry index was not found in any configuration: `local`"

**Error Message**:
```
error: failed to get `dev-engineeringlabs-rustboot-config` as a dependency
...
Caused by:
  registry index was not found in any configuration: `local`
```

**Cause**: `CARGO_REGISTRIES_LOCAL_INDEX` environment variable is not set.

**Solution**:
```bash
# Check if variable is set
echo $CARGO_REGISTRIES_LOCAL_INDEX

# If empty, source your bashrc
source ~/.bashrc

# Or manually export
export CARGO_REGISTRIES_LOCAL_INDEX="file:///home/adentic/.cargo/registry.local/index"

# Then rebuild
cargo build --manifest-path host/Cargo.toml
```

### Issue 2: "engine.wasm not found" during tests

**Error Message**:
```
assertion failed: engine.wasm not found — build it first
```

**Cause**: Integration tests require the compiled WASM module.

**Solution**:
```bash
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown --release
```

### Issue 3: Escape sequences displayed (`^[[A`, `^[[B`) when pressing arrow keys

**Cause**: Old build without rustyline support.

**Solution**:
```bash
# Rebuild host with updated dependencies
cargo build --manifest-path host/Cargo.toml

# Run the new binary
./target/debug/swebash
```

### Issue 4: History not persisting across sessions

**Cause**: HOME directory not writable or wrong permissions.

**Solution**:
```bash
# Check permissions
ls -la ~/.swebash_history

# Fix if needed
chmod 644 ~/.swebash_history
```

### Issue 5: AI commands not working

**Cause**: API key not configured or AI service failed to initialize.

**Solution**:
1. Check that `.env` file exists with `ANTHROPIC_API_KEY`
2. Or export the variable: `export ANTHROPIC_API_KEY="sk-ant-..."`
3. Verify in shell output - if AI fails to initialize, you'll see an error on startup

### Issue 6: Build fails on dependency updates

**Cause**: Cargo.lock out of sync or local registry packages need updating.

**Solution**:
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown --release
cargo build --manifest-path host/Cargo.toml
```

## Development Workflow

### Making Changes to Engine (WASM)

```bash
# Edit files in engine/src/
# Rebuild WASM
cargo build --manifest-path engine/Cargo.toml \
  --target wasm32-unknown-unknown --release

# Test (rebuilds host automatically)
cargo build --manifest-path host/Cargo.toml
./target/debug/swebash
```

### Making Changes to Host (Native REPL)

```bash
# Edit files in host/src/
# Rebuild host
cargo build --manifest-path host/Cargo.toml

# Run
./target/debug/swebash
```

### Making Changes to AI Module

```bash
# Edit files in ai/src/
# Run tests
cargo test --manifest-path ai/Cargo.toml

# Rebuild host (which depends on ai)
cargo build --manifest-path host/Cargo.toml
```

### Running with Debug Logs

```bash
# Enable tracing logs
RUST_LOG=debug ./target/debug/swebash

# Or specific modules
RUST_LOG=swebash=debug,swebash_ai=trace ./target/debug/swebash
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

### CLion / IntelliJ IDEA

- Install Rust plugin
- Configure toolchain: Settings → Languages & Frameworks → Rust
- Add run configuration for `swebash` binary

## Git Workflow

### Branch Strategy

- `main`: Stable releases
- `dev`: Active development
- Feature branches: `feature/description`
- Bug fixes: `fix/description`

### Committing Changes

```bash
# Stage changes
git add .

# Commit with descriptive message
git commit -m "feat(host): add command history with rustyline"

# Push to remote
git push origin dev
```

## Performance Tips

### Faster Builds

1. **Use release mode for WASM** (already recommended above)
2. **Use `cargo check` for quick validation**:
   ```bash
   cargo check --manifest-path host/Cargo.toml
   ```

3. **Incremental compilation** (enabled by default in dev builds)

4. **Parallel compilation**:
   ```bash
   # Use all CPU cores
   cargo build -j $(nproc)
   ```

### Reducing Binary Size

The release profile in `Cargo.toml` is already optimized:
```toml
[profile.release]
opt-level = "s"      # Optimize for size
lto = true           # Link-time optimization
strip = true         # Strip symbols
```

## Getting Help

- **Documentation**: See `docs/` directory
- **Architecture**: `docs/architecture.md`
- **AI Integration**: `docs/ai-integration.md`
- **Command Design**: `docs/command-design.md`
- **History Feature**: `docs/history-feature.md`

## Quick Reference

```bash
# Full build from scratch
cargo build --manifest-path engine/Cargo.toml --target wasm32-unknown-unknown --release
cargo build --manifest-path host/Cargo.toml

# Run shell
./target/debug/swebash

# Run all tests
cargo build --manifest-path engine/Cargo.toml --target wasm32-unknown-unknown --release
cargo test --manifest-path host/Cargo.toml --test integration

# Check environment
echo $CARGO_REGISTRIES_LOCAL_INDEX
echo $ANTHROPIC_API_KEY
```
