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

# ── Preflight checks ──────────────────────────────────────────────────
preflight() {
  ensure_registry
  verify_registry
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
      # WSL: Windows home may differ from Linux home
      local win_user
      win_user=$(cmd.exe /C "echo %USERNAME%" 2>/dev/null | tr -d '\r' || echo "")
      if [ -n "$win_user" ]; then
        export CARGO_REGISTRIES_LOCAL_INDEX="file:///mnt/c/Users/$win_user/.cargo/registry.local/index"
      else
        export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"
      fi
    else
      export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"
    fi
  fi
}

# ── Registry verification ─────────────────────────────────────────────
verify_registry() {
  if [ -z "${CARGO_REGISTRIES_LOCAL_INDEX:-}" ]; then
    echo "ERROR: CARGO_REGISTRIES_LOCAL_INDEX is not set" >&2
    exit 1
  fi

  # Strip file:// prefix to get the filesystem path
  local index_path="${CARGO_REGISTRIES_LOCAL_INDEX#file://}"
  if [ ! -d "$index_path" ]; then
    echo "ERROR: Local registry index not found at $index_path" >&2
    echo "  CARGO_REGISTRIES_LOCAL_INDEX=$CARGO_REGISTRIES_LOCAL_INDEX" >&2
    exit 1
  fi

  echo "==> Registry: $CARGO_REGISTRIES_LOCAL_INDEX (ok)"
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
