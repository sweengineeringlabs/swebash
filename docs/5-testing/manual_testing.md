# Manual Testing Guide

> **TLDR:** Hub for all manual testing procedures — links to focused test documents by domain.

**Audience**: Developers, QA

**WHAT**: Central navigation hub for manual test procedures
**WHY**: Provides a single entry point so testers can find the right checklist for their task
**HOW**: Organized by domain with shared prerequisites and setup instructions

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Running the Shell](#running-the-shell)
- [Test Documents](#test-documents)
- [Automated Test Suites](#automated-test-suites)

---

## Prerequisites

1. Rust toolchain with `wasm32-unknown-unknown` target installed
2. Credentials for AI feature testing:
   - **Anthropic**: Claude Code OAuth credentials (`~/.claude/.credentials.json`) are used automatically when present, or set `ANTHROPIC_API_KEY` as a fallback. Either is sufficient.
   - **OpenAI**: Set `OPENAI_API_KEY`
   - **Gemini**: Set `GEMINI_API_KEY`
3. `.env` file with credentials (see `.env.example`). Command-line env vars pre-set before `sbh` is invoked take priority over `.env` values.
4. (Optional) `SWEBASH_AI_DOCS_BASE_DIR` — base directory for resolving agent `docs` source paths and project-local config (defaults to cwd)

> **`SWEBASH_AI_ENABLED`**: AI features are **enabled by default**. Set `SWEBASH_AI_ENABLED=false` (or `0`) to disable. The env var only needs to be set explicitly to turn AI off.

## Running the Shell

```bash
# Without AI features
./sbh run

# With AI features — Anthropic via OAuth (auto-detected from ~/.claude/.credentials.json)
export LLM_PROVIDER=anthropic
./sbh run

# With AI features — Anthropic via API key
set -a && source .env && set +a   # .env must contain ANTHROPIC_API_KEY and LLM_PROVIDER
./sbh run
```

### Scripted (Non-Interactive) Testing

Most manual tests can be scripted by piping commands into the shell binary. This is useful for CI or quick regression checks without an interactive terminal.

```bash
# Build first
./sbh build

# Pipe commands — shell basics (no AI key needed)
printf 'echo hello world\npwd\nls\nexit\n' | /tmp/swebash-target/release/swebash 2>/dev/null

# Pipe commands — AI features (Anthropic via API key from .env)
set -a && source .env && set +a   # exports LLM_PROVIDER and ANTHROPIC_API_KEY
{
  echo 'ai status'
  echo 'exit'
} | /tmp/swebash-target/release/swebash 2>/dev/null

# Pipe commands — AI features (Anthropic via OAuth, no .env needed)
export LLM_PROVIDER=anthropic
{
  echo 'ai status'
  echo 'exit'
} | /tmp/swebash-target/release/swebash 2>/dev/null

# AI commands that call the LLM need sleep to wait for the response
{
  echo 'ai ask list all files'
  sleep 10
  echo 'n'
  echo 'exit'
} | /tmp/swebash-target/release/swebash 2>/dev/null
```

**Notes:**
- Output contains ANSI escape codes. Use `cat -v` or `sed 's/\x1b\[[0-9;]*[a-zA-Z]//g'` to inspect.
- Redirect stderr (`2>/dev/null`) to suppress tracing/warning logs.
- LLM-dependent commands (`ai ask`, `ai explain`, `ai chat`, `ai suggest`) need `sleep` between the command and subsequent input to allow the API response to arrive.
- Tests that require Docker (section 20) or PowerShell (`sbh.ps1` sections 25-28) cannot be scripted this way on environments without those tools.

---

## Test Documents

| Document | Domain | Tests |
|----------|--------|-------|
| [Shell Tests](manual_shell_tests.md) | Shell basics, file ops, history, workspace sandbox | 32 |
| [Tab Tests](manual_tab_tests.md) | Tab commands, CWD isolation, tab bar, shortcuts, mode tabs | 68 |
| [AI Tests](manual_ai_tests.md) | AI commands, agents, config, memory, docs context | 98 |
| [RAG Tests](manual_rag_tests.md) | RAG indexing, retrieval, staleness, vector stores, SweVecDB | 53 |
| [Git Safety Gate Tests](manual_git_gates_tests.md) | Setup wizard, branch pipeline, safety gates, workspace binding | 91 |
| [sbh Launcher Tests](manual_sbh_tests.md) | sbh/sbh.ps1 help, build, test, gen-aws-docs | 46 |

---

## Automated Test Suites

For reference, the automated tests cover these areas:

```bash
# Unit + integration tests (no API key needed)
cargo test --workspace

# Full integration tests against real API
set -a && source .env && set +a
cargo test -p swebash-ai -p swebash
```

| Suite | Location | Count |
|-------|----------|-------|
| Engine unit tests | `features/shell/engine/src/` | 20 |
| Readline unit tests | `features/shell/readline/src/` | 54 |
| Host unit tests | `features/shell/host/src/spi/{git_config,git_gates,config}.rs` | 112 |
| Host integration | `features/shell/host/tests/integration.rs` | 107 |
| Host readline tests | `features/shell/host/tests/readline_tests.rs` | 19 |
| AI unit tests | `features/ai/src/` | 164 |
| AI integration | `features/ai/tests/integration.rs` | 201 |
| **Total** | | **677** |

### Agent-Specific Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `agent_list_returns_all_builtins` | AI integration | 10 agents registered |
| `agent_default_is_shell` | AI integration | Default agent is shell |
| `agent_switch_and_current_round_trip` | AI integration | Switch between agents and verify |
| `agent_switch_unknown_returns_error` | AI integration | Error on unknown agent |
| `agent_list_marks_active` | AI integration | Exactly one active flag, follows switches |
| `agent_auto_detect_git_keyword` | AI integration | "git commit" triggers git agent |
| `agent_auto_detect_docker_keyword` | AI integration | "docker ps" triggers devops agent |
| `agent_auto_detect_no_match_stays` | AI integration | Stays on shell when no keywords match |
| `agent_auto_detect_disabled` | AI integration | Config flag disables detection |
| `agent_active_agent_id` | AI integration | Direct ID accessor |
| `agent_engine_caching` | AI integration | Engine survives round-trip switches |
| `agent_default_config_override` | AI integration | Custom default agent from config |
| `ai_agents_list_command` | Host integration | `ai agents` lists all 10 agents |
| `ai_agent_switch_in_ai_mode` | Host integration | `@review` switches agent |
| `ai_agent_list_in_ai_mode` | Host integration | `agents` command inside AI mode |
| `ai_mode_prompt_shows_default_agent` | Host integration | Prompt shows `[AI:shell]` |
| `ai_agent_one_shot_from_shell` | Host integration | `ai @review hello` one-shot |
| `ai_agent_switch_back_and_forth` | Host integration | Multiple switches without crash |
| `switch_agent_enters_ai_mode` | Host unit | `SwitchAgent` returns true (enters AI mode) |
| `switch_agent_enters_ai_mode_all_agents` | Host unit | All agent names enter AI mode |
| `enter_mode_enters_ai_mode` | Host unit | `EnterMode` returns true |
| `exit_mode_does_not_enter_ai_mode` | Host unit | `ExitMode` returns false |
| `agent_chat_does_not_enter_ai_mode` | Host unit | One-shot chat stays in shell mode |
| `status_does_not_enter_ai_mode` | Host unit | Status returns false |
| `list_agents_does_not_enter_ai_mode` | Host unit | ListAgents returns false |
| `history_does_not_enter_ai_mode` | Host unit | History returns false |
| `clear_does_not_enter_ai_mode` | Host unit | Clear returns false |
| `chat_does_not_enter_ai_mode` | Host unit | Chat returns false |
| `ai_agent_switch_from_shell_enters_ai_mode` | Host integration | `@devops` from shell enters AI mode |
| `ai_agent_switch_from_shell_no_shell_execution` | Host integration | NL after `@devops` not executed as command |
| `ai_agent_switch_from_shell_exit_returns_to_shell` | Host integration | Exit after `@devops` returns to working shell |
| `ai_agent_switch_from_shell_all_agents` | Host integration | All `@agent` shorthands enter AI mode |
| `ai_agent_switch_from_shell_with_ai_prefix` | Host integration | `ai @devops` enters AI mode |
| `yaml_parse_embedded_defaults` | AI integration | Embedded YAML parses with 10 agents |
| `yaml_parse_defaults_section` | AI integration | Defaults (temperature, maxTokens, tools) correct |
| `yaml_parse_agent_ids_match_originals` | AI integration | shell/review/devops/git IDs present |
| `yaml_parse_trigger_keywords_preserved` | AI integration | All keywords match original structs |
| `yaml_parse_tool_overrides` | AI integration | Tool configs per agent correct |
| `yaml_parse_system_prompts_non_empty` | AI integration | Every agent has a system prompt |
| `yaml_parse_rejects_malformed_input` | AI integration | Broken YAML returns error |
| `config_agent_inherits_defaults` | AI integration | ConfigAgent uses default temp/tokens/tools |
| `config_agent_overrides_temperature_and_tokens` | AI integration | Explicit values override defaults |
| `config_agent_tool_filter_only` | AI integration | Partial tools → ToolFilter::Only |
| `config_agent_tool_filter_none` | AI integration | All false → ToolFilter::None |
| `config_agent_tool_filter_all` | AI integration | All true → ToolFilter::All |
| `config_agent_trigger_keywords` | AI integration | Keywords passed through correctly |
| `config_agent_system_prompt_preserved` | AI integration | Multiline prompts preserved |
| `config_agent_inherits_custom_defaults` | AI integration | Non-default defaults section works |
| `yaml_registry_loads_all_default_agents` | AI integration | 10 agents via create_default_registry |
| `yaml_registry_{shell,review,devops,git}_agent_properties` | AI integration | Properties match originals |
| `yaml_registry_clitester_agent_properties` | AI integration | clitester tools, keywords, maxIterations |
| `yaml_registry_apitester_agent_properties` | AI integration | apitester tools, keywords, maxIterations |
| `yaml_registry_detect_agent_from_keywords` | AI integration | Keyword detection from YAML |
| `yaml_registry_suggest_agent_from_keywords` | AI integration | Suggestion from YAML keywords |
| `yaml_registry_system_prompts_contain_key_content` | AI integration | Prompts contain expected terms |
| `yaml_registry_agents_sorted_by_id` | AI integration | list() returns sorted order |
| `yaml_user_config_env_var_loads_custom_agent` | AI integration | SWEBASH_AGENTS_CONFIG adds agent |
| `yaml_user_config_overrides_builtin_agent` | AI integration | User YAML replaces agent by ID |
| `yaml_user_config_invalid_file_ignored` | AI integration | Bad YAML falls back to defaults |
| `yaml_user_config_nonexistent_path_ignored` | AI integration | Missing file falls back to defaults |
| `yaml_user_config_adds_multiple_agents` | AI integration | Multiple user agents + custom defaults |
| `yaml_user_config_detect_agent_includes_user_keywords` | AI integration | User keywords in detect/suggest |
| `yaml_service_list_agents_returns_correct_info` | AI integration | Display names from YAML in API |
| `yaml_service_switch_to_yaml_loaded_agent` | AI integration | Switch through all YAML agents |
| `yaml_service_auto_detect_uses_yaml_keywords` | AI integration | YAML keywords drive auto-detection |
| `yaml_service_with_user_override_reflects_in_api` | AI integration | User override visible through service |
| `test_directives_prepended_to_system_prompt` | AI unit | Default directives appear in `<directives>` block before system prompt |
| `test_empty_directives_no_block` | AI unit | No `<directives>` block when defaults have no directives |
| `test_agent_directives_override_defaults` | AI unit | Agent-level directives replace default directives |
| `test_agent_empty_directives_suppresses_defaults` | AI unit | Agent with `directives: []` has no directives block |
| `test_directives_ordering_with_docs_and_think_first` | AI unit | Order: `<directives>` → `<documentation>` → prompt → thinkFirst suffix |

### Workspace Sandbox Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `workspace_status_shows_info` | Host integration | `workspace` command displays sandbox status |
| `workspace_write_allowed_in_workspace` | Host integration | `touch` succeeds inside workspace root |
| `workspace_write_denied_outside_workspace` | Host integration | `touch` denied outside workspace |
| `workspace_cd_denied_outside_workspace` | Host integration | `cd` denied outside workspace |
| `workspace_rw_mode_allows_write` | Host integration | `workspace rw` enables writes |
| `workspace_ro_mode_denies_write` | Host integration | `workspace ro` blocks writes |
| `workspace_allow_adds_path` | Host integration | `workspace allow PATH` adds writable path |
| `workspace_allow_ro_denies_write` | Host integration | `workspace allow PATH ro` denies write to that path |
| `workspace_disable_allows_everything` | Host integration | `workspace disable` bypasses sandbox |
| `workspace_enable_after_disable_restricts` | Host integration | `workspace enable` re-activates sandbox |
| `workspace_ls_allowed_in_workspace` | Host integration | `ls` succeeds in workspace |
| `workspace_cat_allowed_in_workspace` | Host integration | `cat` succeeds in workspace |
| `workspace_cat_denied_outside_workspace` | Host integration | `cat` denied outside workspace |
| `workspace_mkdir_allowed_in_workspace` | Host integration | `mkdir` succeeds in workspace |
| `workspace_mkdir_denied_outside_workspace` | Host integration | `mkdir` denied outside workspace |
| `workspace_rm_allowed_in_workspace` | Host integration | `rm` succeeds in workspace |
| `workspace_rm_denied_outside_workspace` | Host integration | `rm` denied outside workspace |
| `workspace_cp_within_workspace_allowed` | Host integration | `cp` within workspace succeeds |
| `workspace_cp_to_outside_denied` | Host integration | `cp` to outside workspace denied |
| `workspace_mv_to_outside_denied` | Host integration | `mv` to outside workspace denied |
| `workspace_multiple_allowed_paths` | Host integration | Multiple `workspace allow` paths work together |
| `workspace_nested_path_in_workspace_allowed` | Host integration | Nested paths inside workspace allowed |

### Path Normalization Automated Tests

Tests verifying that paths use forward slashes for cross-platform copy-paste compatibility.

| Test | Suite | Verifies |
|------|-------|----------|
| `unix_path_unchanged` | Host unit | Unix paths remain unchanged |
| `windows_path_normalized` | Host unit | Windows backslashes converted to forward slashes |
| `unc_path_normalized` | Host unit | UNC prefix `\\?\` normalized to `//?/` |
| `mixed_separators` | Host unit | Mixed separators normalized to forward slashes |
| `path_pwd_uses_forward_slashes` | Host integration | `pwd` output contains only forward slashes |
| `path_prompt_uses_forward_slashes` | Host integration | Shell prompt uses forward slashes |
| `path_workspace_status_uses_forward_slashes` | Host integration | `workspace` status uses forward slashes |
| `path_workspace_allow_uses_forward_slashes` | Host integration | `workspace allow` output uses forward slashes |
| `path_sandbox_error_uses_forward_slashes` | Host integration | Sandbox error messages use forward slashes |
| `path_copy_paste_roundtrip` | Host integration | Path from `pwd` can be used directly in `cd` |

### Git Safety Gate Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `default_pipeline_has_six_branches` | Host unit | Pipeline generates exactly 6 branches |
| `default_pipeline_branch_names` | Host unit | Branch names: main, dev_{user}, test, integration, uat, staging-prod |
| `default_pipeline_dev_branch_uses_user_id` | Host unit | Dev branch name uses user ID |
| `default_pipeline_roles` | Host unit | Correct BranchRole for each pipeline position |
| `default_pipeline_protection_flags` | Host unit | main=protected, dev=open, rest=protected |
| `default_gates_protected_branch_blocks_commit_push_merge` | Host unit | Protected → BlockWithOverride for commit/push/merge, Deny for force-push |
| `default_gates_open_branch_allows_everything` | Host unit | Open → Allow for all operations |
| `default_gates_count_matches_pipeline` | Host unit | One gate rule per pipeline branch |
| `gate_action_serde_roundtrip` | Host unit | GateAction serializes/deserializes correctly |
| `git_config_serde_roundtrip` | Host unit | Full GitConfig survives TOML roundtrip |
| `gate_action_display` | Host unit | Display trait outputs snake_case strings |
| `branch_role_display` | Host unit | Display trait for all 7 roles |
| `empty_pipeline_produces_no_gates` | Host unit | Empty pipeline → empty gates |
| `gate_action_deserialized_from_snake_case` | Host unit | TOML snake_case → GateAction enum |
| `empty_args_allowed` | Host unit | No git args → Allowed |
| `unknown_subcommand_allowed` | Host unit | `git status` → Allowed (passthrough) |
| `unknown_branch_allowed` | Host unit | Branch not in gates → Allowed |
| `commit_on_protected_branch_blocked_with_override` | Host unit | `git commit` on main → BlockedWithOverride |
| `commit_on_open_branch_allowed` | Host unit | `git commit` on dev → Allowed |
| `push_on_protected_branch_blocked_with_override` | Host unit | `git push` on main → BlockedWithOverride |
| `push_on_open_branch_allowed` | Host unit | `git push` on dev → Allowed |
| `force_push_on_protected_branch_denied` | Host unit | `git push --force` on main → Denied |
| `force_push_short_flag_on_protected_branch_denied` | Host unit | `git push -f` on main → Denied |
| `force_with_lease_on_protected_branch_denied` | Host unit | `git push --force-with-lease` → Denied |
| `force_push_on_open_branch_allowed` | Host unit | `git push --force` on dev → Allowed |
| `merge_on_protected_branch_blocked_with_override` | Host unit | `git merge` on main → BlockedWithOverride |
| `rebase_on_protected_branch_blocked_with_override` | Host unit | `git rebase` on main → BlockedWithOverride |
| `merge_on_open_branch_allowed` | Host unit | `git merge` on dev → Allowed |
| `all_protected_branches_block_commit` | Host unit | All 5 protected branches block commit |
| `all_protected_branches_deny_force_push` | Host unit | All 5 protected branches deny force-push |
| `git_log_always_allowed` | Host unit | `git log` → Allowed on any branch |
| `git_diff_always_allowed` | Host unit | `git diff` → Allowed on any branch |
| `git_branch_always_allowed` | Host unit | `git branch` → Allowed on any branch |
| `enforcer_with_no_gates_allows_everything` | Host unit | Empty enforcer → all operations Allowed |
| `enforcer_gate_lookup` | Host unit | gate_for() finds correct branch or None |
| `blocked_message_contains_branch_name` | Host unit | Override message includes branch name and operation |
| `denied_message_contains_branch_name` | Host unit | Deny message includes branch name, operation, "denied" |
| `custom_deny_commit_on_branch` | Host unit | Custom Deny gate → Denied result |
| `custom_allow_force_push` | Host unit | Custom Allow gate for force-push → Allowed |
| `default_config_setup_completed_false` | Host unit | Default config: setup_completed=false, git=None |
| `default_config_workspace_defaults` | Host unit | Default workspace: root=~/.config/swebash/workspace, mode=ro, enabled=true (XDG-compliant) |
| `serde_roundtrip_default_config` | Host unit | Default SwebashConfig survives TOML roundtrip |
| `serde_roundtrip_with_git_config` | Host unit | Config with git section survives roundtrip |
| `deserialize_legacy_config_without_git_fields` | Host unit | Pre-git-gates config.toml loads with defaults |
| `deserialize_with_setup_completed_true` | Host unit | setup_completed parsed from TOML |
| `to_policy_preserves_enabled_flag` | Host unit | Disabled sandbox → policy.enabled=false |
| `to_policy_sets_rw_mode` | Host unit | mode="rw" → ReadWrite in policy |
| `parse_mode_variants` | Host unit | All mode string variants parsed correctly |
| `save_config_serializes_git_section` | Host unit | Serialized output contains [git], gates, setup_completed |
| `setup_command_recognized` | Host integration | `setup` not treated as external command |
| `git_gate_blocks_commit_on_protected_branch` | Host integration | .swebash/git.toml deny → commit blocked |
| `git_gate_allows_commit_on_open_branch` | Host integration | Allow gate → commit succeeds |
| `git_gate_denies_force_push_on_protected_branch` | Host integration | Force-push on main → denied |
| `git_gate_no_config_allows_everything` | Host integration | No .swebash/git.toml → all operations pass |
| `git_gate_passthrough_commands_always_allowed` | Host integration | git status/log always allowed even with deny gates |

### Workspace-Repo Binding Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `bound_workspace_matches_workspace_exact` | Host unit | Exact path match for workspace binding |
| `bound_workspace_matches_workspace_subdirectory` | Host unit | Subdirectory matches parent workspace binding |
| `bound_workspace_no_match_different_path` | Host unit | Different paths do not match |
| `bound_workspace_matches_remote_same_url` | Host unit | Same remote URL matches |
| `bound_workspace_matches_remote_ssh_vs_https` | Host unit | SSH and HTTPS URLs for same repo match |
| `bound_workspace_no_match_different_remote` | Host unit | Different repos do not match |
| `config_find_workspace_for_path` | Host unit | find_workspace_for_path() returns correct binding |
| `config_multiple_workspaces` | Host unit | Multiple workspaces can be bound |
| `config_verify_repo_binding_success` | Host unit | verify_repo_binding() succeeds when remote matches |
| `config_verify_repo_binding_mismatch` | Host unit | verify_repo_binding() fails when remote differs |
| `bound_workspace_serde_roundtrip` | Host unit | BoundWorkspace survives TOML serialization |
| `normalize_path_preserves_forward_slashes` | Host unit | Forward slashes unchanged |
| `normalize_path_converts_backslashes` | Host unit | Backslashes converted to forward slashes |
| `normalize_remote_https_url` | Host unit | HTTPS URL normalized correctly |
| `normalize_remote_ssh_url` | Host unit | SSH URL normalized correctly |
| `normalize_remote_removes_trailing_git` | Host unit | .git suffix removed |
| `normalize_remote_case_insensitive` | Host unit | Case-insensitive normalization |
| `workspace_binding_config_loads_bindings` | Host integration | Config file with bound_workspaces loads correctly |
| `workspace_binding_multiple_workspaces_allowed` | Host integration | Multiple workspace bindings in config |
| `workspace_binding_path_normalization` | Host integration | Path normalization for cross-platform use |
| `workspace_binding_remote_normalization` | Host integration | Remote URL normalization (SSH vs HTTPS) |
| `workspace_binding_config_with_mismatch_detectable` | Host integration | Config with mismatched remote is detectable |
| `workspace_binding_config_with_matching_remote` | Host integration | Config with matching remote works correctly |

### Empty Folder Message Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `ls_empty_directory_shows_message` | Host integration | `ls` in empty directory shows "(empty)" |
| `ls_long_empty_directory_shows_message` | Host integration | `ls -l` in empty directory shows header + "(empty)" |
| `ls_non_empty_directory_no_empty_message` | Host integration | `ls` with files does not show "(empty)" |

### AI Mode Multiline Input Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `ai_mode_incomplete_quote_does_not_hang` | Host integration | Natural language with apostrophe (what's) doesn't hang |
| `ai_mode_natural_language_with_quotes_works` | Host integration | Contractions (what's, it's, don't) work in AI mode |
| `ai_mode_question_with_apostrophe` | Host integration | Incomplete-looking quotes (how's) process correctly |

### AI Tool Sandbox Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `sandbox_allows_path_inside_workspace` | AI unit | Paths inside workspace root are allowed |
| `sandbox_denies_path_outside_workspace` | AI unit | Paths outside workspace are denied |
| `sandbox_readonly_denies_write` | AI unit | Read-only mode blocks write operations |
| `sandbox_disabled_allows_everything` | AI unit | Disabled sandbox allows all operations |
| `normalize_path_handles_backslashes` | AI unit | Windows backslashes normalized correctly |
| `sandbox_multiple_allowed_paths` | AI unit | Multiple workspace paths with different modes |
| `sandbox_nested_path_allowed` | AI unit | Deeply nested paths inside workspace allowed |
| `sandbox_parent_traversal_blocked` | AI unit | Parent directory access blocked |
| `sandbox_error_message_includes_path` | AI unit | Error messages include the blocked path |
| `sandboxed_tool_check_args_extracts_path` | AI unit | Tool arguments are checked for paths |
| `sandboxed_tool_needs_write_detection` | AI unit | Write operations detected from tool name |
| `ai_sandbox_respects_workspace_policy` | Host integration | AI sandbox inherits workspace restrictions |
