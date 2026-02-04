#!/usr/bin/env bash
source "$(cd "$(dirname "$0")/.." && pwd)/lib/common.sh"

PLATFORM=$(detect_platform)
echo "==> Detected platform: $PLATFORM"

# ── Install WASM target ─────────────────────────────────────────────
echo "==> Installing wasm32-unknown-unknown target..."
rustup target add wasm32-unknown-unknown

# ── Verify / set up local registry ───────────────────────────────────
REGISTRY_DIR="$HOME/.cargo/registry.local/index"

if [ "$PLATFORM" = "wsl" ]; then
  WIN_REGISTRY="/mnt/c/Users/elvis/.cargo/registry.local/index"

  if [ ! -d "$REGISTRY_DIR" ] && [ -d "$WIN_REGISTRY" ]; then
    echo "==> Local registry not found in WSL home; copying from Windows..."
    mkdir -p "$(dirname "$REGISTRY_DIR")"
    cp -r "$WIN_REGISTRY" "$REGISTRY_DIR"
  fi

  REGISTRY_URL="file:///home/adentic/.cargo/registry.local/index"
else
  REGISTRY_URL="file://$HOME/.cargo/registry.local/index"
fi

if [ -d "$REGISTRY_DIR" ]; then
  echo "==> Local registry found at $REGISTRY_DIR"
else
  echo "!! WARNING: Local registry not found at $REGISTRY_DIR"
  echo "   You may need to set up the rustratify registry manually."
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
