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

test_ensure_registry_syncs_cargo_lock_stale_path() {
  local tmpdir
  tmpdir=$(mktemp -d)
  cp -r "$REPO_ROOT/lib" "$tmpdir/lib"

  cat > "$tmpdir/Cargo.lock" << 'EOF'
[[package]]
name = "some-crate"
version = "0.1.0"
source = "registry+file:///mnt/c/Users/olduser/.cargo/registry.local/index"

[[package]]
name = "another-crate"
version = "0.2.0"
source = "registry+file:///mnt/c/Users/olduser/.cargo/registry.local/index"
EOF

  local result
  result=$(
    export CARGO_REGISTRIES_LOCAL_INDEX="file:///C:/Users/newuser/.cargo/registry.local/index"
    set +euo pipefail
    REPO_ROOT="$tmpdir"
    source "$tmpdir/lib/common.sh"
    REPO_ROOT="$tmpdir"
    ensure_registry
    cat "$tmpdir/Cargo.lock"
  )
  rm -rf "$tmpdir"
  assert_contains "$result" "registry+file:///C:/Users/newuser/.cargo/registry.local/index" \
    "Cargo.lock should have new registry path"
  assert_not_contains "$result" "registry+file:///mnt/c/Users/olduser/.cargo/registry.local/index" \
    "Cargo.lock should not have old registry path"
}

test_ensure_registry_skips_cargo_lock_when_path_matches() {
  local tmpdir
  tmpdir=$(mktemp -d)
  cp -r "$REPO_ROOT/lib" "$tmpdir/lib"

  cat > "$tmpdir/Cargo.lock" << 'EOF'
[[package]]
name = "some-crate"
version = "0.1.0"
source = "registry+file:///C:/Users/elvis/.cargo/registry.local/index"
EOF

  local result
  result=$(
    export CARGO_REGISTRIES_LOCAL_INDEX="file:///C:/Users/elvis/.cargo/registry.local/index"
    set +euo pipefail
    REPO_ROOT="$tmpdir"
    source "$tmpdir/lib/common.sh"
    REPO_ROOT="$tmpdir"
    ensure_registry 2>&1
  )
  rm -rf "$tmpdir"
  assert_not_contains "$result" "synced registry paths" \
    "should not print sync message when path already matches"
}

test_ensure_registry_noops_cargo_lock_when_file_missing() {
  local tmpdir
  tmpdir=$(mktemp -d)
  cp -r "$REPO_ROOT/lib" "$tmpdir/lib"
  # No Cargo.lock created

  local ec
  (
    export CARGO_REGISTRIES_LOCAL_INDEX="file:///C:/Users/elvis/.cargo/registry.local/index"
    set +euo pipefail
    REPO_ROOT="$tmpdir"
    source "$tmpdir/lib/common.sh"
    REPO_ROOT="$tmpdir"
    ensure_registry
  )
  ec=$?
  rm -rf "$tmpdir"
  assert_exit_code 0 "$ec" "ensure_registry should no-op when Cargo.lock is missing"
}

test_ensure_registry_preserves_cratesio_in_cargo_lock() {
  local tmpdir
  tmpdir=$(mktemp -d)
  cp -r "$REPO_ROOT/lib" "$tmpdir/lib"

  cat > "$tmpdir/Cargo.lock" << 'EOF'
[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "local-crate"
version = "0.1.0"
source = "registry+file:///mnt/c/Users/old/.cargo/registry.local/index"
EOF

  local result
  result=$(
    export CARGO_REGISTRIES_LOCAL_INDEX="file:///C:/Users/new/.cargo/registry.local/index"
    set +euo pipefail
    REPO_ROOT="$tmpdir"
    source "$tmpdir/lib/common.sh"
    REPO_ROOT="$tmpdir"
    ensure_registry
    cat "$tmpdir/Cargo.lock"
  )
  rm -rf "$tmpdir"
  assert_contains "$result" "registry+https://github.com/rust-lang/crates.io-index" \
    "crates.io source should be unchanged"
  assert_contains "$result" "registry+file:///C:/Users/new/.cargo/registry.local/index" \
    "local registry path should be updated"
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

test_load_env_ignores_comment_lines() {
  local tmpdir result
  tmpdir=$(mktemp -d)
  cp -r "$REPO_ROOT/lib" "$tmpdir/lib"
  printf '# this is a comment\nACTUAL_VAR=real_value\n' > "$tmpdir/.env"

  result=$(
    set +euo pipefail
    REPO_ROOT="$tmpdir"
    source "$tmpdir/lib/common.sh"
    REPO_ROOT="$tmpdir"
    load_env
    echo "$ACTUAL_VAR"
  )
  rm -rf "$tmpdir"
  assert_eq "real_value" "$result" "load_env should ignore comments and set real vars"
}

test_load_env_handles_quoted_values() {
  local tmpdir result
  tmpdir=$(mktemp -d)
  cp -r "$REPO_ROOT/lib" "$tmpdir/lib"
  echo 'QUOTED_VAR="hello world"' > "$tmpdir/.env"

  result=$(
    set +euo pipefail
    REPO_ROOT="$tmpdir"
    source "$tmpdir/lib/common.sh"
    REPO_ROOT="$tmpdir"
    load_env
    echo "$QUOTED_VAR"
  )
  rm -rf "$tmpdir"
  assert_eq "hello world" "$result" "load_env should handle quoted values"
}

test_load_env_noops_when_file_missing() {
  local tmpdir
  tmpdir=$(mktemp -d)
  cp -r "$REPO_ROOT/lib" "$tmpdir/lib"
  # no .env file created

  local ec
  (
    set +euo pipefail
    REPO_ROOT="$tmpdir"
    source "$tmpdir/lib/common.sh"
    REPO_ROOT="$tmpdir"
    load_env
  )
  ec=$?
  rm -rf "$tmpdir"
  assert_exit_code 0 "$ec" "load_env should no-op when .env is missing"
}
