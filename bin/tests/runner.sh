#!/usr/bin/env bash
# bin/tests/runner.sh -- DIY bash test runner for swebash scripts
set -uo pipefail

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"

# -- Color support ----------------------------------------------------
if [ -t 1 ]; then
  GREEN=$'\033[0;32m'  RED=$'\033[0;31m'  YELLOW=$'\033[0;33m'
  BOLD=$'\033[1m'      RESET=$'\033[0m'
else
  GREEN=''  RED=''  YELLOW=''  BOLD=''  RESET=''
fi

# -- Counters ---------------------------------------------------------
PASS=0  FAIL=0  SKIP=0  TOTAL=0

# -- Assertion primitives ---------------------------------------------
assert_eq() {
  local expected="$1" actual="$2" msg="${3:-assert_eq}"
  if [ "$expected" = "$actual" ]; then
    return 0
  else
    echo "  FAIL: $msg" >&2
    echo "    expected: '$expected'" >&2
    echo "    actual:   '$actual'" >&2
    return 1
  fi
}

assert_contains() {
  local haystack="$1" needle="$2" msg="${3:-assert_contains}"
  if echo "$haystack" | grep -qF -- "$needle"; then
    return 0
  else
    echo "  FAIL: $msg" >&2
    echo "    expected to contain: '$needle'" >&2
    echo "    actual: '$haystack'" >&2
    return 1
  fi
}

assert_not_contains() {
  local haystack="$1" needle="$2" msg="${3:-assert_not_contains}"
  if echo "$haystack" | grep -qF -- "$needle"; then
    echo "  FAIL: $msg" >&2
    echo "    expected NOT to contain: '$needle'" >&2
    echo "    actual: '$haystack'" >&2
    return 1
  else
    return 0
  fi
}

assert_match() {
  local actual="$1" pattern="$2" msg="${3:-assert_match}"
  if echo "$actual" | grep -qE -- "$pattern"; then
    return 0
  else
    echo "  FAIL: $msg" >&2
    echo "    expected to match: '$pattern'" >&2
    echo "    actual: '$actual'" >&2
    return 1
  fi
}

assert_exit_code() {
  local expected="$1" actual="$2" msg="${3:-assert_exit_code}"
  if [ "$expected" -eq "$actual" ]; then
    return 0
  else
    echo "  FAIL: $msg" >&2
    echo "    expected exit code: $expected" >&2
    echo "    actual exit code:   $actual" >&2
    return 1
  fi
}

skip_test() {
  local msg="${1:-skipped}"
  echo "  SKIP: $msg"
  exit 77
}

# -- Platform detection ------------------------------------------------
# MINGW bash swallows stdout from `bash script.sh`; source workaround needed.
if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
  _RUNNER_MINGW=1
else
  _RUNNER_MINGW=0
fi

# -- Invoke helper (low-level) -----------------------------------------
# Usage: run_bash <script> [args...]
# Runs a bash script with stdout/stderr inherited.  Tests that need raw
# invocation (e.g. with custom env vars or redirects) use this instead of
# run_script.
run_bash() {
  if [ "$_RUNNER_MINGW" -eq 1 ]; then
    bash -c '. "$0" "$@"' "$@"
  else
    bash "$@"
  fi
}

# -- Script runner helper ----------------------------------------------
# Usage: run_script <script> [args...]
# Sets: STDOUT, STDERR, EXIT_CODE
run_script() {
  local script="$1"; shift
  local tmpout tmperr
  tmpout=$(mktemp)
  tmperr=$(mktemp)
  set +e
  run_bash "$script" "$@" >"$tmpout" 2>"$tmperr"
  EXIT_CODE=$?
  set -e
  STDOUT=$(cat "$tmpout")
  STDERR=$(cat "$tmperr")
  rm -f "$tmpout" "$tmperr"
}

# -- Test discovery and execution --------------------------------------
run_test_file() {
  local test_file="$1"
  local rel_path="${test_file#$TESTS_DIR/}"
  printf "%s%s%s\n" "$BOLD" "$rel_path" "$RESET"

  # Extract test_ function names from the file
  local funcs
  funcs=$(grep -oE '^test_[a-zA-Z_0-9]+\s*\(\)' "$test_file" | sed 's/()//' | tr -d ' ')

  if [ -z "$funcs" ]; then
    echo "  (no test functions found)"
    return
  fi

  for func in $funcs; do
    TOTAL=$((TOTAL + 1))
    # Run each test in a subshell to prevent bleed
    local result
    result=$(
      # Source the test file in the subshell so functions are available
      source "$test_file"
      "$func"
    ) 2>&1
    local exit_code=$?

    if [ $exit_code -eq 77 ]; then
      SKIP=$((SKIP + 1))
      printf "  %sSKIP%s %s\n" "$YELLOW" "$RESET" "$func"
      if [ -n "$result" ]; then echo "$result"; fi
    elif [ $exit_code -eq 0 ]; then
      PASS=$((PASS + 1))
      printf "  %sPASS%s %s\n" "$GREEN" "$RESET" "$func"
    else
      FAIL=$((FAIL + 1))
      printf "  %sFAIL%s %s\n" "$RED" "$RESET" "$func"
      if [ -n "$result" ]; then echo "$result"; fi
    fi
  done
}

# -- Main --------------------------------------------------------------
TARGET="${1:-}"
DIRS=()

if [ -z "$TARGET" ]; then
  DIRS=("$TESTS_DIR/feature" "$TESTS_DIR/e2e")
elif [ "$TARGET" = "feature" ]; then
  DIRS=("$TESTS_DIR/feature")
elif [ "$TARGET" = "e2e" ]; then
  DIRS=("$TESTS_DIR/e2e")
else
  echo "Usage: runner.sh [feature|e2e]" >&2
  exit 1
fi

printf "%s==> Running bash script tests%s\n" "$BOLD" "$RESET"
echo ""

for dir in "${DIRS[@]}"; do
  if [ ! -d "$dir" ]; then continue; fi
  for test_file in "$dir"/*.test.sh; do
    [ -f "$test_file" ] || continue
    run_test_file "$test_file"
    echo ""
  done
done

# -- Summary -----------------------------------------------------------
printf "%sResults: %s%d passed%s, %s%d failed%s, %s%d skipped%s (%d total)\n" \
  "$BOLD" "$GREEN" "$PASS" "$RESET" "$RED" "$FAIL" "$RESET" "$YELLOW" "$SKIP" "$RESET" "$TOTAL"

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
exit 0
