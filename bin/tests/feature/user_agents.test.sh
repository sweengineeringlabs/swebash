#!/usr/bin/env bash
# Tests for user-level agents.yaml (awscli agent at ~/.config/swebash/)

TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/../.." && pwd)"

SWEBASH_BIN="/tmp/swebash-target/release/swebash"
USER_CONFIG="$HOME/.config/swebash/agents.yaml"
USER_DOCS_DIR="$HOME/.config/swebash/docs/aws"

# -- Prerequisite checks --------------------------------------------------

test_user_agents_yaml_exists() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found at $USER_CONFIG"
  fi
  assert_eq 0 0 "user agents.yaml exists"
}

test_user_agents_yaml_is_valid() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  # Basic YAML validation: must contain version and agents keys
  local content
  content=$(cat "$USER_CONFIG")
  assert_contains "$content" "version:" "should contain version key"
  assert_contains "$content" "agents:" "should contain agents key"
}

# -- awscli agent config validation ----------------------------------------

test_awscli_agent_defined_in_yaml() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  local content
  content=$(cat "$USER_CONFIG")
  assert_contains "$content" "id: awscli" "should define awscli agent"
}

test_awscli_agent_has_name() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  local content
  content=$(cat "$USER_CONFIG")
  assert_contains "$content" "name: AWS Cloud Assistant" "should have correct name"
}

test_awscli_agent_has_tools_config() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  local content
  content=$(cat "$USER_CONFIG")
  assert_contains "$content" "fs: true" "should have fs: true"
  assert_contains "$content" "exec: true" "should have exec: true"
  assert_contains "$content" "web: false" "should have web: false"
}

test_awscli_agent_has_max_iterations() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  local content
  content=$(cat "$USER_CONFIG")
  assert_contains "$content" "maxIterations: 25" "should have maxIterations: 25"
}

test_awscli_agent_has_trigger_keywords() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  local content
  content=$(cat "$USER_CONFIG")
  assert_contains "$content" "aws" "keywords should include aws"
  assert_contains "$content" "s3" "keywords should include s3"
  assert_contains "$content" "ec2" "keywords should include ec2"
  assert_contains "$content" "lambda" "keywords should include lambda"
  assert_contains "$content" "cloudformation" "keywords should include cloudformation"
  assert_contains "$content" "terraform" "keywords should include terraform"
}

test_awscli_agent_has_docs_config() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  local content
  content=$(cat "$USER_CONFIG")
  assert_contains "$content" "budget: 12000" "should have docs budget 12000"
  assert_contains "$content" "services_reference.md" "should reference services_reference.md"
  assert_contains "$content" "iac_patterns.md" "should reference iac_patterns.md"
  assert_contains "$content" "troubleshooting.md" "should reference troubleshooting.md"
}

test_awscli_agent_has_system_prompt() {
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  local content
  content=$(cat "$USER_CONFIG")
  assert_contains "$content" "systemPrompt" "should have systemPrompt"
  assert_contains "$content" "AWS Cloud assistant" "prompt should mention AWS Cloud assistant"
}

# -- Docs files validation -------------------------------------------------

test_awscli_docs_directory_exists() {
  if [ ! -d "$USER_DOCS_DIR" ]; then
    skip_test "AWS docs directory not found at $USER_DOCS_DIR"
  fi
  assert_eq 0 0 "AWS docs directory exists"
}

test_awscli_docs_services_reference_exists() {
  local doc_file="$USER_DOCS_DIR/services_reference.md"
  if [ ! -f "$doc_file" ]; then
    skip_test "services_reference.md not found"
  fi
  local content
  content=$(cat "$doc_file")
  assert_contains "$content" "EC2" "should contain EC2 section"
  assert_contains "$content" "S3" "should contain S3 section"
  assert_contains "$content" "Lambda" "should contain Lambda section"
}

test_awscli_docs_iac_patterns_exists() {
  local doc_file="$USER_DOCS_DIR/iac_patterns.md"
  if [ ! -f "$doc_file" ]; then
    skip_test "iac_patterns.md not found"
  fi
  local content
  content=$(cat "$doc_file")
  assert_contains "$content" "CDK" "should contain CDK section"
  assert_contains "$content" "SAM" "should contain SAM section"
  assert_contains "$content" "Terraform" "should contain Terraform section"
}

test_awscli_docs_troubleshooting_exists() {
  local doc_file="$USER_DOCS_DIR/troubleshooting.md"
  if [ ! -f "$doc_file" ]; then
    skip_test "troubleshooting.md not found"
  fi
  local content
  content=$(cat "$doc_file")
  assert_contains "$content" "Authentication" "should contain Authentication section"
  assert_contains "$content" "Debugging" "should contain Debugging section"
}

# -- Runtime integration ---------------------------------------------------

test_awscli_agent_appears_in_agent_list() {
  if [ ! -f "$SWEBASH_BIN" ]; then
    skip_test "swebash binary not found at $SWEBASH_BIN"
  fi
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  local out
  out=$(printf 'ai agents\nexit\n' | "$SWEBASH_BIN" 2>/dev/null)
  assert_contains "$out" "awscli" "ai agents should list awscli"
  assert_contains "$out" "AWS CLI" "should show awscli description"
}

test_awscli_docs_loaded_at_runtime() {
  if [ ! -f "$SWEBASH_BIN" ]; then
    skip_test "swebash binary not found at $SWEBASH_BIN"
  fi
  if [ ! -f "$USER_CONFIG" ]; then
    skip_test "user agents.yaml not found"
  fi
  # Check stderr for WARN about unresolved docs — awscli should NOT appear
  local err
  err=$(printf 'ai agents\nexit\n' | SWEBASH_AI_DOCS_BASE_DIR="$HOME/.config/swebash" "$SWEBASH_BIN" 2>&1 1>/dev/null)
  assert_not_contains "$err" "agent=awscli" "awscli should not have unresolved docs warnings"
}
