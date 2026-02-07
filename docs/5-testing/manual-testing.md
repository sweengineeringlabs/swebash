# Manual Testing Guide

**Audience**: Developers, QA

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

## Test Checklist

### 1. Shell Basics

| Test | Command | Expected |
|------|---------|----------|
| Startup banner | `./sbh run` | Prints `wasm-shell v0.1.0` and prompt |
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
| List from shell | `ai agents` | Prints table of 8 agents (shell\*, review, devops, git, web, seaaudit, rscagent, docreview) with descriptions. Active agent marked with `*`. |
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

### 17b. History and Clear Commands

| Test | Steps | Expected |
|------|-------|----------|
| History shows messages | `ai` → send a few messages → `history` | Prints numbered list of recent user/assistant messages for the active agent |
| History empty | `ai` → `@review` → `history` | Shows "(no chat history)" since review agent has no messages yet |
| Clear resets agent | `ai` → send a message → `clear` → `history` | Shows "(no chat history)" — only active agent's history is cleared |
| Clear does not affect others | `ai` → chat with shell → `@review` → `clear` → `@shell` → `history` | Shell agent history is still intact after clearing review |

### 18. User-Configurable Agents (YAML)

Agents are loaded from an embedded YAML file compiled into the binary. Users can add or override agents via a config file.

Agents are loaded in three layers (later layers override earlier ones):

**Config file lookup order:**
1. Built-in (`default_agents.yaml` embedded in binary)
2. Project-local (`<SWEBASH_AI_DOCS_BASE_DIR>/.swebash/agents.yaml`, if present)
3. User-level (first match wins):
   - `$SWEBASH_AGENTS_CONFIG` (env var, highest priority)
   - `~/.config/swebash/agents.yaml`
   - `~/.swebash/agents.yaml`

| Test | Steps | Expected |
|------|-------|----------|
| Add custom agent | Create `~/.config/swebash/agents.yaml` with a new agent (e.g. `id: security`), restart shell | `ai agents` lists 9 agents (8 defaults + custom) |
| Switch to custom agent | `@security` | Prints "Switched to Security Scanner (security)", prompt shows `[AI:security] >` |
| Custom trigger keywords | Add `triggerKeywords: [scan, cve]` to custom agent, restart, enter AI mode | Typing `scan this file` auto-detects the custom agent |
| Override built-in agent | Add `id: shell` entry with custom `name` and `description` to user YAML, restart | `ai agents` shows custom name/description for shell agent; still 8 agents total |
| Invalid user YAML | Write broken YAML to the config file, restart | Shell starts normally with 8 default agents (invalid file silently ignored) |
| Env var override | `export SWEBASH_AGENTS_CONFIG=/path/to/agents.yaml`, restart shell | Agents from the specified file are loaded |
| No config file | Ensure no user YAML exists anywhere, restart | Shell starts normally with 8 default agents |
| Project-local config | Create `.swebash/agents.yaml` in project root with a new agent (e.g. `id: projbot`), restart shell | `ai agents` lists 9 agents (8 defaults + project agent) |
| Project-local override | Add `id: shell` entry to `.swebash/agents.yaml` with custom name, restart | Shell agent shows custom name (project layer overrides built-in) |
| User overrides project | Same `id` in both project-local and user-level config, restart | User-level config wins (loaded last) |
| Docs base dir | `export SWEBASH_AI_DOCS_BASE_DIR=/path/to/project`, restart | Project-local config is read from `/path/to/project/.swebash/agents.yaml` |

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
    maxIterations: 15
    tools:
      fs: true
      exec: true
      web: false
    docs:
      budget: 4000
      sources:
        - docs/security-policy.md
        - docs/threat-model.md
```

</details>

### 19. Shared Directives

Directives are quality standards defined in the `defaults.directives` section of `default_agents.yaml`. They are automatically prepended as a `<directives>` block to every agent's system prompt.

| Test | Steps | Expected |
|------|-------|----------|
| Directives present | `ai` then ask agent to describe its system prompt, or inspect via debug logging | System prompt starts with `<directives>` block containing 7 quality directives |
| Directives before docs | Switch to `@rscagent` or `@docreview` (agents with docs) | `<directives>` block appears before `<documentation>` block in system prompt |
| Directives before prompt | Inspect any agent's system prompt | `<directives>` block appears before the agent's own system prompt text |
| All agents inherit | Check system prompts of shell, review, devops, git, web, seaaudit, rscagent, docreview | All agents have the same `<directives>` block from defaults |
| Agent override | Add `directives: ["Custom rule."]` to a user agent in `agents.yaml`, restart | That agent's `<directives>` block contains only `- Custom rule.`, not the defaults |
| Agent suppress | Add `directives: []` to a user agent in `agents.yaml`, restart | That agent has no `<directives>` block at all |

<details>
<summary>Expected directives block</summary>

```
<directives>
- Always produce production-ready, professional, bug-free code.
- Never use workarounds, simplified solutions, or short-term fixes — solve problems at their root.
- Handle errors explicitly — no silent failures, no swallowed exceptions, no bare unwrap() in production paths.
- Validate all external inputs at system boundaries; never trust unvalidated data.
- Follow least-privilege principles; avoid exposing unnecessary public surface area.
- Write self-documenting code with meaningful names; avoid magic numbers and unexplained literals.
- Consider edge cases and failure modes; write code that is testable and verifiable.
</directives>
```

</details>

### 19b. Agent docs_context

Agents can declare a `docs` section in their YAML with a `budget` (max characters) and a list of `sources` (file globs resolved relative to `SWEBASH_AI_DOCS_BASE_DIR`). Matching files are loaded and prepended to the system prompt inside a `<documentation>` block.

| Test | Steps | Expected |
|------|-------|----------|
| rscagent has docs | Switch to `@rscagent`, inspect system prompt via debug logging or ask agent | System prompt contains `<documentation>` block with content from its 20 source files |
| docreview has docs | Switch to `@docreview`, inspect system prompt | System prompt contains `<documentation>` block with content from its 4 source files |
| Docs before prompt | Inspect `@rscagent` system prompt | `<directives>` block appears first, then `<documentation>` block, then agent's own prompt text |
| Missing sources skipped | Set `SWEBASH_AI_DOCS_BASE_DIR` to a directory without doc files, restart | Agents start normally; missing doc sources are silently skipped |
| Budget truncation | Create an agent YAML with `docs: { budget: 100, sources: [large-file.md] }`, restart | Documentation is truncated to approximately 100 characters |

### 19c. Agent maxIterations

Some agents override the global `SWEBASH_AI_TOOLS_MAX_ITER` with a per-agent `maxIterations` value in YAML. This limits how many tool-calling iterations the agent performs per turn.

| Test | Steps | Expected |
|------|-------|----------|
| seaaudit has 25 | Check seaaudit agent config or debug logging | `maxIterations` is 25 (vs global default of 10) |
| rscagent has 20 | Check rscagent agent config | `maxIterations` is 20 |
| docreview has 25 | Check docreview agent config | `maxIterations` is 25 |
| Default agents use global | Check shell, review, devops, git agents | `maxIterations` inherits from global config (default 10) |

### 20. DevOps Agent (Docker-specific)

> Requires Docker installed (`docker --version`). Permission errors are expected if user is not in the `docker` group.

| Test | Command | Expected |
|------|---------|----------|
| Container listing | `ai @devops list running docker containers` | DevOps agent responds with `docker ps` guidance and permission troubleshooting if needed |
| Docker images | `ai @devops what docker images do I have` | Explains `docker images` command, offers permission fix |
| Dockerfile help | `ai @devops how do I write a Dockerfile for nginx` | Provides Dockerfile example with explanation |
| Compose guidance | `ai` then `docker compose up` | Auto-switches to devops, explains compose command |
| K8s guidance | `ai` then `k8s get pods` | Devops agent explains kubectl, offers install instructions |

---

## sbh Launcher

The `sbh` (and `sbh.ps1`) launcher is the primary entry point. These tests verify it delegates correctly.

### 21. sbh help

| Test | Command | Expected |
|------|---------|----------|
| Help flag | `./sbh --help` | Prints usage with all commands: setup, build, run, test |
| Help command | `./sbh help` | Same output as `--help` |
| No args | `./sbh` | Prints usage and exits with code 0 (same as help) |
| Unknown command | `./sbh foo` | Prints usage and exits with code 1 |

### 22. sbh test

| Test | Command | Expected |
|------|---------|----------|
| All suites | `./sbh test` | Runs engine, readline, host, ai tests in order; all pass |
| Engine only | `./sbh test engine` | Runs engine tests only |
| Host only | `./sbh test host` | Runs host tests only |
| Readline only | `./sbh test readline` | Runs readline tests only |
| AI only | `./sbh test ai` | Runs AI tests only |
| Scripts only | `./sbh test scripts` | Runs bash script tests via `bin/tests/runner.sh` (feature + e2e `*.test.sh` files) |
| Help text matches suites | `./sbh --help` | Test suite list includes `engine|host|readline|ai|scripts|all` |

### 23. Cargo registry

The project depends on a local Cargo registry for rustratify crates. The test scripts verify the registry is configured and reachable before running tests.

| Test | Command | Expected |
|------|---------|----------|
| Registry set | `./sbh test engine` | First line prints `==> Registry: file:///...index (ok)` |
| Registry missing | `CARGO_REGISTRIES_LOCAL_INDEX=file:///nonexistent ./sbh test engine` | Prints `ERROR: Local registry index not found`, exits 1 |
| Registry unset | Unset `CARGO_REGISTRIES_LOCAL_INDEX` and remove from `.bashrc`, run `./sbh test engine` | `preflight` sets fallback path; verify it resolves |

### 24. sbh build & run

| Test | Command | Expected |
|------|---------|----------|
| Release build | `./sbh build` | Builds engine WASM (release) and host (release) without errors |
| Debug build | `./sbh build --debug` | Builds engine WASM (release) and host (debug) without errors |
| Run | `./sbh run` | Launches shell, shows banner and prompt |

### 25. sbh.ps1 help (PowerShell)

| Test | Command | Expected |
|------|---------|----------|
| Help flag | `.\sbh.ps1 --help` | Prints usage with all commands: setup, build, run, test |
| Help short flag | `.\sbh.ps1 -h` | Same output as `--help` |
| Help command | `.\sbh.ps1 help` | Same output as `--help` |
| No args | `.\sbh.ps1` | Prints usage, exits with code 0 |
| Unknown command | `.\sbh.ps1 foo` | Prints usage, exits with code 1 |

### 26. sbh.ps1 test (PowerShell)

| Test | Command | Expected |
|------|---------|----------|
| All suites | `.\sbh.ps1 test` | Runs engine, readline, host, ai tests in order; all pass |
| Engine only | `.\sbh.ps1 test engine` | Runs engine tests only |
| Host only | `.\sbh.ps1 test host` | Runs host tests only |
| Readline only | `.\sbh.ps1 test readline` | Runs readline tests only |
| AI only | `.\sbh.ps1 test ai` | Runs AI tests only |
| Scripts only | `.\sbh.ps1 test scripts` | Runs Pester script tests (feature + e2e) |

### 27. sbh.ps1 setup (PowerShell)

| Test | Command | Expected |
|------|---------|----------|
| Setup dispatch | `.\sbh.ps1 setup` | Dispatches to `bin\setup.ps1`; checks prerequisites, registry, .env |
| No parse errors | `.\sbh.ps1 setup` | No `ParserError` or `MissingEndCurlyBrace` errors |

### 28. sbh.ps1 build & run (PowerShell)

| Test | Command | Expected |
|------|---------|----------|
| Release build | `.\sbh.ps1 build` | Builds engine WASM (release) and host (release) without errors |
| Debug build | `.\sbh.ps1 build -Debug` | Builds engine WASM (release) and host (debug) without errors |
| Run | `.\sbh.ps1 run` | Launches shell, shows banner and prompt |

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
| AI unit tests | `features/ai/src/` | 123 |
| AI integration | `features/ai/tests/integration.rs` | 155 |
| **Total** | | **431** |

### Agent-Specific Automated Tests

| Test | Suite | Verifies |
|------|-------|----------|
| `agent_list_returns_all_builtins` | AI integration | 8 agents registered |
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
| `ai_agents_list_command` | Host integration | `ai agents` lists all 8 agents |
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
| `yaml_parse_embedded_defaults` | AI integration | Embedded YAML parses with 8 agents |
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
| `yaml_registry_loads_all_default_agents` | AI integration | 8 agents via create_default_registry |
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
| `test_directives_prepended_to_system_prompt` | AI unit | Default directives appear in `<directives>` block before system prompt |
| `test_empty_directives_no_block` | AI unit | No `<directives>` block when defaults have no directives |
| `test_agent_directives_override_defaults` | AI unit | Agent-level directives replace default directives |
| `test_agent_empty_directives_suppresses_defaults` | AI unit | Agent with `directives: []` has no directives block |
| `test_directives_ordering_with_docs_and_think_first` | AI unit | Order: `<directives>` → `<documentation>` → prompt → thinkFirst suffix |
