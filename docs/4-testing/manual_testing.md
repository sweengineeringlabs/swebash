# Manual Testing Guide

## Prerequisites

1. Rust toolchain with `wasm32-unknown-unknown` target installed
2. An LLM API key for AI feature testing (Anthropic, OpenAI, or Gemini)
3. `.env` file with credentials (see `.env.example`)

## Running the Shell

```bash
# Without AI features
cargo run

# With AI features (Anthropic)
set -a && source .env && set +a
export LLM_PROVIDER=anthropic
cargo run
```

## Test Checklist

### 1. Shell Basics

| Test | Command | Expected |
|------|---------|----------|
| Startup banner | `cargo run` | Prints `wasm-shell v0.1.0` and prompt |
| Echo | `echo hello world` | Prints `hello world` |
| PWD | `pwd` | Prints current working directory |
| LS | `ls` | Lists files in current directory |
| LS path | `ls /tmp` | Lists files in /tmp |
| LS long | `ls -l` | Long-format listing |
| Exit | `exit` | Shell exits cleanly |

### 2. Directory Navigation

| Test | Command | Expected |
|------|---------|----------|
| CD absolute | `cd /tmp` | Prompt updates to /tmp |
| CD relative | `cd ..` | Moves up one directory |
| CD nonexistent | `cd /no/such/dir` | Prints error, stays in current dir |
| PWD after CD | `cd /tmp && pwd` | Prints `/tmp` |

### 3. File Operations

| Test | Command | Expected |
|------|---------|----------|
| Touch | `touch /tmp/test_manual.txt` | Creates empty file |
| Cat | `cat /tmp/test_manual.txt` | Shows file contents (empty) |
| Cat missing | `cat /tmp/no_such_file` | Prints error |
| Head | `head -5 <file>` | Shows first 5 lines |
| Tail | `tail -5 <file>` | Shows last 5 lines |
| Mkdir | `mkdir /tmp/test_dir` | Creates directory |
| Mkdir recursive | `mkdir -p /tmp/a/b/c` | Creates nested directories |
| CP | `cp /tmp/test_manual.txt /tmp/copy.txt` | Copies file |
| MV | `mv /tmp/copy.txt /tmp/moved.txt` | Renames file |
| RM | `rm /tmp/moved.txt` | Deletes file |
| RM recursive | `rm -r /tmp/test_dir` | Deletes directory tree |
| RM force | `rm -f /tmp/no_such_file` | No error for missing file |

### 4. Environment Variables

| Test | Command | Expected |
|------|---------|----------|
| Export | `export FOO=bar` | Sets variable |
| Env | `env` | Lists all env vars (FOO=bar visible) |

### 5. External Commands

| Test | Command | Expected |
|------|---------|----------|
| External echo | `/bin/echo test` | Runs host system echo |
| Unknown command | `notarealcommand` | Prints "not recognized" error |

### 6. History

| Test | Steps | Expected |
|------|-------|----------|
| History file | Run a few commands, then exit | `~/.swebash_history` file exists |
| History persistence | Restart shell | Previous commands available via arrow keys |

---

## AI Feature Tests

> Requires `ANTHROPIC_API_KEY` (or equivalent) and `LLM_PROVIDER` set.

### 7. AI Status

| Test | Command | Expected |
|------|---------|----------|
| Status | `ai status` | Shows provider, model, enabled=yes, ready=yes |

### 8. AI Ask (NL to Command)

| Test | Command | Expected |
|------|---------|----------|
| Ask via subcommand | `ai ask list all files` | Suggests a command (e.g. `ls -la`), prompts Execute? |
| Ask via shorthand | `? find rust files` | Same behavior as `ai ask` |
| Cancel execution | Press `n` at Execute? prompt | Prints "Cancelled", returns to shell |

### 9. AI Explain

| Test | Command | Expected |
|------|---------|----------|
| Explain via subcommand | `ai explain ls -la` | Natural language explanation of the command |
| Explain via shorthand | `?? ps aux \| grep rust` | Explains the pipeline |
| Explain simple | `ai explain echo test` | Short explanation, no leading/trailing whitespace |

### 10. AI Chat Mode

| Test | Steps | Expected |
|------|-------|----------|
| Enter mode | Type `ai` | Prints "Entered AI mode", prompt changes to `[AI:shell] >` |
| Basic chat | Type a question | Shows "thinking...", then a response |
| Multi-turn memory | Say "My name is Alice", then "What is my name?" | Second reply mentions Alice |
| Exit mode | Type `quit` or `exit` | Prints "Exited AI mode", prompt returns to normal |

### 11. AI Chat from Shell

| Test | Command | Expected |
|------|---------|----------|
| Direct chat | `ai chat what is Rust?` | Prints AI response inline without entering AI mode |

### 12. AI Suggest

| Test | Command | Expected |
|------|---------|----------|
| Suggest | `ai suggest` | Shows autocomplete suggestions based on recent commands |

### 13. Agent Listing

| Test | Command | Expected |
|------|---------|----------|
| List from shell | `ai agents` | Prints table of 4 agents (shell\*, review, devops, git) with descriptions. Active agent marked with `*`. |
| List from AI mode | `ai` then `agents` | Same table, shown inside AI mode |

### 14. Agent Switching (AI Mode)

| Test | Steps | Expected |
|------|-------|----------|
| Switch to review | `ai` then `@review` | Prints "Switched to Code Reviewer (review)", prompt changes to `[AI:review] >` |
| Switch to git | `@git` | Prompt changes to `[AI:git] >` |
| Switch to devops | `@devops` | Prompt changes to `[AI:devops] >` |
| Switch back to shell | `@shell` | Prompt changes to `[AI:shell] >` |
| Multiple switches | `@git` → `@review` → `@shell` → `exit` | Each switch updates prompt, exit works cleanly |
| Active marker follows | `@review` then `agents` | Agent list shows `*review` as active |

### 14b. Agent Switching (from Shell Mode)

Typing `@agent` directly from the shell prompt (without entering AI mode first with `ai`) should switch to the agent **and** enter AI mode. Natural language input must be routed to the AI, not executed as a shell command.

| Test | Steps | Expected |
|------|-------|----------|
| @devops enters AI mode | Type `@devops` from shell prompt | Prints "Switched to DevOps Assistant (devops)" and "Entered AI mode", prompt changes to `[AI:devops] >` |
| @git enters AI mode | Type `@git` from shell prompt | Prints "Switched to Git Assistant (git)" and "Entered AI mode", prompt changes to `[AI:git] >` |
| @review enters AI mode | Type `@review` from shell prompt | Prints "Switched to Code Reviewer (review)" and "Entered AI mode", prompt changes to `[AI:review] >` |
| NL not executed as command | `@devops` then `do we have docker installed?` | AI responds (or shows "not configured"); does **not** print "No such file or directory" |
| Exit returns to shell | `@devops` then `exit` then `echo hello` | Exits AI mode, `echo hello` prints `hello` normally |
| `ai @agent` also works | Type `ai @devops` from shell prompt | Same behavior as `@devops` — enters AI mode with devops agent |

### 15. One-Shot Agent Chat (Shell Mode)

| Test | Command | Expected |
|------|---------|----------|
| One-shot devops | `ai @devops how do I check running containers` | Prints `[devops] DevOps Assistant`, shows response with Docker commands, returns to shell prompt (not AI mode) |
| One-shot review | `ai @review check main.rs` | Prints `[review] Code Reviewer`, shows code review response, returns to shell prompt |
| Agent restored | `ai @devops hello` then `ai` then `exit` | After one-shot, entering AI mode still shows `[AI:shell]` (previous agent restored) |

### 16. Auto-Detection (AI Mode)

Auto-detection switches the active agent based on keywords in the input.

| Test | Steps | Expected |
|------|-------|----------|
| Docker keyword | `ai` then `docker ps` | Prints "Switched to DevOps Assistant (devops)", prompt changes to `[AI:devops] >`, then shows explanation |
| Docker compose | `docker compose up` (while in AI mode) | Stays on devops (or switches if on another agent), shows explanation |
| K8s keyword | `k8s get pods` | Switches to devops if not already, shows kubectl guidance |
| Git keyword | `git rebase` | Prints "Switched to Git Assistant (git)", prompt changes to `[AI:git] >`, shows explanation |
| No match stays | `how do I list files` | No switch message, stays on current agent |
| Prompt tracks agent | Observe prompt after each auto-switch | Prompt always reflects current agent: `[AI:shell]`, `[AI:devops]`, `[AI:git]`, etc. |

### 17. Agent Memory Isolation

| Test | Steps | Expected |
|------|-------|----------|
| Isolated history | `ai` → chat with shell → `@review` → chat with review → `@shell` | Returning to shell still has shell's conversation context, review has its own |
| Clear history | `clear` then `history` | Shows "(no chat history)" for active agent only |

### 18. User-Configurable Agents (YAML)

Agents are loaded from an embedded YAML file compiled into the binary. Users can add or override agents via a config file.

**Config file lookup order:**
1. `$SWEBASH_AGENTS_CONFIG` (env var, highest priority)
2. `~/.config/swebash/agents.yaml`
3. `~/.swebash/agents.yaml`

| Test | Steps | Expected |
|------|-------|----------|
| Add custom agent | Create `~/.config/swebash/agents.yaml` with a new agent (e.g. `id: security`), restart shell | `ai agents` lists 5 agents (4 defaults + custom) |
| Switch to custom agent | `@security` | Prints "Switched to Security Scanner (security)", prompt shows `[AI:security] >` |
| Custom trigger keywords | Add `triggerKeywords: [scan, cve]` to custom agent, restart, enter AI mode | Typing `scan this file` auto-detects the custom agent |
| Override built-in agent | Add `id: shell` entry with custom `name` and `description` to user YAML, restart | `ai agents` shows custom name/description for shell agent; still 4 agents total |
| Invalid user YAML | Write broken YAML to the config file, restart | Shell starts normally with 4 default agents (invalid file silently ignored) |
| Env var override | `export SWEBASH_AGENTS_CONFIG=/path/to/agents.yaml`, restart shell | Agents from the specified file are loaded |
| No config file | Ensure no user YAML exists anywhere, restart | Shell starts normally with 4 default agents |

<details>
<summary>Example user agents.yaml</summary>

```yaml
version: 1
agents:
  - id: security
    name: Security Scanner
    description: Scans code for vulnerabilities
    systemPrompt: |
      You are a security scanner...
    triggerKeywords: [security, scan, cve]
    tools:
      fs: true
      exec: true
      web: false
```

</details>

### 19. DevOps Agent (Docker-specific)

> Requires Docker installed (`docker --version`). Permission errors are expected if user is not in the `docker` group.

| Test | Command | Expected |
|------|---------|----------|
| Container listing | `ai @devops list running docker containers` | DevOps agent responds with `docker ps` guidance and permission troubleshooting if needed |
| Docker images | `ai @devops what docker images do I have` | Explains `docker images` command, offers permission fix |
| Dockerfile help | `ai @devops how do I write a Dockerfile for nginx` | Provides Dockerfile example with explanation |
| Compose guidance | `ai` then `docker compose up` | Auto-switches to devops, explains compose command |
| K8s guidance | `ai` then `k8s get pods` | Devops agent explains kubectl, offers install instructions |

---

## Automated Test Suites

For reference, the automated tests cover these areas:

```bash
# Unit + integration tests (no API key needed)
cargo test

# Full integration tests against real API
set -a && source .env && set +a
cargo test -p swebash-ai -p swebash
```

| Suite | Location | Count |
|-------|----------|-------|
| Host unit tests | `host/src/` | 100 |
| Host integration | `host/tests/integration.rs` | 54 |
| Readline integration | `host/tests/readline_tests.rs` | 19 |
| AI unit tests | `ai/src/` | 19 |
| AI integration | `ai/tests/integration.rs` | 98 |
| **Total** | | **290** |

### Agent-Specific Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `agent_list_returns_all_builtins` | AI integration | 4 agents registered |
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
| `ai_agents_list_command` | Host integration | `ai agents` lists all 4 agents |
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
| `yaml_parse_embedded_defaults` | AI integration | Embedded YAML parses with 4 agents |
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
| `yaml_registry_loads_all_default_agents` | AI integration | 4 agents via create_default_registry |
| `yaml_registry_{shell,review,devops,git}_agent_properties` | AI integration | Properties match originals |
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
