#!/usr/bin/env bash
# Unit tests for bin/build.sh

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"

# -- Helpers -----------------------------------------------------------
setup_shims() {
  SHIM_DIR=$(mktemp -d)
  FAKE_REGISTRY=$(mktemp -d)
  FAKE_HOME=$(mktemp -d)
  FAKE_REPO=$(mktemp -d)

  cat > "$SHIM_DIR/cargo" << 'SHIM'
#!/usr/bin/env bash
echo "cargo $*"
SHIM
  chmod +x "$SHIM_DIR/cargo"

  cat > "$SHIM_DIR/rustup" << 'SHIM'
#!/usr/bin/env bash
echo "rustup $*"
SHIM
  chmod +x "$SHIM_DIR/rustup"

  # Create fake registry structure that verify_registry expects
  mkdir -p "$FAKE_REGISTRY"

  # Copy lib/common.sh to fake repo (needed for sourcing)
  mkdir -p "$FAKE_REPO/lib"
  cp "$REPO_ROOT/lib/common.sh" "$FAKE_REPO/lib/"
  # Copy the build script
  mkdir -p "$FAKE_REPO/bin"
  cp "$REPO_ROOT/bin/build.sh" "$FAKE_REPO/bin/"

  # Save original values
  ORIG_PATH="$PATH"
  ORIG_HOME="$HOME"
  ORIG_REGISTRY="${CARGO_REGISTRIES_LOCAL_INDEX:-}"

  export PATH="$SHIM_DIR:$PATH"
  export HOME="$FAKE_HOME"
  export CARGO_REGISTRIES_LOCAL_INDEX="file://$FAKE_REGISTRY"
}

teardown_shims() {
  # Restore original values
  export PATH="$ORIG_PATH"
  export HOME="$ORIG_HOME"
  if [ -n "$ORIG_REGISTRY" ]; then
    export CARGO_REGISTRIES_LOCAL_INDEX="$ORIG_REGISTRY"
  else
    unset CARGO_REGISTRIES_LOCAL_INDEX
  fi
  rm -rf "$SHIM_DIR" "$FAKE_REGISTRY" "$FAKE_HOME" "$FAKE_REPO"
}

# -- Tests -------------------------------------------------------------

test_default_build_is_release() {
  setup_shims
  local out
  out=$(run_bash "$FAKE_REPO/bin/build.sh" 2>&1)
  local ec=$?
  teardown_shims
  assert_exit_code 0 "$ec" "build.sh should exit 0"
  assert_contains "$out" "release" "default build should be release"
  assert_not_contains "$out" "debug" "default should not be debug profile"
}

test_debug_flag_selects_debug_profile() {
  setup_shims
  local out
  out=$(run_bash "$FAKE_REPO/bin/build.sh" --debug 2>&1)
  local ec=$?
  teardown_shims
  assert_exit_code 0 "$ec" "build.sh --debug should exit 0"
  assert_contains "$out" "debug" "should mention debug profile"
}

test_builds_both_engine_and_host() {
  setup_shims
  local out
  out=$(run_bash "$FAKE_REPO/bin/build.sh" 2>&1)
  teardown_shims
  assert_contains "$out" "engine" "should build engine"
  assert_contains "$out" "host" "should build host"
}
