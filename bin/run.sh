#!/usr/bin/env bash
source "$(cd "$(dirname "$0")/.." && pwd)/lib/common.sh"

preflight
load_env

PROFILE_DIR="debug"
CARGO_BUILD_FLAG=""
for arg in "$@"; do
  case "$arg" in
    --release) PROFILE_DIR="release"; CARGO_BUILD_FLAG="--release" ;;
    --debug)   PROFILE_DIR="debug";   CARGO_BUILD_FLAG="" ;;
  esac
done

WASM_BIN="$TARGET_DIR/wasm32-unknown-unknown/$PROFILE_DIR/engine.wasm"
HOST_BIN="$TARGET_DIR/$PROFILE_DIR/swebash"

if [ ! -f "$WASM_BIN" ] || [ ! -f "$HOST_BIN" ]; then
  echo "==> Binaries not found, building ($PROFILE_DIR)..."
  cargo build --manifest-path "$REPO_ROOT/features/shell/engine/Cargo.toml" \
    --target wasm32-unknown-unknown $CARGO_BUILD_FLAG
  cargo build --manifest-path "$REPO_ROOT/features/shell/host/Cargo.toml" $CARGO_BUILD_FLAG
fi

echo "==> Launching swebash ($PROFILE_DIR)..."
exec "$HOST_BIN"
