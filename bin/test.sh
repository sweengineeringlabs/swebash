#!/usr/bin/env bash
source "$(cd "$(dirname "$0")/.." && pwd)/lib/common.sh"

SUITE="${1:-all}"

if [ "$SUITE" = "scripts" ]; then
  if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
    # MINGW: exec/bash swallows stdout; source workaround
    source "$REPO_ROOT/bin/tests/runner.sh"
    exit $?
  else
    exec bash "$REPO_ROOT/bin/tests/runner.sh"
  fi
fi

preflight

echo "==> Building engine WASM (required for tests)..."
cargo build --manifest-path "$REPO_ROOT/features/shell/engine/Cargo.toml" \
  --target wasm32-unknown-unknown --release

run_engine_tests() {
  echo "==> Testing engine..."
  cargo test --manifest-path "$REPO_ROOT/features/shell/engine/Cargo.toml"
}

run_host_tests() {
  echo "==> Testing host..."
  cargo test --manifest-path "$REPO_ROOT/features/shell/host/Cargo.toml"
}

run_readline_tests() {
  echo "==> Testing readline..."
  cargo test --manifest-path "$REPO_ROOT/features/shell/readline/Cargo.toml"
}

run_ai_tests() {
  echo "==> Testing ai..."
  cargo test --manifest-path "$REPO_ROOT/features/ai/Cargo.toml"
}

case "$SUITE" in
  engine)   run_engine_tests ;;
  host)     run_host_tests ;;
  readline) run_readline_tests ;;
  ai)       run_ai_tests ;;
  all)
    run_engine_tests
    run_readline_tests
    run_host_tests
    run_ai_tests
    ;;
  *)
    echo "Usage: ./sbh test [engine|host|readline|ai|scripts|all]"
    exit 1
    ;;
esac

echo "==> Tests complete ($SUITE)"
