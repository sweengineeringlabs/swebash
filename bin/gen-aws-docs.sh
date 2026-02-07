#!/usr/bin/env bash
# bin/gen-aws-docs.sh — generate AWS reference docs from live CLI help
#
# Sources: aws <service> help, cdk --help, sam --help, terraform --help
# Output:  ~/.config/swebash/docs/aws/{services_reference,iac_patterns,troubleshooting}.md
#
# Usage:   ./sbh gen-aws-docs          (default services)
#          bash bin/gen-aws-docs.sh     (direct invocation)

source "$(cd "$(dirname "$0")/.." && pwd)/lib/common.sh"

# ── Configuration ─────────────────────────────────────────────────────
OUTPUT_DIR="${SWEBASH_AWS_DOCS_DIR:-$HOME/.config/swebash/docs/aws}"

# Services to document (order determines output order)
SERVICES=(
  ec2 s3 lambda iam cloudformation
  ecs eks rds dynamodb
  sqs sns cloudwatch route53
)

# Per-file character budget (12k tokens ≈ 48k chars; split across 3 files)
CHAR_BUDGET=15000

# ── Helpers ───────────────────────────────────────────────────────────

require_cmd() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: $1 not found in PATH. Install it first." >&2
    return 1
  fi
}

# ── AWS CLI installer (opt-in) ────────────────────────────────────────

install_aws_cli() {
  local arch
  arch=$(uname -m)
  local platform
  platform=$(uname -s)

  # Determine download URL
  local zip_url=""
  case "$platform" in
    Linux)
      case "$arch" in
        x86_64)  zip_url="https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" ;;
        aarch64) zip_url="https://awscli.amazonaws.com/awscli-exe-linux-aarch64.zip" ;;
        *)
          echo "ERROR: Unsupported architecture: $arch" >&2
          return 1
          ;;
      esac
      ;;
    Darwin)
      zip_url="https://awscli.amazonaws.com/AWSCLIV2.pkg"
      ;;
    *)
      echo "ERROR: Unsupported platform: $platform" >&2
      echo "  Install manually: https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html" >&2
      return 1
      ;;
  esac

  # Check prerequisites
  for dep in curl unzip; do
    if ! command -v "$dep" &>/dev/null; then
      echo "ERROR: $dep is required for installation but not found." >&2
      return 1
    fi
  done

  # Choose install mode
  local install_dir="$HOME/.local/aws-cli"
  local bin_dir="$HOME/.local/bin"
  local use_sudo=false

  echo ""
  echo "Install AWS CLI v2?"
  echo "  [1] Local install to ~/.local  (no sudo required) — recommended"
  echo "  [2] System install to /usr/local  (requires sudo)"
  echo "  [n] Skip — exit without installing"
  echo ""
  read -rp "Choice [1/2/n]: " choice

  case "$choice" in
    1)
      use_sudo=false
      install_dir="$HOME/.local/aws-cli"
      bin_dir="$HOME/.local/bin"
      ;;
    2)
      use_sudo=true
      install_dir="/usr/local/aws-cli"
      bin_dir="/usr/local/bin"
      ;;
    *)
      echo "Skipping install."
      return 1
      ;;
  esac

  # macOS uses .pkg installer with different flow
  if [ "$platform" = "Darwin" ]; then
    echo "==> macOS detected — downloading .pkg installer..."
    local pkg_file
    pkg_file=$(mktemp /tmp/awscli-XXXXXX.pkg)
    curl -fSL "$zip_url" -o "$pkg_file"
    if [ "$use_sudo" = true ]; then
      sudo installer -pkg "$pkg_file" -target /
    else
      installer -pkg "$pkg_file" -target CurrentUserHomeDirectory
    fi
    rm -f "$pkg_file"
    echo "==> AWS CLI installed (macOS). Run 'aws --version' to verify."
    return 0
  fi

  # Linux install flow
  local tmp_dir
  tmp_dir=$(mktemp -d /tmp/awscli-install-XXXXXX)
  trap 'rm -rf "$tmp_dir"' RETURN

  echo "==> Downloading AWS CLI v2 ($arch)..."
  curl -fSL "$zip_url" -o "$tmp_dir/awscliv2.zip"

  echo "==> Extracting..."
  unzip -q "$tmp_dir/awscliv2.zip" -d "$tmp_dir"

  echo "==> Installing to $install_dir (bin: $bin_dir)..."
  mkdir -p "$bin_dir"
  if [ "$use_sudo" = true ]; then
    sudo "$tmp_dir/aws/install" --install-dir "$install_dir" --bin-dir "$bin_dir" --update
  else
    "$tmp_dir/aws/install" --install-dir "$install_dir" --bin-dir "$bin_dir" --update
  fi

  # Verify installation
  if "$bin_dir/aws" --version &>/dev/null; then
    echo "==> Installed: $("$bin_dir/aws" --version)"
  else
    echo "ERROR: Installation completed but 'aws' not working at $bin_dir/aws" >&2
    return 1
  fi

  # Ensure bin_dir is on PATH for the rest of this script
  if ! echo "$PATH" | tr ':' '\n' | grep -qx "$bin_dir"; then
    export PATH="$bin_dir:$PATH"
    echo ""
    echo "NOTE: $bin_dir is not in your PATH."
    echo "  Add to your shell profile:"
    echo "    echo 'export PATH=\"$bin_dir:\$PATH\"' >> ~/.bashrc"
  fi
}

# Capture aws help output (bypasses pager)
aws_help() {
  AWS_PAGER="" aws "$@" help 2>/dev/null
}

# Strip ANSI escape codes and rST formatting noise from aws help output
strip_formatting() {
  # Remove backspace-based bold/underline (e.g. _^Hx), ANSI escapes, and leading whitespace noise
  # Note: $'\x08' is ANSI-C quoting — embeds a literal backspace byte in the sed pattern
  sed $'s/.\x08//g' \
    | sed 's/\x1b\[[0-9;]*m//g' \
    | sed 's/\r//g'
}

# Extract the SYNOPSIS and DESCRIPTION sections from aws help output
extract_synopsis() {
  local raw="$1"
  local synopsis="" description="" in_section=""

  while IFS= read -r line; do
    case "$line" in
      SYNOPSIS*) in_section="synopsis" ;;
      DESCRIPTION*) in_section="description" ;;
      AVAILABLE\ COMMANDS*|AVAILABLE\ SUBCOMMANDS*|OPTIONS*|EXAMPLES*|SEE\ ALSO*|OUTPUT*|GLOBAL\ FLAGS*|GLOBAL\ OPTIONS*)
        if [ "$in_section" = "description" ]; then
          break
        fi
        in_section=""
        ;;
    esac
    case "$in_section" in
      synopsis) synopsis+="$line"$'\n' ;;
      description) description+="$line"$'\n' ;;
    esac
  done <<< "$raw"

  # Trim and truncate description to keep things concise
  description=$(echo "$description" | head -8)
  printf '%s\n%s' "$synopsis" "$description"
}

# Extract subcommand names from AVAILABLE COMMANDS section
extract_subcommands() {
  local raw="$1"
  local in_section="" commands=()

  while IFS= read -r line; do
    case "$line" in
      AVAILABLE\ COMMANDS*|AVAILABLE\ SUBCOMMANDS*) in_section="commands" ;;
      SYNOPSIS*|DESCRIPTION*|OPTIONS*|EXAMPLES*|SEE\ ALSO*|OUTPUT*|GLOBAL*)
        [ "$in_section" = "commands" ] && break
        ;;
    esac
    if [ "$in_section" = "commands" ]; then
      # Subcommands appear as indented "o <name>" or "* <name>" or just indented words
      local cmd
      cmd=$(echo "$line" | sed -n 's/^[[:space:]]*o[[:space:]]\+\([a-z][-a-z0-9]*\).*/\1/p')
      [ -z "$cmd" ] && cmd=$(echo "$line" | sed -n 's/^[[:space:]]*\*[[:space:]]\+\([a-z][-a-z0-9]*\).*/\1/p')
      if [ -n "$cmd" ] && [ "$cmd" != "help" ]; then
        commands+=("$cmd")
      fi
    fi
  done <<< "$raw"

  # Return first 15 subcommands
  printf '%s\n' "${commands[@]:0:15}"
}

# ── Generate services_reference.md ────────────────────────────────────
generate_services_reference() {
  require_cmd aws || return 1

  local aws_version
  aws_version=$(aws --version 2>&1 | head -1)

  local out=""
  out+="# AWS CLI Services Reference"$'\n'
  out+="<!-- Generated by bin/gen-aws-docs.sh from: $aws_version -->"$'\n'
  out+=$'\n'

  for svc in "${SERVICES[@]}"; do
    echo "  $svc..." >&2
    local raw
    raw=$(aws_help "$svc" | strip_formatting) || continue

    local upper_svc
    upper_svc=$(echo "$svc" | tr '[:lower:]' '[:upper:]')
    out+="## $upper_svc"$'\n\n'

    # Synopsis
    local synopsis
    synopsis=$(extract_synopsis "$raw")
    if [ -n "$synopsis" ]; then
      out+='```'$'\n'"$synopsis"$'\n''```'$'\n\n'
    fi

    # Key subcommands
    local subcmds
    subcmds=$(extract_subcommands "$raw")
    if [ -n "$subcmds" ]; then
      out+="**Key commands:**"$'\n'
      while IFS= read -r cmd; do
        [ -n "$cmd" ] && out+="- \`aws $svc $cmd\`"$'\n'
      done <<< "$subcmds"
      out+=$'\n'
    fi

    # Budget guard: stop adding services if we're over budget
    if [ "${#out}" -gt "$CHAR_BUDGET" ]; then
      out+=$'\n'"*Truncated at ${#SERVICES[@]} services to stay within budget.*"$'\n'
      break
    fi
  done

  echo "$out"
}

# ── Generate iac_patterns.md ──────────────────────────────────────────
generate_iac_patterns() {
  local out=""
  out+="# Infrastructure as Code Patterns"$'\n'
  out+="<!-- Generated by bin/gen-aws-docs.sh -->"$'\n'
  out+=$'\n'

  # CloudFormation (via aws cli)
  if command -v aws &>/dev/null; then
    echo "  CloudFormation (aws cli)..." >&2
    local cfn_raw
    cfn_raw=$(aws_help cloudformation | strip_formatting)
    local cfn_cmds
    cfn_cmds=$(extract_subcommands "$cfn_raw")

    out+="## CloudFormation (CFN)"$'\n\n'
    out+='```bash'$'\n'
    out+="# Validate template"$'\n'
    out+="aws cloudformation validate-template --template-body file://template.yaml"$'\n'
    out+=$'\n'
    out+="# Deploy stack"$'\n'
    out+="aws cloudformation deploy \\"$'\n'
    out+="  --template-file template.yaml \\"$'\n'
    out+="  --stack-name my-stack \\"$'\n'
    out+="  --capabilities CAPABILITY_IAM"$'\n'
    out+=$'\n'
    out+="# Describe stack events (troubleshoot failures)"$'\n'
    out+="aws cloudformation describe-stack-events --stack-name my-stack"$'\n'
    out+=$'\n'
    out+="# Delete stack"$'\n'
    out+="aws cloudformation delete-stack --stack-name my-stack"$'\n'
    out+='```'$'\n\n'

    if [ -n "$cfn_cmds" ]; then
      out+="**All CFN commands:** "
      out+=$(echo "$cfn_cmds" | tr '\n' ', ' | sed 's/,$//')
      out+=$'\n\n'
    fi
  fi

  # CDK
  if command -v cdk &>/dev/null; then
    echo "  CDK..." >&2
    local cdk_version
    cdk_version=$(cdk --version 2>&1 | head -1)
    local cdk_help
    cdk_help=$(cdk --help 2>&1 | head -30)

    out+="## AWS CDK"$'\n'
    out+="<!-- Source: cdk $cdk_version -->"$'\n\n'
    out+='```bash'$'\n'
    out+="$cdk_help"$'\n'
    out+='```'$'\n\n'
  else
    out+="## AWS CDK"$'\n\n'
    out+='```bash'$'\n'
    out+="# Install: npm install -g aws-cdk"$'\n'
    out+="cdk init app --language typescript"$'\n'
    out+="cdk synth          # Generate CloudFormation template"$'\n'
    out+="cdk diff           # Preview changes"$'\n'
    out+="cdk deploy         # Deploy stack"$'\n'
    out+="cdk destroy        # Tear down stack"$'\n'
    out+='```'$'\n\n'
  fi

  # SAM
  if command -v sam &>/dev/null; then
    echo "  SAM..." >&2
    local sam_version
    sam_version=$(sam --version 2>&1 | head -1)
    local sam_help
    sam_help=$(sam --help 2>&1 | head -30)

    out+="## AWS SAM"$'\n'
    out+="<!-- Source: $sam_version -->"$'\n\n'
    out+='```bash'$'\n'
    out+="$sam_help"$'\n'
    out+='```'$'\n\n'
  else
    out+="## AWS SAM"$'\n\n'
    out+='```bash'$'\n'
    out+="# Install: pip install aws-sam-cli"$'\n'
    out+="sam init                          # Scaffold new project"$'\n'
    out+="sam build                         # Build artifacts"$'\n'
    out+="sam local invoke MyFunction       # Test locally"$'\n'
    out+="sam local start-api               # Local API Gateway"$'\n'
    out+="sam deploy --guided               # Deploy to AWS"$'\n'
    out+="sam logs -n MyFunction --tail     # Tail function logs"$'\n'
    out+='```'$'\n\n'
  fi

  # Terraform
  if command -v terraform &>/dev/null; then
    echo "  Terraform..." >&2
    local tf_version
    tf_version=$(terraform version 2>&1 | head -1)
    local tf_help
    tf_help=$(terraform --help 2>&1 | head -30)

    out+="## Terraform"$'\n'
    out+="<!-- Source: $tf_version -->"$'\n\n'
    out+='```bash'$'\n'
    out+="$tf_help"$'\n'
    out+='```'$'\n\n'
  else
    out+="## Terraform"$'\n\n'
    out+='```bash'$'\n'
    out+="# Install: https://developer.hashicorp.com/terraform/install"$'\n'
    out+="terraform init       # Initialize provider plugins"$'\n'
    out+="terraform plan       # Preview changes"$'\n'
    out+="terraform apply      # Apply changes"$'\n'
    out+="terraform destroy    # Tear down infrastructure"$'\n'
    out+="terraform fmt        # Format HCL files"$'\n'
    out+="terraform validate   # Validate configuration"$'\n'
    out+='```'$'\n\n'
  fi

  echo "$out"
}

# ── Generate troubleshooting.md ───────────────────────────────────────
generate_troubleshooting() {
  local out=""
  out+="# AWS Troubleshooting Guide"$'\n'
  out+="<!-- Generated by bin/gen-aws-docs.sh -->"$'\n'
  out+=$'\n'

  # Authentication section — pull from sts help if available
  out+="## Authentication"$'\n\n'

  if command -v aws &>/dev/null; then
    echo "  sts (auth diagnostics)..." >&2
    local sts_raw
    sts_raw=$(aws_help sts | strip_formatting)
    local sts_cmds
    sts_cmds=$(extract_subcommands "$sts_raw")

    out+='```bash'$'\n'
    out+="# Verify current identity"$'\n'
    out+="aws sts get-caller-identity"$'\n'
    out+=$'\n'
    out+="# Check configured profiles"$'\n'
    out+="aws configure list"$'\n'
    out+="aws configure list-profiles"$'\n'
    out+=$'\n'
    out+="# Assume a role"$'\n'
    out+="aws sts assume-role --role-arn arn:aws:iam::ACCOUNT:role/ROLE --role-session-name mysession"$'\n'
    out+=$'\n'
    out+="# Decode authorization failure message"$'\n'
    out+="aws sts decode-authorization-message --encoded-message <message>"$'\n'
    out+='```'$'\n\n'

    if [ -n "$sts_cmds" ]; then
      out+="**STS commands:** "
      out+=$(echo "$sts_cmds" | tr '\n' ', ' | sed 's/,$//')
      out+=$'\n\n'
    fi
  fi

  out+="### Common auth errors"$'\n\n'
  out+="| Error | Cause | Fix |"$'\n'
  out+="|-------|-------|-----|"$'\n'
  out+="| \`ExpiredTokenException\` | Session/temp credentials expired | Run \`aws sso login\` or refresh credentials |"$'\n'
  out+="| \`InvalidClientTokenId\` | Access key ID is wrong or inactive | Check \`aws configure list\`, verify key in IAM console |"$'\n'
  out+="| \`SignatureDoesNotMatch\` | Secret key mismatch or clock skew | Re-run \`aws configure\`, check system clock (\`date\`) |"$'\n'
  out+="| \`AccessDenied\` | IAM policy doesn't allow action | Check policy with \`aws iam simulate-principal-policy\` |"$'\n'
  out+="| \`UnauthorizedAccess\` | No permission / SCP blocking | Check SCPs, permission boundaries, resource policies |"$'\n'
  out+=$'\n'

  # Debugging section
  out+="## Debugging"$'\n\n'
  out+='```bash'$'\n'
  out+="# Enable CLI debug output"$'\n'
  out+="aws s3 ls --debug 2>&1 | head -50"$'\n'
  out+=$'\n'
  out+="# Trace API calls with verbose HTTP logging"$'\n'
  out+="aws s3 ls --debug 2>&1 | grep -E '(MainThread|HTTPSConnection)'"$'\n'
  out+=$'\n'
  out+="# Check CLI configuration resolution"$'\n'
  out+="aws configure list"$'\n'
  out+=$'\n'
  out+="# Verify endpoint resolution"$'\n'
  out+="aws s3 ls --debug 2>&1 | grep 'endpoint_url'"$'\n'
  out+='```'$'\n\n'

  out+="### Debug environment variables"$'\n\n'
  out+='```bash'$'\n'
  out+="export AWS_DEBUG=true              # Enable SDK-level debug"$'\n'
  out+="export AWS_CA_BUNDLE=/path/to/cert # Custom CA (corp proxies)"$'\n'
  out+="export AWS_MAX_ATTEMPTS=5          # Retry count"$'\n'
  out+="export AWS_RETRY_MODE=adaptive     # Retry strategy"$'\n'
  out+="export AWS_DEFAULT_OUTPUT=json     # Output format: json|text|table"$'\n'
  out+='```'$'\n\n'

  # CloudWatch Logs — common debugging target
  if command -v aws &>/dev/null; then
    echo "  CloudWatch Logs (debugging)..." >&2
    out+="### CloudWatch Logs (log tailing)"$'\n\n'
    out+='```bash'$'\n'
    out+="# List log groups"$'\n'
    out+="aws logs describe-log-groups --query 'logGroups[].logGroupName' --output text"$'\n'
    out+=$'\n'
    out+="# Tail logs (live)"$'\n'
    out+="aws logs tail /aws/lambda/my-function --follow"$'\n'
    out+=$'\n'
    out+="# Filter log events"$'\n'
    out+="aws logs filter-log-events \\"$'\n'
    out+="  --log-group-name /aws/lambda/my-function \\"$'\n'
    out+="  --filter-pattern \"ERROR\" \\"$'\n'
    out+="  --start-time \$(date -d '1 hour ago' +%s)000"$'\n'
    out+='```'$'\n\n'
  fi

  out+="### Common operational errors"$'\n\n'
  out+="| Error | Service | Fix |"$'\n'
  out+="|-------|---------|-----|"$'\n'
  out+="| \`ThrottlingException\` | Any | Implement exponential backoff, request limit increase |"$'\n'
  out+="| \`ResourceNotFoundException\` | Any | Verify resource name/ARN and region (\`--region\`) |"$'\n'
  out+="| \`BucketAlreadyExists\` | S3 | S3 bucket names are globally unique — pick another |"$'\n'
  out+="| \`InstanceLimitExceeded\` | EC2 | Request service quota increase via Service Quotas |"$'\n'
  out+="| \`TooManyRequestsException\` | Lambda | Increase reserved concurrency or request limit bump |"$'\n'

  echo "$out"
}

# ── Main ──────────────────────────────────────────────────────────────
main() {
  echo "==> Generating AWS docs to $OUTPUT_DIR"

  if ! command -v aws &>/dev/null; then
    echo ""
    echo "AWS CLI not found."
    if [ -t 0 ]; then
      # Interactive terminal — offer to install
      install_aws_cli || {
        echo ""
        echo "Cannot generate docs without AWS CLI."
        echo "Install manually: https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
        exit 1
      }
    else
      # Non-interactive (piped/CI) — just fail
      echo "Install AWS CLI first, or run interactively for guided install."
      echo "  https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
      exit 1
    fi
  fi

  mkdir -p "$OUTPUT_DIR"

  echo "==> [1/3] services_reference.md"
  generate_services_reference > "$OUTPUT_DIR/services_reference.md"
  local size1
  size1=$(wc -c < "$OUTPUT_DIR/services_reference.md")
  echo "    wrote $size1 bytes"

  echo "==> [2/3] iac_patterns.md"
  generate_iac_patterns > "$OUTPUT_DIR/iac_patterns.md"
  local size2
  size2=$(wc -c < "$OUTPUT_DIR/iac_patterns.md")
  echo "    wrote $size2 bytes"

  echo "==> [3/3] troubleshooting.md"
  generate_troubleshooting > "$OUTPUT_DIR/troubleshooting.md"
  local size3
  size3=$(wc -c < "$OUTPUT_DIR/troubleshooting.md")
  echo "    wrote $size3 bytes"

  local total=$((size1 + size2 + size3))
  echo ""
  echo "==> Done: 3 files, $total bytes total"
  echo "    $OUTPUT_DIR/services_reference.md  ($size1 bytes)"
  echo "    $OUTPUT_DIR/iac_patterns.md        ($size2 bytes)"
  echo "    $OUTPUT_DIR/troubleshooting.md     ($size3 bytes)"

  if [ "$total" -gt 48000 ]; then
    echo ""
    echo "WARNING: Total output ($total bytes) exceeds 48k char target."
    echo "  Consider reducing SERVICES list or CHAR_BUDGET."
  fi
}

main "$@"
