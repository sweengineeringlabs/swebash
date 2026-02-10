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
2. An LLM API key for AI feature testing (Anthropic, OpenAI, or Gemini)
3. `.env` file with credentials (see `.env.example`)
4. (Optional) `SWEBASH_AI_DOCS_BASE_DIR` — base directory for resolving agent `docs` source paths and project-local config (defaults to cwd)

## Running the Shell

```bash
# Without AI features
./sbh run

# With AI features (Anthropic)
set -a && source .env && set +a
export LLM_PROVIDER=anthropic
./sbh run
```

### Scripted (Non-Interactive) Testing

Most manual tests can be scripted by piping commands into the shell binary. This is useful for CI or quick regression checks without an interactive terminal.

```bash
# Build first
./sbh build

# Pipe commands — shell basics (no AI key needed)
printf 'echo hello world\npwd\nls\nexit\n' | /tmp/swebash-target/release/swebash 2>/dev/null

# Pipe commands — AI features (requires .env)
set -a && source .env && set +a
export LLM_PROVIDER=anthropic SWEBASH_AI_ENABLED=true
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
| [AI Tests](manual_ai_tests.md) | AI commands, agents, config, memory, docs context | 97 |
| [RAG Tests](manual_rag_tests.md) | RAG indexing, retrieval, staleness, vector stores, SweVecDB | 53 |
| [sbh Launcher Tests](manual_sbh_tests.md) | sbh/sbh.ps1 help, build, test, gen-aws-docs | 42 |

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
| Host integration | `features/shell/host/tests/integration.rs` | 60 |
| Host readline tests | `features/shell/host/tests/readline_tests.rs` | 19 |
| AI unit tests | `features/ai/src/` | 182 |
| AI integration | `features/ai/tests/integration.rs` | 181 |
| **Total** | | **516** |

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
