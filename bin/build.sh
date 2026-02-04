#!/usr/bin/env bash
source "$(cd "$(dirname "$0")/.." && pwd)/lib/common.sh"

BUILD_MODE="--release"
for arg in "$@"; do
  case "$arg" in
    --debug) BUILD_MODE="" ;;
  esac
done

PROFILE_LABEL="${BUILD_MODE:+release}"
PROFILE_LABEL="${PROFILE_LABEL:-debug}"

preflight

echo "==> Building engine (wasm32, $PROFILE_LABEL)..."
cargo build --manifest-path "$REPO_ROOT/features/shell/engine/Cargo.toml" \
  --target wasm32-unknown-unknown $BUILD_MODE

echo "==> Building host ($PROFILE_LABEL)..."
cargo build --manifest-path "$REPO_ROOT/features/shell/host/Cargo.toml" $BUILD_MODE

echo "==> Build complete ($PROFILE_LABEL)"
