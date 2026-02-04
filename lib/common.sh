#!/usr/bin/env bash
# lib/common.sh — shared helpers for swebash bash scripts

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ── Platform detection ───────────────────────────────────────────────
detect_platform() {
  if grep -qi microsoft /proc/version 2>/dev/null; then
    echo "wsl"
  else
    echo "linux"
  fi
}

# ── Registry setup ───────────────────────────────────────────────────
ensure_registry() {
  if [ -z "${CARGO_REGISTRIES_LOCAL_INDEX:-}" ]; then
    source "$HOME/.bashrc" 2>/dev/null || true
  fi
  if [ -z "${CARGO_REGISTRIES_LOCAL_INDEX:-}" ]; then
    local platform
    platform=$(detect_platform)
    if [ "$platform" = "wsl" ]; then
      export CARGO_REGISTRIES_LOCAL_INDEX="file:///home/adentic/.cargo/registry.local/index"
    else
      export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"
    fi
  fi
}

# ── Load .env ────────────────────────────────────────────────────────
load_env() {
  if [ -f "$REPO_ROOT/.env" ]; then
    set -a
    source "$REPO_ROOT/.env"
    set +a
  fi
}

# ── Target directory (matches .cargo/config.toml) ────────────────────
TARGET_DIR="/tmp/swebash-target"
