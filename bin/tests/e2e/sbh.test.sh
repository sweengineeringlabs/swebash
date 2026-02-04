#!/usr/bin/env bash
# E2E tests for the sbh entrypoint

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"
SBH="$REPO_ROOT/sbh"

# -- Help / Usage tests ------------------------------------------------

test_help_flag_shows_usage_exit_0() {
  run_script "$SBH" --help
  assert_exit_code 0 "$EXIT_CODE" "sbh --help should exit 0"
  assert_contains "$STDOUT" "Usage" "should show usage"
}

test_h_flag_shows_usage_exit_0() {
  run_script "$SBH" -h
  assert_exit_code 0 "$EXIT_CODE" "sbh -h should exit 0"
  assert_contains "$STDOUT" "Usage" "should show usage"
}

test_help_command_shows_usage_exit_0() {
  run_script "$SBH" help
  assert_exit_code 0 "$EXIT_CODE" "sbh help should exit 0"
  assert_contains "$STDOUT" "Usage" "should show usage"
}

test_no_args_shows_usage_exit_0() {
  run_script "$SBH"
  assert_exit_code 0 "$EXIT_CODE" "sbh (no args) should exit 0"
  assert_contains "$STDOUT" "Usage" "should show usage"
}

# -- Unknown command ---------------------------------------------------

test_unknown_command_shows_usage_exit_1() {
  run_script "$SBH" "totally_invalid_command"
  assert_exit_code 1 "$EXIT_CODE" "unknown command should exit 1"
  assert_contains "$STDOUT" "Usage" "should show usage on unknown command"
}

# -- Help text completeness --------------------------------------------

test_help_lists_all_commands() {
  run_script "$SBH" --help
  assert_contains "$STDOUT" "setup" "help should list setup"
  assert_contains "$STDOUT" "build" "help should list build"
  assert_contains "$STDOUT" "run" "help should list run"
  assert_contains "$STDOUT" "test" "help should list test"
}

test_help_lists_all_test_suites() {
  run_script "$SBH" --help
  assert_contains "$STDOUT" "engine" "help should list engine suite"
  assert_contains "$STDOUT" "host" "help should list host suite"
  assert_contains "$STDOUT" "readline" "help should list readline suite"
  assert_contains "$STDOUT" "ai" "help should list ai suite"
  assert_contains "$STDOUT" "scripts" "help should list scripts suite"
}

# -- Command routing ---------------------------------------------------

test_command_routing_dispatches_to_correct_script() {
  local stub_dir
  stub_dir=$(mktemp -d)
  cp "$REPO_ROOT/sbh" "$stub_dir/sbh"
  mkdir -p "$stub_dir/bin"
  cat > "$stub_dir/bin/build.sh" << 'STUB'
#!/usr/bin/env bash
echo "DISPATCH_OK:build"
STUB
  chmod +x "$stub_dir/bin/build.sh"

  # Patch the sbh copy: set REPO_ROOT and replace exec with source
  # (exec in bash -c doesn't propagate stdout on MINGW)
  sed -i "s|REPO_ROOT=.*|REPO_ROOT=\"$stub_dir\"|" "$stub_dir/sbh"
  sed -i 's|exec "$REPO_ROOT/bin/build.sh"|source "$REPO_ROOT/bin/build.sh"|' "$stub_dir/sbh"

  run_script "$stub_dir/sbh" build
  assert_contains "$STDOUT" "DISPATCH_OK:build" "should dispatch to build.sh"

  rm -rf "$stub_dir"
}
