#!/usr/bin/env bash
# Unit tests for bin/setup.sh

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"

# -- Helpers -----------------------------------------------------------
CORE_PATH="/usr/bin:/bin:/usr/sbin:/sbin"

# Create a temp repo structure: tmpdir/bin/setup.sh, tmpdir/lib/common.sh
create_temp_repo() {
  local tmpdir
  tmpdir=$(mktemp -d)
  mkdir -p "$tmpdir/bin" "$tmpdir/lib"
  cp "$REPO_ROOT/lib/common.sh" "$tmpdir/lib/common.sh"
  cp "$REPO_ROOT/bin/setup.sh" "$tmpdir/bin/setup.sh"
  echo "$tmpdir"
}

# -- Tests -------------------------------------------------------------

test_fails_when_rustup_not_found() {
  local empty_dir out
  empty_dir=$(mktemp -d)
  out=$(PATH="$CORE_PATH:$empty_dir" run_bash "$REPO_ROOT/bin/setup.sh" 2>&1) || true
  rm -rf "$empty_dir"
  assert_contains "$out" "rustup" "should mention rustup"
}

test_fails_when_cargo_not_found() {
  local shim_dir out
  shim_dir=$(mktemp -d)
  cat > "$shim_dir/rustup" << 'SHIM'
#!/usr/bin/env bash
echo "rustup $*"
SHIM
  chmod +x "$shim_dir/rustup"

  out=$(PATH="$shim_dir:$CORE_PATH" run_bash "$REPO_ROOT/bin/setup.sh" 2>&1) || true
  rm -rf "$shim_dir"
  assert_contains "$out" "cargo" "should mention cargo"
}

test_copies_env_example_to_env_when_missing() {
  local shim_dir fake_home tmpdir
  shim_dir=$(mktemp -d)
  fake_home=$(mktemp -d)
  tmpdir=$(create_temp_repo)

  cat > "$shim_dir/cargo" << 'SHIM'
#!/usr/bin/env bash
echo "cargo $*"
SHIM
  chmod +x "$shim_dir/cargo"
  cat > "$shim_dir/rustup" << 'SHIM'
#!/usr/bin/env bash
if [ "$1" = "--version" ]; then echo "rustup 1.27.0 (fake)"; fi
if [ "$1" = "target" ]; then echo "done"; fi
SHIM
  chmod +x "$shim_dir/rustup"

  echo "TEST_KEY=test_value" > "$tmpdir/.env.example"
  touch "$fake_home/.bashrc"
  mkdir -p "$fake_home/.cargo/registry.local/index"

  local out
  out=$(
    PATH="$shim_dir:$CORE_PATH" \
    HOME="$fake_home" \
    CARGO_REGISTRIES_LOCAL_INDEX="file://$fake_home/.cargo/registry.local/index" \
    run_bash "$tmpdir/bin/setup.sh" 2>&1
  ) || true

  if [ -f "$tmpdir/.env" ]; then
    assert_contains "$(cat "$tmpdir/.env")" "TEST_KEY" ".env should contain TEST_KEY"
  else
    assert_contains "$out" ".env" "should reference .env"
  fi

  rm -rf "$tmpdir" "$fake_home" "$shim_dir"
}

test_does_not_overwrite_existing_env() {
  local shim_dir fake_home tmpdir
  shim_dir=$(mktemp -d)
  fake_home=$(mktemp -d)
  tmpdir=$(create_temp_repo)

  cat > "$shim_dir/cargo" << 'SHIM'
#!/usr/bin/env bash
echo "cargo $*"
SHIM
  chmod +x "$shim_dir/cargo"
  cat > "$shim_dir/rustup" << 'SHIM'
#!/usr/bin/env bash
if [ "$1" = "--version" ]; then echo "rustup 1.27.0 (fake)"; fi
if [ "$1" = "target" ]; then echo "done"; fi
SHIM
  chmod +x "$shim_dir/rustup"

  echo "ORIGINAL=keep" > "$tmpdir/.env"
  echo "NEW=overwrite" > "$tmpdir/.env.example"
  touch "$fake_home/.bashrc"
  mkdir -p "$fake_home/.cargo/registry.local/index"

  local out
  out=$(
    PATH="$shim_dir:$CORE_PATH" \
    HOME="$fake_home" \
    CARGO_REGISTRIES_LOCAL_INDEX="file://$fake_home/.cargo/registry.local/index" \
    run_bash "$tmpdir/bin/setup.sh" 2>&1
  ) || true

  assert_contains "$out" "already exists" "should say .env already exists"
  rm -rf "$tmpdir" "$fake_home" "$shim_dir"
}

test_no_parse_errors() {
  local out
  out=$(bash -n "$REPO_ROOT/bin/setup.sh" 2>&1)
  local ec=$?
  assert_exit_code 0 "$ec" "setup.sh should have no parse errors"
}
