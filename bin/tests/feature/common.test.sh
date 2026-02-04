#!/usr/bin/env bash
# Unit tests for lib/common.sh

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"

# -- detect_platform --------------------------------------------------

test_detect_platform_returns_wsl_or_linux() {
  local result
  result=$(
    set +euo pipefail
    source "$REPO_ROOT/lib/common.sh"
    detect_platform
  )
  assert_match "$result" "^(wsl|linux)$" "detect_platform should return wsl or linux"
}

# -- verify_registry ---------------------------------------------------

test_verify_registry_exits_1_when_env_unset() {
  local tmpout tmperr
  tmpout=$(mktemp); tmperr=$(mktemp)
  (
    unset CARGO_REGISTRIES_LOCAL_INDEX
    set +euo pipefail
    source "$REPO_ROOT/lib/common.sh"
    verify_registry
  ) >"$tmpout" 2>"$tmperr"
  local ec=$?
  local err=$(cat "$tmperr")
  rm -f "$tmpout" "$tmperr"
  assert_exit_code 1 "$ec" "verify_registry should exit 1 when env var unset"
  assert_contains "$err" "not set" "should mention 'not set'"
}

test_verify_registry_exits_1_when_path_missing() {
  local tmpout tmperr
  tmpout=$(mktemp); tmperr=$(mktemp)
  (
    export CARGO_REGISTRIES_LOCAL_INDEX="file:///nonexistent/path/does/not/exist"
    set +euo pipefail
    source "$REPO_ROOT/lib/common.sh"
    verify_registry
  ) >"$tmpout" 2>"$tmperr"
  local ec=$?
  local err=$(cat "$tmperr")
  rm -f "$tmpout" "$tmperr"
  assert_exit_code 1 "$ec" "verify_registry should exit 1 when path missing"
  assert_contains "$err" "not found" "should mention 'not found'"
}

test_verify_registry_prints_ok_for_valid_path() {
  local tmpdir tmpout tmperr
  tmpdir=$(mktemp -d)
  tmpout=$(mktemp); tmperr=$(mktemp)
  (
    export CARGO_REGISTRIES_LOCAL_INDEX="file://$tmpdir"
    set +euo pipefail
    source "$REPO_ROOT/lib/common.sh"
    verify_registry
  ) >"$tmpout" 2>"$tmperr"
  local ec=$?
  local out=$(cat "$tmpout")
  rm -f "$tmpout" "$tmperr"
  rm -rf "$tmpdir"
  assert_exit_code 0 "$ec" "verify_registry should exit 0 for valid path"
  assert_contains "$out" "(ok)" "should print (ok)"
}

# -- ensure_registry ---------------------------------------------------

test_ensure_registry_sets_env_var_when_unset() {
  local result
  result=$(
    unset CARGO_REGISTRIES_LOCAL_INDEX
    export HOME=$(mktemp -d)
    set +euo pipefail
    source "$REPO_ROOT/lib/common.sh"
    ensure_registry
    echo "$CARGO_REGISTRIES_LOCAL_INDEX"
  )
  assert_match "$result" "^file://" "ensure_registry should set CARGO_REGISTRIES_LOCAL_INDEX"
}

# -- load_env ----------------------------------------------------------

test_load_env_sets_vars_from_env_file() {
  local tmpdir result
  tmpdir=$(mktemp -d)
  cp -r "$REPO_ROOT/lib" "$tmpdir/lib"
  echo 'TEST_VAR_FROM_ENV=hello_from_env' > "$tmpdir/.env"

  result=$(
    set +euo pipefail
    REPO_ROOT="$tmpdir"
    source "$tmpdir/lib/common.sh"
    REPO_ROOT="$tmpdir"
    load_env
    echo "$TEST_VAR_FROM_ENV"
  )
  rm -rf "$tmpdir"
  assert_eq "hello_from_env" "$result" "load_env should set vars from .env"
}
