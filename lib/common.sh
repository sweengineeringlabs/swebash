#!/usr/bin/env bash
# lib/common.sh — shared helpers for swebash bash scripts

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ── Platform detection ───────────────────────────────────────────────
detect_platform() {
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
      echo "mingw"
      ;;
    *)
      if grep -qi microsoft /proc/version 2>/dev/null; then
        echo "wsl"
      else
        echo "linux"
      fi
      ;;
  esac
}

# ── Preflight checks ──────────────────────────────────────────────────
preflight() {
  load_env
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
    case "$platform" in
      wsl)
        # WSL: Windows home may differ from Linux home
        local win_user
        win_user=$(cmd.exe /C "echo %USERNAME%" 2>/dev/null | tr -d '\r' || echo "")
        if [ -n "$win_user" ]; then
          export CARGO_REGISTRIES_LOCAL_INDEX="file:///mnt/c/Users/$win_user/.cargo/registry.local/index"
        else
          export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"
        fi
        ;;
      mingw)
        # MINGW/Git Bash: $HOME is already Windows path in MINGW format
        export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"
        ;;
      *)
        # Linux: use $HOME directly
        export CARGO_REGISTRIES_LOCAL_INDEX="file://$HOME/.cargo/registry.local/index"
        ;;
    esac
  fi

  # Sync config.json dl path to match current platform
  local index_path="${CARGO_REGISTRIES_LOCAL_INDEX#file://}"
  local config_json="$index_path/config.json"
  if [ -f "$config_json" ]; then
    local registry_base="${CARGO_REGISTRIES_LOCAL_INDEX%/index}"
    local expected_dl="${registry_base}/crates/{crate}/{version}/download"
    local current_dl
    current_dl=$(grep -oP '"dl"\s*:\s*"\K[^"]+' "$config_json" || echo "")
    if [ "$current_dl" != "$expected_dl" ]; then
      # Platform changed — update dl and commit
      local new_json
      new_json=$(printf '{\n  "dl": "%s",\n  "api": null\n}\n' "$expected_dl")
      echo "$new_json" > "$config_json"
      git -C "$index_path" add config.json
      git -C "$index_path" commit -m "sync dl path for $(detect_platform)" --quiet
    fi
  fi

  # Sync Cargo.lock registry paths to match current platform
  local cargo_lock="$REPO_ROOT/Cargo.lock"
  if [ -f "$cargo_lock" ]; then
    local expected_url="registry+$CARGO_REGISTRIES_LOCAL_INDEX"
    local old_url
    old_url=$(grep -m1 -oP 'registry\+file:///[^"]*registry\.local/index' "$cargo_lock" || echo "")
    if [ -n "$old_url" ] && [ "$old_url" != "$expected_url" ]; then
      sed -i "s|$old_url|$expected_url|g" "$cargo_lock"
      echo "==> Cargo.lock: synced registry paths for $(detect_platform)"
    fi
  fi

  # Sync registry index metadata to match current platform
  if [ -d "$index_path" ]; then
    local expected_registry="$CARGO_REGISTRIES_LOCAL_INDEX"
    local old_registry
    old_registry=$(grep -rhm1 -oP 'file:///[^"]*registry\.local/index' "$index_path" --exclude-dir='.git' 2>/dev/null | head -1 || echo "")
    if [ -n "$old_registry" ] && [ "$old_registry" != "$expected_registry" ]; then
      grep -rl "$old_registry" "$index_path" --exclude-dir='.git' 2>/dev/null | while read -r f; do
        sed -i "s|$old_registry|$expected_registry|g" "$f"
      done
      git -C "$index_path" add -A
      git -C "$index_path" commit -m "sync registry metadata paths for $(detect_platform)" --quiet
      # Delete Cargo's index clone so it re-fetches from updated source
      local cargo_home="${index_path%/registry.local/index}"
      find "$cargo_home/registry/index" -maxdepth 1 -mindepth 1 -type d ! -name '*crates.io*' -exec rm -rf {} + 2>/dev/null || true
      echo "==> Registry index: synced metadata paths for $(detect_platform)"
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
TARGET_DIR="${TARGET_DIR:-/tmp/swebash-target}"
