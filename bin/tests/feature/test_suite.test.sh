#!/usr/bin/env bash
# Unit tests for bin/test.sh

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"

# -- Helpers -----------------------------------------------------------
# test.sh sources common.sh which calls preflight -> verify_registry.
# For invalid-suite tests that reach preflight, we need a fake registry
# and cargo shim so the script gets past preflight to the case statement.
setup_test_env() {
  TEST_SHIM_DIR=$(mktemp -d)
  TEST_FAKE_REG=$(mktemp -d)
  cat > "$TEST_SHIM_DIR/cargo" << 'SHIM'
#!/usr/bin/env bash
echo "cargo $*"
SHIM
  chmod +x "$TEST_SHIM_DIR/cargo"
  export CARGO_REGISTRIES_LOCAL_INDEX="file://$TEST_FAKE_REG"
  export PATH="$TEST_SHIM_DIR:$PATH"
}
teardown_test_env() {
  rm -rf "$TEST_SHIM_DIR" "$TEST_FAKE_REG"
}

# -- Tests -------------------------------------------------------------

test_invalid_suite_exits_1_with_usage() {
  setup_test_env
  local tmpout tmperr
  tmpout=$(mktemp); tmperr=$(mktemp)
  run_bash "$REPO_ROOT/bin/test.sh" "invalid_suite_name" >"$tmpout" 2>"$tmperr"
  local ec=$?
  local out="$(cat "$tmpout")$(cat "$tmperr")"
  rm -f "$tmpout" "$tmperr"
  teardown_test_env
  assert_exit_code 1 "$ec" "invalid suite should exit 1"
  assert_contains "$out" "Usage" "should show usage"
}

test_all_valid_suite_names_in_usage() {
  setup_test_env
  local tmpout tmperr
  tmpout=$(mktemp); tmperr=$(mktemp)
  run_bash "$REPO_ROOT/bin/test.sh" "not_a_suite" >"$tmpout" 2>"$tmperr"
  local out="$(cat "$tmpout")$(cat "$tmperr")"
  rm -f "$tmpout" "$tmperr"
  teardown_test_env
  assert_contains "$out" "engine" "usage should list engine"
  assert_contains "$out" "host" "usage should list host"
  assert_contains "$out" "readline" "usage should list readline"
  assert_contains "$out" "ai" "usage should list ai"
  assert_contains "$out" "all" "usage should list all"
  assert_contains "$out" "scripts" "usage should list scripts"
}

test_scripts_suite_dispatches_to_runner() {
  # Verify test.sh has the scripts suite early-return that sources the runner.
  # We can't actually run `test.sh scripts` here without infinite recursion
  # (the runner would re-discover this test file), so we verify the wiring.
  local content
  content=$(cat "$REPO_ROOT/bin/test.sh")
  assert_contains "$content" '"scripts"' "test.sh should handle scripts suite"
  assert_contains "$content" "runner.sh" "scripts suite should reference runner.sh"
  # Verify it appears before preflight (early return)
  local scripts_line preflight_line
  scripts_line=$(grep -n 'scripts' "$REPO_ROOT/bin/test.sh" | head -1 | cut -d: -f1)
  preflight_line=$(grep -n 'preflight' "$REPO_ROOT/bin/test.sh" | head -1 | cut -d: -f1)
  assert_eq 1 "$([ "$scripts_line" -lt "$preflight_line" ] && echo 1 || echo 0)" \
    "scripts suite should appear before preflight"
}

test_run_functions_defined() {
  local out
  out=$(bash -n "$REPO_ROOT/bin/test.sh" 2>&1)
  local ec=$?
  assert_exit_code 0 "$ec" "test.sh should have no parse errors"

  local content
  content=$(cat "$REPO_ROOT/bin/test.sh")
  assert_contains "$content" "run_engine_tests" "should define run_engine_tests"
  assert_contains "$content" "run_host_tests" "should define run_host_tests"
  assert_contains "$content" "run_readline_tests" "should define run_readline_tests"
  assert_contains "$content" "run_ai_tests" "should define run_ai_tests"
}
