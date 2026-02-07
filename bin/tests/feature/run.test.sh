#!/usr/bin/env bash
# Unit tests for bin/run.sh

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"

# -- Helpers -----------------------------------------------------------
setup_shims() {
  SHIM_DIR=$(mktemp -d)
  FAKE_REGISTRY=$(mktemp -d)

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

  export PATH="$SHIM_DIR:$PATH"
  export CARGO_REGISTRIES_LOCAL_INDEX="file://$FAKE_REGISTRY"
}

teardown_shims() {
  rm -rf "$SHIM_DIR" "$FAKE_REGISTRY"
}

# -- Tests -------------------------------------------------------------

test_default_profile_is_debug() {
  setup_shims
  local out
  out=$(run_bash "$REPO_ROOT/bin/run.sh" 2>&1) || true
  teardown_shims
  assert_contains "$out" "debug" "default profile should be debug"
}

test_release_flag_selects_release() {
  setup_shims
  local out
  out=$(run_bash "$REPO_ROOT/bin/run.sh" --release 2>&1) || true
  teardown_shims
  assert_contains "$out" "release" "should select release profile"
}

test_triggers_build_when_binaries_missing() {
  setup_shims
  local empty_target
  empty_target=$(mktemp -d)
  local out
  out=$(TARGET_DIR="$empty_target" run_bash "$REPO_ROOT/bin/run.sh" 2>&1) || true
  teardown_shims
  rm -rf "$empty_target"
  # run.sh prints "Binaries not found, building" when they're missing
  assert_match "$out" "[Bb]uild" "should trigger build when binaries missing"
}
