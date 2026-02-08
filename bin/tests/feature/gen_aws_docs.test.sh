#!/usr/bin/env bash
# Unit tests for bin/gen-aws-docs.sh

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"

SCRIPT="$REPO_ROOT/bin/gen-aws-docs.sh"
SBH="$REPO_ROOT/sbh"

# -- Helper: run script (Windows-compatible) ---------------------------
# On Windows/MINGW, direct 'bash script.sh' fails silently. Use bash -c instead.
_run_script() {
  local script="$1"; shift
  local abs_script
  abs_script=$(cd "$(dirname "$script")" && pwd)/$(basename "$script")

  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
      bash -c "$(cat "$abs_script")" "$abs_script" "$@"
      ;;
    *)
      bash "$script" "$@"
      ;;
  esac
}

# -- Helper: source script functions without executing main ------------
# Sources lib/common.sh and all function definitions from gen-aws-docs.sh
# but skips the `source` line (we handle it) and the `main "$@"` call.
_load_functions() {
  source "$REPO_ROOT/lib/common.sh"
  eval "$(sed -e '/^source /d' -e '/^main "\$@"/d' "$SCRIPT")"
}

# -- Helper: create a mock aws CLI -------------------------------------
# Creates a fake `aws` binary in a temp dir and returns the dir path.
# The mock handles: --version, <service> help
_create_aws_mock() {
  local mock_dir
  mock_dir=$(mktemp -d)
  cat > "$mock_dir/aws" << 'MOCKEOF'
#!/usr/bin/env bash
# Mock aws CLI for testing gen-aws-docs.sh
if [ "$1" = "--version" ]; then
  echo "aws-cli/2.0.0-mock Python/3.9.0 Linux/5.15.0 source/x86_64"
  exit 0
fi

# Handle: aws <service> help
service="$1"
if [ "$2" = "help" ]; then
  upper=$(echo "$service" | tr '[:lower:]' '[:upper:]')
  cat << EOF
${upper}()                                                                ${upper}()

NAME
       ${service} -

DESCRIPTION
       Amazon ${upper} service provides functionality for ${service} operations.
       Use this service to manage your ${upper} resources in the cloud.
       Additional description line for testing truncation.

SYNOPSIS
          aws ${service} <command> [<args>...]

AVAILABLE COMMANDS
       o create-${service}-resource

       o delete-${service}-resource

       o describe-${service}-resources

       o list-${service}-items

       o update-${service}-resource

       o help

OPTIONS
       None

SEE ALSO
       aws help
EOF
  exit 0
fi

echo "Unknown command: $*" >&2
exit 1
MOCKEOF
  chmod +x "$mock_dir/aws"
  echo "$mock_dir"
}

# =====================================================================
# Pure function tests: strip_formatting
# =====================================================================

test_strip_formatting_removes_backspace_bold() {
  local result
  result=$(
    _load_functions
    # Simulate bold: each char preceded by char + backspace (e.g. N^HN)
    printf 'N\x08Na\x08am\x08me\x08e\n' | strip_formatting
  )
  assert_eq "Name" "$result" "should remove backspace-based bold sequences"
}

test_strip_formatting_removes_ansi_escape_codes() {
  local result
  result=$(
    _load_functions
    printf '\033[1mBOLD\033[0m normal\n' | strip_formatting
  )
  assert_eq "BOLD normal" "$result" "should remove ANSI escape codes"
}

test_strip_formatting_removes_carriage_returns() {
  local result
  result=$(
    _load_functions
    printf 'line one\r\nline two\r\n' | strip_formatting
  )
  assert_contains "$result" "line one" "should preserve line content"
  assert_not_contains "$result" $'\r' "should strip carriage returns"
}

test_strip_formatting_passes_clean_text_through() {
  local result
  result=$(
    _load_functions
    printf 'clean text no formatting\n' | strip_formatting
  )
  assert_eq "clean text no formatting" "$result" "clean text should pass through unchanged"
}

# =====================================================================
# Pure function tests: extract_synopsis
# =====================================================================

test_extract_synopsis_captures_synopsis_section() {
  local input result
  input=$(cat << 'EOF'
NAME
       s3 -

SYNOPSIS
          aws s3 <command> [<args>...]

DESCRIPTION
       Amazon S3 service.

AVAILABLE COMMANDS
       o ls
EOF
  )
  result=$(
    _load_functions
    extract_synopsis "$input"
  )
  assert_contains "$result" "aws s3" "should contain the synopsis command"
}

test_extract_synopsis_captures_description() {
  local input result
  input=$(cat << 'EOF'
SYNOPSIS
          aws ec2 <command> [<args>...]

DESCRIPTION
       Amazon Elastic Compute Cloud provides compute capacity.

AVAILABLE COMMANDS
       o run-instances
EOF
  )
  result=$(
    _load_functions
    extract_synopsis "$input"
  )
  assert_contains "$result" "Elastic Compute Cloud" "should contain description text"
}

test_extract_synopsis_stops_at_available_commands() {
  local input result
  input=$(cat << 'EOF'
SYNOPSIS
          aws s3 <command>

DESCRIPTION
       S3 service.

AVAILABLE COMMANDS
       o ls

       o cp
EOF
  )
  result=$(
    _load_functions
    extract_synopsis "$input"
  )
  assert_not_contains "$result" "o ls" "should not include AVAILABLE COMMANDS content"
}

test_extract_synopsis_truncates_long_description_to_8_lines() {
  local input result
  input="SYNOPSIS"$'\n'"  aws test"$'\n'"DESCRIPTION"
  for i in $(seq 1 15); do
    input+=$'\n'"  Description line $i"
  done
  input+=$'\n'"AVAILABLE COMMANDS"

  result=$(
    _load_functions
    extract_synopsis "$input"
  )
  assert_not_contains "$result" "line 12" "should truncate description beyond 8 lines"
}

test_extract_synopsis_stops_at_options_section() {
  local input result
  input=$(cat << 'EOF'
SYNOPSIS
          aws iam <command>

DESCRIPTION
       IAM service.

OPTIONS
       --debug
EOF
  )
  result=$(
    _load_functions
    extract_synopsis "$input"
  )
  assert_not_contains "$result" "--debug" "should stop before OPTIONS"
}

# =====================================================================
# Pure function tests: extract_subcommands
# =====================================================================

test_extract_subcommands_parses_bullet_list() {
  local input result
  input=$(cat << 'EOF'
DESCRIPTION
       A service.

AVAILABLE COMMANDS
       o create-thing

       o delete-thing

       o list-things

OPTIONS
       None
EOF
  )
  result=$(
    _load_functions
    extract_subcommands "$input"
  )
  assert_contains "$result" "create-thing" "should extract create-thing"
  assert_contains "$result" "delete-thing" "should extract delete-thing"
  assert_contains "$result" "list-things" "should extract list-things"
}

test_extract_subcommands_skips_help_command() {
  local input result
  input=$(cat << 'EOF'
AVAILABLE COMMANDS
       o describe-stuff

       o help

       o list-stuff

OPTIONS
       None
EOF
  )
  result=$(
    _load_functions
    extract_subcommands "$input"
  )
  assert_contains "$result" "describe-stuff" "should include describe-stuff"
  assert_contains "$result" "list-stuff" "should include list-stuff"
  assert_not_contains "$result" "help" "should exclude the help subcommand"
}

test_extract_subcommands_stops_at_next_section() {
  local input result
  input=$(cat << 'EOF'
AVAILABLE COMMANDS
       o alpha

       o bravo

EXAMPLES
       aws svc alpha --flag

SEE ALSO
       aws help
EOF
  )
  result=$(
    _load_functions
    extract_subcommands "$input"
  )
  assert_contains "$result" "alpha" "should include alpha"
  assert_contains "$result" "bravo" "should include bravo"
  assert_not_contains "$result" "EXAMPLES" "should not bleed into EXAMPLES"
}

test_extract_subcommands_limits_to_15() {
  local input
  input="AVAILABLE COMMANDS"$'\n'
  for i in $(seq 1 20); do
    input+="       o cmd-$(printf '%02d' "$i")"$'\n\n'
  done
  input+="OPTIONS"$'\n'

  local result
  result=$(
    _load_functions
    extract_subcommands "$input"
  )
  local count
  count=$(echo "$result" | grep -c '^cmd-')
  # Should have at most 15
  if [ "$count" -gt 15 ]; then
    echo "  FAIL: expected <= 15 subcommands, got $count" >&2
    return 1
  fi
  assert_contains "$result" "cmd-01" "should include first command"
  assert_contains "$result" "cmd-15" "should include 15th command"
}

test_extract_subcommands_handles_asterisk_bullets() {
  local input result
  input=$(cat << 'EOF'
AVAILABLE SUBCOMMANDS
       * get-object

       * put-object

OPTIONS
       None
EOF
  )
  result=$(
    _load_functions
    extract_subcommands "$input"
  )
  assert_contains "$result" "get-object" "should parse asterisk-style bullets"
  assert_contains "$result" "put-object" "should parse asterisk-style bullets"
}

test_extract_subcommands_returns_empty_when_no_commands_section() {
  local input result
  input=$(cat << 'EOF'
SYNOPSIS
       aws svc <command>

DESCRIPTION
       A service.

OPTIONS
       None
EOF
  )
  result=$(
    _load_functions
    extract_subcommands "$input"
  )
  assert_eq "" "$result" "should return empty when no AVAILABLE COMMANDS section"
}

# =====================================================================
# require_cmd
# =====================================================================

test_require_cmd_returns_0_for_existing_command() {
  local ec
  (
    _load_functions
    require_cmd bash
  ) >/dev/null 2>&1
  ec=$?
  assert_exit_code 0 "$ec" "require_cmd should succeed for 'bash'"
}

test_require_cmd_returns_1_for_missing_command() {
  local ec
  (
    _load_functions
    require_cmd nonexistent_command_xyz_123
  ) >/dev/null 2>&1
  ec=$?
  assert_exit_code 1 "$ec" "require_cmd should fail for missing command"
}

test_require_cmd_prints_error_to_stderr() {
  local err
  err=$(
    _load_functions
    require_cmd nonexistent_cmd_abc 2>&1 1>/dev/null
  ) || true
  assert_contains "$err" "not found" "should print 'not found' error"
}

# =====================================================================
# Script-level tests: non-interactive failure without aws
# =====================================================================

test_noninteractive_exits_1_without_aws() {
  # Run with empty PATH so aws isn't found, piped stdin (non-interactive)
  run_script "$SCRIPT" < /dev/null
  assert_exit_code 1 "$EXIT_CODE" "should exit 1 when aws not found non-interactively"
}

test_noninteractive_prints_install_url() {
  local combined
  combined=$( (_run_script "$SCRIPT" < /dev/null) 2>&1 ) || true
  assert_contains "$combined" "docs.aws.amazon.com" "should print AWS install URL"
}

test_noninteractive_mentions_aws_cli_not_found() {
  local combined
  combined=$( (_run_script "$SCRIPT" < /dev/null) 2>&1 ) || true
  assert_contains "$combined" "AWS CLI not found" "should say AWS CLI not found"
}

# =====================================================================
# Integration tests with mock aws
# =====================================================================

test_mock_aws_generates_three_files() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  local combined ec
  combined=$(
    PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" 2>&1
  ) || true
  ec=$?

  local have_svc have_iac have_tbl
  [ -f "$out_dir/services_reference.md" ] && have_svc=1 || have_svc=0
  [ -f "$out_dir/iac_patterns.md" ] && have_iac=1 || have_iac=0
  [ -f "$out_dir/troubleshooting.md" ] && have_tbl=1 || have_tbl=0

  rm -rf "$mock_dir" "$out_dir"

  assert_eq 1 "$have_svc" "services_reference.md should exist"
  assert_eq 1 "$have_iac" "iac_patterns.md should exist"
  assert_eq 1 "$have_tbl" "troubleshooting.md should exist"
}

test_mock_services_reference_contains_expected_sections() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local content=""
  [ -f "$out_dir/services_reference.md" ] && content=$(cat "$out_dir/services_reference.md")
  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$content" "EC2" "should contain EC2 section"
  assert_contains "$content" "S3" "should contain S3 section"
  assert_contains "$content" "LAMBDA" "should contain LAMBDA section"
}

test_mock_services_reference_contains_subcommands() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local content=""
  [ -f "$out_dir/services_reference.md" ] && content=$(cat "$out_dir/services_reference.md")
  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$content" "create-ec2-resource" "should list mock ec2 subcommands"
  assert_contains "$content" "describe-s3-resources" "should list mock s3 subcommands"
}

test_mock_services_reference_has_version_comment() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local content=""
  [ -f "$out_dir/services_reference.md" ] && content=$(cat "$out_dir/services_reference.md")
  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$content" "aws-cli/2.0.0-mock" "should embed aws version in comment"
}

test_mock_iac_patterns_contains_cdk_sam_terraform() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local content=""
  [ -f "$out_dir/iac_patterns.md" ] && content=$(cat "$out_dir/iac_patterns.md")
  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$content" "CDK" "should contain CDK section"
  assert_contains "$content" "SAM" "should contain SAM section"
  assert_contains "$content" "Terraform" "should contain Terraform section"
}

test_mock_iac_patterns_contains_cloudformation_recipes() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local content=""
  [ -f "$out_dir/iac_patterns.md" ] && content=$(cat "$out_dir/iac_patterns.md")
  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$content" "cloudformation deploy" "should have cfn deploy recipe"
  assert_contains "$content" "validate-template" "should have cfn validate recipe"
}

test_mock_troubleshooting_contains_auth_section() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local content=""
  [ -f "$out_dir/troubleshooting.md" ] && content=$(cat "$out_dir/troubleshooting.md")
  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$content" "Authentication" "should contain Authentication section"
  assert_contains "$content" "get-caller-identity" "should have sts identity check"
}

test_mock_troubleshooting_contains_debug_section() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local content=""
  [ -f "$out_dir/troubleshooting.md" ] && content=$(cat "$out_dir/troubleshooting.md")
  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$content" "Debugging" "should contain Debugging section"
  assert_contains "$content" "--debug" "should mention --debug flag"
}

test_mock_troubleshooting_contains_error_tables() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local content=""
  [ -f "$out_dir/troubleshooting.md" ] && content=$(cat "$out_dir/troubleshooting.md")
  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$content" "ExpiredTokenException" "should list ExpiredTokenException"
  assert_contains "$content" "ThrottlingException" "should list ThrottlingException"
}

# =====================================================================
# Output directory override
# =====================================================================

test_output_dir_override_via_env_var() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)/custom/path

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local exists=0
  [ -d "$out_dir" ] && exists=1

  rm -rf "$mock_dir" "$(dirname "$out_dir")"
  assert_eq 1 "$exists" "should create custom output directory from SWEBASH_AWS_DOCS_DIR"
}

test_output_prints_byte_counts() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  local combined
  combined=$(
    PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" 2>&1
  ) || true

  rm -rf "$mock_dir" "$out_dir"

  assert_contains "$combined" "bytes" "should print byte counts"
  assert_contains "$combined" "Done: 3 files" "should print completion summary"
}

# =====================================================================
# sbh dispatch
# =====================================================================

test_sbh_help_lists_gen_aws_docs() {
  run_script "$SBH" --help
  assert_contains "$STDOUT" "gen-aws-docs" "sbh --help should list gen-aws-docs command"
}

test_sbh_dispatches_gen_aws_docs() {
  # Should fail (no aws) but should route to the right script, not show usage
  local combined
  combined=$( (bash "$SBH" gen-aws-docs < /dev/null) 2>&1 ) || true
  assert_not_contains "$combined" "Usage:" "should not show sbh usage (should dispatch to script)"
}

# =====================================================================
# Budget guard
# =====================================================================

test_services_reference_stays_within_budget() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local size=0
  [ -f "$out_dir/services_reference.md" ] && size=$(wc -c < "$out_dir/services_reference.md")

  rm -rf "$mock_dir" "$out_dir"

  # Budget is 15000 chars per file — allow some margin for the truncation message
  if [ "$size" -gt 16000 ]; then
    echo "  FAIL: services_reference.md is $size bytes, exceeds budget" >&2
    return 1
  fi
  assert_eq 0 0 "services_reference.md ($size bytes) is within budget"
}

test_total_output_within_48k_budget() {
  local mock_dir out_dir
  mock_dir=$(_create_aws_mock)
  out_dir=$(mktemp -d)

  PATH="$mock_dir:$PATH" SWEBASH_AWS_DOCS_DIR="$out_dir" _run_script "$SCRIPT" >/dev/null 2>&1 || true

  local total=0
  for f in "$out_dir"/*.md; do
    [ -f "$f" ] && total=$((total + $(wc -c < "$f")))
  done

  rm -rf "$mock_dir" "$out_dir"

  if [ "$total" -gt 48000 ]; then
    echo "  FAIL: total output is $total bytes, exceeds 48k budget" >&2
    return 1
  fi
  assert_eq 0 0 "total output ($total bytes) within 48k budget"
}

# =====================================================================
# Configurable URL tests
# =====================================================================

test_help_shows_url_env_vars() {
  local combined
  combined=$( (_run_script "$SCRIPT" --help) 2>&1 ) || true
  assert_contains "$combined" "SWEBASH_AWS_CLI_URL_LINUX_X64" "help should list Linux x64 URL env var"
  assert_contains "$combined" "SWEBASH_AWS_CLI_URL_LINUX_ARM" "help should list Linux ARM URL env var"
  assert_contains "$combined" "SWEBASH_AWS_CLI_URL_MACOS" "help should list macOS URL env var"
  assert_contains "$combined" "SWEBASH_AWS_CLI_URL_WINDOWS" "help should list Windows URL env var"
}

test_url_vars_default_to_aws_urls() {
  local result
  result=$(
    _load_functions
    echo "$AWS_CLI_URL_LINUX_X64"
  )
  assert_contains "$result" "awscli.amazonaws.com" "Linux x64 URL should default to AWS"
  assert_contains "$result" "x86_64" "Linux x64 URL should contain x86_64"
}

test_url_vars_linux_arm_default() {
  local result
  result=$(
    _load_functions
    echo "$AWS_CLI_URL_LINUX_ARM"
  )
  assert_contains "$result" "awscli.amazonaws.com" "Linux ARM URL should default to AWS"
  assert_contains "$result" "aarch64" "Linux ARM URL should contain aarch64"
}

test_url_vars_macos_default() {
  local result
  result=$(
    _load_functions
    echo "$AWS_CLI_URL_MACOS"
  )
  assert_contains "$result" "awscli.amazonaws.com" "macOS URL should default to AWS"
  assert_contains "$result" ".pkg" "macOS URL should be .pkg"
}

test_url_vars_windows_default() {
  local result
  result=$(
    _load_functions
    echo "$AWS_CLI_URL_WINDOWS"
  )
  assert_contains "$result" "awscli.amazonaws.com" "Windows URL should default to AWS"
  assert_contains "$result" ".msi" "Windows URL should be .msi"
}

test_url_override_via_env_var() {
  local result
  result=$(
    export SWEBASH_AWS_CLI_URL_LINUX_X64="https://mirror.example.com/aws-cli.zip"
    _load_functions
    echo "$AWS_CLI_URL_LINUX_X64"
  )
  assert_eq "https://mirror.example.com/aws-cli.zip" "$result" "should use custom URL from env var"
}

test_url_override_macos_via_env_var() {
  local result
  result=$(
    export SWEBASH_AWS_CLI_URL_MACOS="https://corp.mirror/AWSCLIV2.pkg"
    _load_functions
    echo "$AWS_CLI_URL_MACOS"
  )
  assert_eq "https://corp.mirror/AWSCLIV2.pkg" "$result" "should use custom macOS URL from env var"
}

test_url_override_windows_via_env_var() {
  local result
  result=$(
    export SWEBASH_AWS_CLI_URL_WINDOWS="https://internal/AWSCLIV2.msi"
    _load_functions
    echo "$AWS_CLI_URL_WINDOWS"
  )
  assert_eq "https://internal/AWSCLIV2.msi" "$result" "should use custom Windows URL from env var"
}

# =====================================================================
# Help flag tests
# =====================================================================

test_help_flag_exits_0() {
  run_script "$SCRIPT" --help
  assert_exit_code 0 "$EXIT_CODE" "--help should exit 0"
}

test_h_flag_exits_0() {
  run_script "$SCRIPT" -h
  assert_exit_code 0 "$EXIT_CODE" "-h should exit 0"
}

test_help_shows_install_flag() {
  local combined
  combined=$( (_run_script "$SCRIPT" --help) 2>&1 ) || true
  assert_contains "$combined" "--install" "help should mention --install flag"
  assert_contains "$combined" "-y" "help should mention -y alias"
}

test_help_shows_env_vars() {
  local combined
  combined=$( (_run_script "$SCRIPT" --help) 2>&1 ) || true
  assert_contains "$combined" "SWEBASH_AWS_INSTALL" "help should mention SWEBASH_AWS_INSTALL"
  assert_contains "$combined" "SWEBASH_AWS_DOCS_DIR" "help should mention SWEBASH_AWS_DOCS_DIR"
}
