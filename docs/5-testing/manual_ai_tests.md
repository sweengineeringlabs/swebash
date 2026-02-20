# Manual AI Tests

> **TLDR:** Manual test checklist for AI features: commands, agents, config, memory, docs context, and auto-detection.

**Audience**: Developers, QA

**WHAT**: Manual test procedures for AI-powered shell features
**WHY**: Validates AI command dispatch, agent lifecycle, YAML config, and docs integration
**HOW**: Step-by-step test tables with expected outcomes

> Requires `LLM_PROVIDER` set and at least one credential source: Claude Code OAuth (`~/.claude/.credentials.json`) or `ANTHROPIC_API_KEY` for Anthropic; equivalent key for OpenAI/Gemini. AI is enabled by default — no need to set `SWEBASH_AI_ENABLED=true`. See [Manual Testing Hub](manual_testing.md) for full prerequisites.

---

## Table of Contents

- [Authentication](#6b-authentication)
- [AI Status](#7-ai-status)
- [AI Ask](#8-ai-ask-nl-to-command)
- [AI Explain](#9-ai-explain)
- [AI Chat Mode](#10-ai-chat-mode)
- [AI Chat from Shell](#11-ai-chat-from-shell)
- [AI Suggest](#12-ai-suggest)
- [Agent Listing](#13-agent-listing)
- [Agent Switching (AI Mode)](#14-agent-switching-ai-mode)
- [Agent Switching (Shell Mode)](#14b-agent-switching-from-shell-mode)
- [One-Shot Agent Chat](#15-one-shot-agent-chat-shell-mode)
- [Auto-Detection](#16-auto-detection-ai-mode)
- [Agent Memory Isolation](#17-agent-memory-isolation)
- [History and Clear](#17b-history-and-clear-commands)
- [User-Configurable Agents](#18-user-configurable-agents-yaml)
- [Shared Directives](#19-shared-directives)
- [Agent docs_context](#19b-agent-docs_context)
- [Agent maxIterations](#19d-agent-maxiterations)
- [DevOps Agent](#20-devops-agent-docker-specific)
- [AWS Cloud Agent](#20b-aws-cloud-agent-user-level)
- [Request/Response Logging](#21-requestresponse-logging)

---

## 6b. Authentication

For the `anthropic` provider, the shell tries credentials in this order:

1. **Claude Code OAuth** — `~/.claude/.credentials.json` (set by Claude Code IDE/CLI)
2. **API key** — `ANTHROPIC_API_KEY` environment variable

Either source is sufficient. Both sources absent → `NotConfigured` error on startup; AI commands will not respond.

| Test | Steps | Expected |
|------|-------|----------|
| OAuth primary (Anthropic) | Ensure `~/.claude/.credentials.json` exists (Claude Code installed); unset `ANTHROPIC_API_KEY`; run `export LLM_PROVIDER=anthropic && ./sbh run` | Shell starts; `ai status` shows `Enabled: yes, Ready: yes` |
| API key fallback (Anthropic) | Remove/rename `~/.claude/.credentials.json`; set `ANTHROPIC_API_KEY`; run shell | Shell starts; `ai status` shows `Enabled: yes, Ready: yes` |
| No credentials error | Remove both OAuth file and `ANTHROPIC_API_KEY`; start shell | Shell starts but AI service unavailable; `ai status` prints error: _"No credentials found for provider 'anthropic'. Configure Claude Code OAuth or set ANTHROPIC_API_KEY."_ |
| OpenAI unaffected | `LLM_PROVIDER=openai` with `OPENAI_API_KEY` set | OAuth file is irrelevant; API key used as normal |
| Disabled explicitly | `SWEBASH_AI_ENABLED=false ./sbh run` then `ai status` | `ai status` returns error: _"AI features disabled (SWEBASH_AI_ENABLED=false)"_ |
| Enabled by default | Start shell with valid credentials; do not set `SWEBASH_AI_ENABLED` | AI is active without any extra flag |

## 7. AI Status

| Test | Command | Expected |
|------|---------|----------|
| Status | `ai status` | Shows `Enabled: yes`, `Provider: <name>`, `Model: <name>`, `Ready: yes` |
| Status — no credentials | Start shell without any credential source (see §6b) | `ai status` returns an error; no status table printed |

## 8. AI Ask (NL to Command)

| Test | Command | Expected |
|------|---------|----------|
| Ask via subcommand | `ai ask list all files` | Suggests a command (e.g. `ls -la`), prompts Execute? |
| Ask via shorthand | `? find rust files` | Same behavior as `ai ask` |
| Cancel execution | Press `n` at Execute? prompt | Prints "Cancelled", returns to shell |

## 9. AI Explain

| Test | Command | Expected |
|------|---------|----------|
| Explain via subcommand | `ai explain ls -la` | Natural language explanation of the command |
| Explain via shorthand | `?? ps aux \| grep rust` | Explains the pipeline |
| Explain simple | `ai explain echo test` | Short explanation, no leading/trailing whitespace |

## 10. AI Chat Mode

| Test | Steps | Expected |
|------|-------|----------|
| Enter mode | Type `ai` | Prints "Entered AI mode", prompt changes to `[AI:shell] >` |
| Basic chat | Type a question | Shows "thinking...", then a response |
| Multi-turn memory | Say "My name is Alice", then "What is my name?" | Second reply mentions Alice |
| Exit mode | Type `quit` or `exit` | Prints "Exited AI mode", prompt returns to normal |

## 11. AI Chat from Shell

| Test | Command | Expected |
|------|---------|----------|
| Direct chat | `ai chat what is Rust?` | Prints AI response inline without entering AI mode |

## 12. AI Suggest

| Test | Command | Expected |
|------|---------|----------|
| Suggest | `ai suggest` | Shows autocomplete suggestions based on recent commands |

## 13. Agent Listing

| Test | Command | Expected |
|------|---------|----------|
| List from shell | `ai agents` | Prints table of 10 agents (shell\*, review, devops, git, web, seaaudit, rscagent, docreview, clitester, apitester) with descriptions. Active agent marked with `*`. |
| List from AI mode | `ai` then `agents` | Same table, shown inside AI mode |

## 14. Agent Switching (AI Mode)

| Test | Steps | Expected |
|------|-------|----------|
| Switch to review | `ai` then `@review` | Prints "Switched to Code Reviewer (review)", prompt changes to `[AI:review] >` |
| Switch to git | `@git` | Prompt changes to `[AI:git] >` |
| Switch to devops | `@devops` | Prompt changes to `[AI:devops] >` |
| Switch back to shell | `@shell` | Prompt changes to `[AI:shell] >` |
| Multiple switches | `@git` → `@review` → `@shell` → `exit` | Each switch updates prompt, exit works cleanly |
| Active marker follows | `@review` then `agents` | Agent list shows `*review` as active |

## 14b. Agent Switching (from Shell Mode)

Typing `@agent` directly from the shell prompt (without entering AI mode first with `ai`) should switch to the agent **and** enter AI mode. Natural language input must be routed to the AI, not executed as a shell command.

| Test | Steps | Expected |
|------|-------|----------|
| @devops enters AI mode | Type `@devops` from shell prompt | Prints "Switched to DevOps Assistant (devops)" and "Entered AI mode", prompt changes to `[AI:devops] >` |
| @git enters AI mode | Type `@git` from shell prompt | Prints "Switched to Git Assistant (git)" and "Entered AI mode", prompt changes to `[AI:git] >` |
| @review enters AI mode | Type `@review` from shell prompt | Prints "Switched to Code Reviewer (review)" and "Entered AI mode", prompt changes to `[AI:review] >` |
| NL not executed as command | `@devops` then `do we have docker installed?` | AI responds (or shows "not configured"); does **not** print "No such file or directory" |
| Exit returns to shell | `@devops` then `exit` then `echo hello` | Exits AI mode, `echo hello` prints `hello` normally |
| `ai @agent` also works | Type `ai @devops` from shell prompt | Same behavior as `@devops` — enters AI mode with devops agent |

## 15. One-Shot Agent Chat (Shell Mode)

| Test | Command | Expected |
|------|---------|----------|
| One-shot devops | `ai @devops how do I check running containers` | Prints `[devops] DevOps Assistant`, shows response with Docker commands, returns to shell prompt (not AI mode) |
| One-shot review | `ai @review check main.rs` | Prints `[review] Code Reviewer`, shows code review response, returns to shell prompt |
| One-shot awscli | `ai @awscli list my S3 buckets` | Prints `[awscli] AWS Cloud Assistant`, shows AWS CLI guidance, returns to shell prompt |
| Agent restored | `ai @devops hello` then `ai` then `exit` | After one-shot, entering AI mode still shows `[AI:shell]` (previous agent restored) |

## 16. Auto-Detection (AI Mode)

Auto-detection switches the active agent based on keywords in the input.

| Test | Steps | Expected |
|------|-------|----------|
| Docker keyword | `ai` then `docker ps` | Prints "Switched to DevOps Assistant (devops)", prompt changes to `[AI:devops] >`, then shows explanation |
| Docker compose | `docker compose up` (while in AI mode) | Stays on devops (or switches if on another agent), shows explanation |
| K8s keyword | `k8s get pods` | Switches to devops if not already, shows kubectl guidance |
| Git keyword | `git rebase` | Prints "Switched to Git Assistant (git)", prompt changes to `[AI:git] >`, shows explanation |
| AWS keyword | `aws s3 ls` | Switches to awscli agent (if user config loaded), prompt changes to `[AI:awscli] >` |
| No match stays | `how do I list files` | No switch message, stays on current agent |
| Prompt tracks agent | Observe prompt after each auto-switch | Prompt always reflects current agent: `[AI:shell]`, `[AI:devops]`, `[AI:git]`, etc. |

## 17. Agent Memory Isolation

| Test | Steps | Expected |
|------|-------|----------|
| Isolated history | `ai` → chat with shell → `@review` → chat with review → `@shell` | Returning to shell still has shell's conversation context, review has its own |
| Clear history | `clear` then `history` | Shows "(no chat history)" for active agent only |

## 17b. History and Clear Commands

| Test | Steps | Expected |
|------|-------|----------|
| History shows messages | `ai` → send a few messages → `history` | Prints numbered list of recent user/assistant messages for the active agent |
| History empty | `ai` → `@review` → `history` | Shows "(no chat history)" since review agent has no messages yet |
| Clear resets agent | `ai` → send a message → `clear` → `history` | Shows "(no chat history)" — only active agent's history is cleared |
| Clear does not affect others | `ai` → chat with shell → `@review` → `clear` → `@shell` → `history` | Shell agent history is still intact after clearing review |

## 18. User-Configurable Agents (YAML)

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
| Add custom agent | Create `~/.config/swebash/agents.yaml` with a new agent (e.g. `id: security`), restart shell | `ai agents` lists 11 agents (10 defaults + custom) |
| Switch to custom agent | `@security` | Prints "Switched to Security Scanner (security)", prompt shows `[AI:security] >` |
| Custom trigger keywords | Add `triggerKeywords: [scan, cve]` to custom agent, restart, enter AI mode | Typing `scan this file` auto-detects the custom agent |
| Override built-in agent | Add `id: shell` entry with custom `name` and `description` to user YAML, restart | `ai agents` shows custom name/description for shell agent; still 10 agents total |
| Invalid user YAML | Write broken YAML to the config file, restart | Shell starts normally with 10 default agents (invalid file silently ignored) |
| Env var override | `export SWEBASH_AGENTS_CONFIG=/path/to/agents.yaml`, restart shell | Agents from the specified file are loaded |
| No config file | Ensure no user YAML exists anywhere, restart | Shell starts normally with 10 default agents |
| Project-local config | Create `.swebash/agents.yaml` in project root with a new agent (e.g. `id: projbot`), restart shell | `ai agents` lists 11 agents (10 defaults + project agent) |
| Project-local override | Add `id: shell` entry to `.swebash/agents.yaml` with custom name, restart | Shell agent shows custom name (project layer overrides built-in) |
| User overrides project | Same `id` in both project-local and user-level config, restart | User-level config wins (loaded last) |
| Docs base dir | `export SWEBASH_AI_DOCS_BASE_DIR=/path/to/project`, restart | Project-local config is read from `/path/to/project/.swebash/agents.yaml` |

<details>
<summary>Example user agents.yaml (awscli — deployed at ~/.config/swebash/agents.yaml)</summary>

```yaml
version: 1
agents:
  - id: awscli
    name: AWS Cloud Assistant
    description: Assists with AWS CLI, CDK, SAM, CloudFormation, Terraform, and cloud infrastructure
    tools:
      fs: true
      exec: true
      web: false
    maxIterations: 25
    triggerKeywords: [aws, s3, ec2, lambda, iam, cloudformation, cdk, sam, ecs, rds, dynamodb, sqs, sns, route53, cloudwatch, terraform]
    docs:
      budget: 12000
      sources:
        - docs/aws/services_reference.md
        - docs/aws/iac_patterns.md
        - docs/aws/troubleshooting.md
    systemPrompt: |
      You are an AWS Cloud assistant embedded in swebash...
```

Docs files live at `~/.config/swebash/docs/aws/`.

</details>

## 19. Shared Directives

Directives are quality standards defined in the `defaults.directives` section of `default_agents.yaml`. They are automatically prepended as a `<directives>` block to every agent's system prompt.

| Test | Steps | Expected |
|------|-------|----------|
| Directives present | `ai` then ask agent to describe its system prompt, or inspect via debug logging | System prompt starts with `<directives>` block containing 7 quality directives |
| Directives before docs | Switch to `@rscagent` or `@docreview` (agents with docs) | `<directives>` block appears before `<documentation>` block in system prompt |
| Directives before prompt | Inspect any agent's system prompt | `<directives>` block appears before the agent's own system prompt text |
| All agents inherit | Check system prompts of shell, review, devops, git, web, seaaudit, rscagent, docreview, clitester, apitester | All agents have the same `<directives>` block from defaults |
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

## 19b. Agent docs_context

Agents can declare a `docs` section in their YAML with a `budget` (max characters) and a list of `sources` (file globs resolved relative to `SWEBASH_AI_DOCS_BASE_DIR`). Matching files are loaded and prepended to the system prompt inside a `<documentation>` block.

| Test | Steps | Expected |
|------|-------|----------|
| rscagent has docs | Switch to `@rscagent`, inspect system prompt via debug logging or ask agent | System prompt contains `<documentation>` block with content from its 20 source files |
| docreview has docs | Switch to `@docreview`, inspect system prompt | System prompt contains `<documentation>` block with content from its 4 source files |
| awscli has docs | Switch to `@awscli` (requires user config + `SWEBASH_AI_DOCS_BASE_DIR=~/.config/swebash`), inspect system prompt | System prompt contains `<documentation>` block with content from 3 AWS doc files (services_reference.md, iac_patterns.md, troubleshooting.md) |
| Docs before prompt | Inspect `@rscagent` system prompt | `<directives>` block appears first, then `<documentation>` block, then agent's own prompt text |
| Missing sources skipped | Set `SWEBASH_AI_DOCS_BASE_DIR` to a directory without doc files, restart | Agents start normally; missing doc sources are silently skipped |
| Budget truncation | Create an agent YAML with `docs: { budget: 100, sources: [large-file.md] }`, restart | Documentation is truncated to approximately 100 characters |

## 19d. Agent maxIterations

Some agents override the global `SWEBASH_AI_TOOLS_MAX_ITER` with a per-agent `maxIterations` value in YAML. This limits how many tool-calling iterations the agent performs per turn.

| Test | Steps | Expected |
|------|-------|----------|
| seaaudit has 25 | Check seaaudit agent config or debug logging | `maxIterations` is 25 (vs global default of 10) |
| rscagent has 20 | Check rscagent agent config | `maxIterations` is 20 |
| docreview has 25 | Check docreview agent config | `maxIterations` is 25 |
| awscli has 25 | Check awscli agent config (user-level) | `maxIterations` is 25 |
| Default agents use global | Check shell, review, devops, git agents | `maxIterations` inherits from global config (default 10) |

## 20. DevOps Agent (Docker-specific)

> Requires Docker installed (`docker --version`). Permission errors are expected if user is not in the `docker` group.

| Test | Command | Expected |
|------|---------|----------|
| Container listing | `ai @devops list running docker containers` | DevOps agent responds with `docker ps` guidance and permission troubleshooting if needed |
| Docker images | `ai @devops what docker images do I have` | Explains `docker images` command, offers permission fix |
| Dockerfile help | `ai @devops how do I write a Dockerfile for nginx` | Provides Dockerfile example with explanation |
| Compose guidance | `ai` then `docker compose up` | Auto-switches to devops, explains compose command |
| K8s guidance | `ai` then `k8s get pods` | Devops agent explains kubectl, offers install instructions |

## 20b. AWS Cloud Agent (User-Level)

> Requires `~/.config/swebash/agents.yaml` with the `awscli` agent and `~/.config/swebash/docs/aws/` reference docs. Set `SWEBASH_AI_DOCS_BASE_DIR=~/.config/swebash` if docs are stored there.

| Test | Steps | Expected |
|------|-------|----------|
| Agent listed | `ai agents` | Lists 11 agents (10 built-in + awscli); awscli shows "Assists with AWS CLI, CDK, SAM, CloudFormation, Terraform, and cloud infrastructure" |
| Switch to awscli | `ai` then `@awscli` | Prints "Switched to AWS Cloud Assistant (awscli)", prompt changes to `[AI:awscli] >` |
| One-shot awscli | `ai @awscli how do I list S3 buckets` | Prints `[awscli] AWS Cloud Assistant`, shows `aws s3 ls` guidance, returns to shell prompt |
| Auto-detect aws | `ai` then `aws s3 ls` | Switches to awscli agent (if not already), shows explanation |
| Auto-detect ec2 | `ai` then `ec2 describe-instances` | Switches to awscli agent |
| Auto-detect lambda | `ai` then `lambda invoke my-func` | Switches to awscli agent |
| Auto-detect terraform | `ai` then `terraform plan` | Switches to awscli agent |
| Docs loaded | Switch to `@awscli`, inspect system prompt via debug logging or ask agent | System prompt contains `<documentation>` block with content from 3 AWS doc files |
| maxIterations is 25 | Check awscli agent config or debug logging | `maxIterations` is 25 |
| Tools fs+exec only | Check awscli agent config | `fs: true`, `exec: true`, `web: false` |
| 16 trigger keywords | Check awscli agent config | Keywords: aws, s3, ec2, lambda, iam, cloudformation, cdk, sam, ecs, rds, dynamodb, sqs, sns, route53, cloudwatch, terraform |
| @awscli from shell | Type `@awscli` from shell prompt | Enters AI mode with awscli agent, prompt shows `[AI:awscli] >` |
| Exit returns to shell | `@awscli` then `exit` then `echo hello` | Exits AI mode, `echo hello` prints `hello` normally |

## 21. Request/Response Logging

Setting `SWEBASH_AI_LOG_DIR` enables two complementary observability layers that each write one JSON file per request:

| Layer | `kind` field | Captures |
|-------|-------------|----------|
| `LoggingAiClient` (higher) | `"ai-complete"` | `AiMessage[]`, `CompletionOptions`, `AiResponse` |
| `LoggingLlmService` (lower) | `"complete"` / `"complete_stream"` | Raw `CompletionRequest` with tool definitions |

Both layers write to the same directory and are activated by the same env var.

### Setup

```sh
export SWEBASH_AI_LOG_DIR=/tmp/swebash-ai-logs
mkdir -p $SWEBASH_AI_LOG_DIR
```

### Manual Test Cases

| Test | Steps | Expected |
|------|-------|----------|
| Logging disabled by default | Start shell without `SWEBASH_AI_LOG_DIR`, run `ai ask list files` | No log files created anywhere |
| Log dir created automatically | Set `SWEBASH_AI_LOG_DIR` to a non-existent path, run `ai ask list files` | Directory is created; log files appear inside it |
| ai-complete file created | `export SWEBASH_AI_LOG_DIR=/tmp/swebash-ai-logs`, run `ai ask list files` | One `*-ai-complete.json` file written to log dir |
| complete file created | Same setup, run `ai ask list files` | One `*-complete.json` file also written (LLM layer) |
| Streaming logs | Same setup, run `ai` then ask something in chat mode | One `*-complete_stream.json` file written when stream ends |
| Error logged | Temporarily revoke API key or saturate quota, run `ai ask test` | Log file written with `"status": "error"` and error message |
| Multiple requests | Run `ai ask` three times | Three separate `*-ai-complete.json` files with unique UUIDs |
| Unset var disables logging | `unset SWEBASH_AI_LOG_DIR`, run `ai ask list files` | No new files written |

### Inspecting a log file

```sh
ls -lt $SWEBASH_AI_LOG_DIR | head -5    # most recent files first
cat $SWEBASH_AI_LOG_DIR/<uuid>-ai-complete.json | python3 -m json.tool
```

**Expected `ai-complete` file structure:**

```json
{
  "id": "<uuid>-ai-complete",
  "timestamp_epoch_ms": 1700000000000,
  "duration_ms": 842,
  "kind": "ai-complete",
  "request": {
    "messages": [
      { "role": "system", "content": "..." },
      { "role": "user", "content": "list files" }
    ],
    "options": { "temperature": 0.1, "max_tokens": 256 }
  },
  "result": {
    "status": "success",
    "response": {
      "content": "ls -la",
      "model": "claude-sonnet-4-20250514"
    }
  }
}
```

**Expected `complete` file structure** (lower LLM layer, includes tool definitions):

```json
{
  "id": "<uuid>-complete",
  "kind": "complete",
  "request": {
    "model": "claude-sonnet-4-20250514",
    "messages": [...],
    "tools": [...]
  },
  "result": {
    "status": "success",
    "response": { "content": "...", "model": "...", "finish_reason": "stop" }
  }
}
```

---

## Automated Agent Tests

For the full list of automated agent tests (60+ tests), see the [Automated Test Suites](manual_testing.md#automated-test-suites) section in the testing hub.

## See Also

- [Manual Testing Hub](manual_testing.md) — prerequisites and setup
- [Manual RAG Tests](manual_rag_tests.md) — RAG integration tests
- [Manual Shell Tests](manual_shell_tests.md) — shell feature tests
- [Agent Architecture](../3-design/agent_architecture.md) — agent framework design
- [AI Integration](../3-design/ai_integration.md) — LLM isolation and provider abstraction
- [Creating Agents](../7-operations/creating_agents.md) — custom agent creation guide
