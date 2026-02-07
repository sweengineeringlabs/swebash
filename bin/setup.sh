#!/usr/bin/env bash
source "$(cd "$(dirname "$0")/.." && pwd)/lib/common.sh"

PLATFORM=$(detect_platform)
echo "==> Detected platform: $PLATFORM"

# ── Check prerequisites ──────────────────────────────────────────────
echo "==> Checking prerequisites..."

if ! command -v rustup &>/dev/null; then
  echo "ERROR: rustup not found. Install from https://rustup.rs" >&2
  exit 1
fi

if ! command -v cargo &>/dev/null; then
  echo "ERROR: cargo not found. Install Rust via rustup." >&2
  exit 1
fi

echo "  rustup: $(rustup --version 2>&1 | head -1)"
echo "  cargo:  $(cargo --version)"

# ── Install WASM target ─────────────────────────────────────────────
echo "==> Installing wasm32-unknown-unknown target..."
rustup target add wasm32-unknown-unknown

# ── Verify / set up local registry ───────────────────────────────────
if [ -n "${CARGO_REGISTRIES_LOCAL_INDEX:-}" ]; then
  # Registry already configured — derive paths from env
  REGISTRY_URL="$CARGO_REGISTRIES_LOCAL_INDEX"
  REGISTRY_DIR="${REGISTRY_URL#file://}"
elif [ "$PLATFORM" = "wsl" ]; then
  WIN_USER=$(cmd.exe /C "echo %USERNAME%" 2>/dev/null | tr -d '\r' || echo "")
  if [ -z "$WIN_USER" ]; then
    echo "ERROR: Could not detect Windows username" >&2
    exit 1
  fi
  WIN_REGISTRY="/mnt/c/Users/$WIN_USER/.cargo/registry.local/index"
  LINUX_REGISTRY="$HOME/.cargo/registry.local/index"

  if [ ! -d "$LINUX_REGISTRY" ] && [ -d "$WIN_REGISTRY" ]; then
    echo "==> Local registry not found in WSL home; copying from Windows..."
    mkdir -p "$(dirname "$LINUX_REGISTRY")"
    cp -r "$WIN_REGISTRY" "$LINUX_REGISTRY"
  fi

  REGISTRY_DIR="$WIN_REGISTRY"
  REGISTRY_URL="file://$WIN_REGISTRY"
else
  REGISTRY_DIR="$HOME/.cargo/registry.local/index"
  REGISTRY_URL="file://$HOME/.cargo/registry.local/index"
fi

if [ -d "$REGISTRY_DIR" ]; then
  echo "==> Local registry found at $REGISTRY_DIR"
else
  echo "ERROR: Local registry not found at $REGISTRY_DIR" >&2
  echo "  Set up the rustratify registry before running setup." >&2
  exit 1
fi

# ── Persist CARGO_REGISTRIES_LOCAL_INDEX ─────────────────────────────
if ! grep -q 'CARGO_REGISTRIES_LOCAL_INDEX' "$HOME/.bashrc" 2>/dev/null; then
  echo "" >> "$HOME/.bashrc"
  echo "# swebash local cargo registry" >> "$HOME/.bashrc"
  echo "export CARGO_REGISTRIES_LOCAL_INDEX=\"$REGISTRY_URL\"" >> "$HOME/.bashrc"
  echo "==> Added CARGO_REGISTRIES_LOCAL_INDEX to ~/.bashrc"
else
  echo "==> CARGO_REGISTRIES_LOCAL_INDEX already in ~/.bashrc"
fi

export CARGO_REGISTRIES_LOCAL_INDEX="$REGISTRY_URL"

# ── Copy .env.example → .env ────────────────────────────────────────
if [ ! -f "$REPO_ROOT/.env" ]; then
  if [ -f "$REPO_ROOT/.env.example" ]; then
    cp "$REPO_ROOT/.env.example" "$REPO_ROOT/.env"
    echo "==> Copied .env.example → .env (edit API keys before running)"
  else
    echo "!! .env.example not found — skipping .env creation"
  fi
else
  echo "==> .env already exists"
fi

# ── Verify setup ─────────────────────────────────────────────────────
echo ""
verify_registry

# ── Summary ──────────────────────────────────────────────────────────
echo ""
echo "Setup complete!"
echo "  Platform:  $PLATFORM"
echo "  Registry:  $REGISTRY_URL"
echo "  .env:      $REPO_ROOT/.env"
echo ""
echo "Next steps:"
echo "  1. Edit .env with your API keys"
echo "  2. Run: source ~/.bashrc"
echo "  3. Run: ./sbh build"
