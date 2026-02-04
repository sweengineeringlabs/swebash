#!/usr/bin/env bash
source "$(cd "$(dirname "$0")/.." && pwd)/lib/common.sh"

load_env

BUILD_MODE="--debug"
PROFILE_DIR="debug"
for arg in "$@"; do
  case "$arg" in
    --release) BUILD_MODE="--release"; PROFILE_DIR="release" ;;
    --debug)   BUILD_MODE="--debug";   PROFILE_DIR="debug" ;;
  esac
done

if [ "$BUILD_MODE" = "--debug" ]; then
  CARGO_BUILD_FLAG=""
else
  CARGO_BUILD_FLAG="--release"
fi

ensure_registry

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
