# Creating Custom Agents

> **TLDR:** Define custom AI agents via YAML config — no code changes or recompilation required.

**Audience**: Users, DevOps
**WHAT**: How to define, configure, and manage custom AI agents in swebash
**WHY**: The 7 built-in agents cover common tasks, but teams often need domain-specific agents with tailored prompts, tool access, and trigger keywords
**HOW**: Create a YAML config file with agent definitions — no code changes or recompilation required

---

## Table of Contents

- [Overview](#overview)
- [Config File Location](#config-file-location)
- [YAML Schema](#yaml-schema)
- [Examples](#examples)
- [Usage](#usage)
- [Prompt Design Tips](#prompt-design-tips)
- [Troubleshooting](#troubleshooting)
- [Environment Variables](#environment-variables)


## Overview

swebash ships with 7 built-in agents (shell, review, devops, git, web, seaaudit, rscagent). You can add custom agents or override built-in ones by creating a YAML config file — no code changes or recompilation required.

## Config File Location

swebash looks for a user agents file in this order:

| Priority | Location |
|----------|----------|
| 1 (highest) | `$SWEBASH_AGENTS_CONFIG` environment variable |
| 2 | `~/.config/swebash/agents.yaml` |
| 3 | `~/.swebash/agents.yaml` |

The first file found wins. If none exist, only the 7 built-in agents are loaded.

## YAML Schema

```yaml
version: 1

# Optional — defaults applied to agents that omit these fields
defaults:
  temperature: 0.5        # LLM sampling temperature (0.0–1.0)
  maxTokens: 1024         # Max tokens per response
  thinkFirst: false       # Prepend "explain reasoning before acting" to prompts
  bypassConfirmation: false  # Skip tool confirmation prompts
  maxIterations: ~        # Max tool-use iterations (null = no limit)
  tools:
    fs: true              # Filesystem access (read files, list dirs)
    exec: true            # Command execution
    web: true             # Web search

# Agent definitions
agents:
  - id: my-agent                    # Unique ID — used with @my-agent
    name: My Agent                  # Display name shown in `ai agents`
    description: What this agent does  # One-line summary
    systemPrompt: |                 # LLM system prompt — defines behavior
      You are a specialist in...
    triggerKeywords: [keyword1, keyword2]  # Auto-detect triggers (optional)
    temperature: 0.3                # Override default (optional)
    maxTokens: 2048                 # Override default (optional)
    maxIterations: 15               # Override default (optional)
    thinkFirst: true                # Override default (optional)
    bypassConfirmation: false       # Override default (optional)
    tools:                          # Override default tool access (optional)
      fs: true
      exec: false
      web: false
```

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier. Used in `@id` to switch. Lowercase, no spaces. |
| `name` | string | Display name shown in agent listings |
| `description` | string | One-line description of the agent's purpose |
| `systemPrompt` | string | The system prompt that defines agent behavior |

### Optional Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `temperature` | float | `0.5` | LLM sampling temperature (0.0 = deterministic, 1.0 = creative) |
| `maxTokens` | int | `1024` | Maximum tokens per LLM response |
| `maxIterations` | int | none | Cap on tool-use iterations per conversation turn |
| `thinkFirst` | bool | `false` | Appends "explain reasoning before acting" instruction to prompt |
| `bypassConfirmation` | bool | `false` | Skip confirmation prompts before tool execution |
| `triggerKeywords` | list | `[]` | Keywords that auto-switch to this agent when detected in input |
| `tools.fs` | bool | `true` | Enable filesystem tools (read files, list directories, check existence) |
| `tools.exec` | bool | `true` | Enable command execution tool |
| `tools.web` | bool | `true` | Enable web search tool |

**Note**: Tool access is intersected with global `SWEBASH_AI_TOOLS_*` env vars. An agent cannot enable a tool that is globally disabled — it can only further restrict.

## Examples

### Add a security scanner agent

Create `~/.config/swebash/agents.yaml`:

```yaml
version: 1
agents:
  - id: security
    name: Security Scanner
    description: Scans code for vulnerabilities and security issues
    triggerKeywords: [security, scan, cve, vulnerability]
    tools:
      fs: true
      exec: true
      web: false
    systemPrompt: |
      You are a security auditor embedded in swebash.

      You specialize in:
      - OWASP Top 10 vulnerabilities
      - Dependency auditing (cargo audit, npm audit)
      - Secret detection in source code
      - Common vulnerability patterns (injection, XSS, SSRF)

      You have filesystem and command execution access.

      Rules:
      - Read the actual source code before making any assessment.
      - Categorize findings by severity: critical, high, medium, low.
      - Reference specific file paths and line numbers.
      - Suggest concrete fixes, not vague recommendations.
```

### Add a database assistant

```yaml
version: 1
agents:
  - id: db
    name: Database Assistant
    description: Helps with SQL, migrations, and database operations
    triggerKeywords: [sql, database, migration, postgres, mysql, sqlite]
    temperature: 0.3
    tools:
      fs: true
      exec: true
      web: false
    systemPrompt: |
      You are a database assistant embedded in swebash.

      You specialize in:
      - SQL queries (PostgreSQL, MySQL, SQLite)
      - Schema design and migrations
      - Query optimization and EXPLAIN analysis
      - Backup and restore operations

      Rules:
      - Always EXPLAIN before running destructive queries.
      - Warn before DROP, TRUNCATE, or DELETE without WHERE.
      - Prefer migrations over ad-hoc schema changes.
```

### Add multiple agents in one file

```yaml
version: 1

defaults:
  temperature: 0.4
  maxTokens: 2048
  tools:
    fs: true
    exec: true
    web: false

agents:
  - id: security
    name: Security Scanner
    description: Scans code for vulnerabilities
    triggerKeywords: [security, scan, cve]
    systemPrompt: |
      You are a security auditor...

  - id: db
    name: Database Assistant
    description: SQL and database operations
    triggerKeywords: [sql, database, migration]
    temperature: 0.3
    systemPrompt: |
      You are a database assistant...

  - id: docs
    name: Documentation Writer
    description: Writes and improves documentation
    triggerKeywords: [docs, readme, document]
    tools:
      fs: true
      exec: false
      web: false
    systemPrompt: |
      You are a technical writer...
```

### Override a built-in agent

To customize the built-in `shell` agent, use the same `id`:

```yaml
version: 1
agents:
  - id: shell
    name: My Shell
    description: Customized shell assistant
    systemPrompt: |
      You are my personal shell assistant.
      Always respond in bullet points.
      Never run commands without explaining them first.
```

This replaces the default `shell` agent entirely. Other built-in agents remain unchanged.

### Read-only agent (no tools)

```yaml
version: 1
agents:
  - id: tutor
    name: Programming Tutor
    description: Explains concepts without running code
    triggerKeywords: [explain, teach, how does]
    tools:
      fs: false
      exec: false
      web: false
    systemPrompt: |
      You are a programming tutor. Explain concepts clearly
      using examples. Do not execute code — teach the user
      to do it themselves.
```

## Usage

### Switch to an agent

```
# From shell mode
ai @security scan main.rs       # One-shot: send message, then return to shell
ai @security                     # Enter AI mode with the security agent

# From AI mode
@security                        # Switch to security agent
@shell                           # Switch back to shell agent
```

### List all agents

```
# From shell mode
ai agents

# From AI mode
agents
```

### Auto-detection

If `triggerKeywords` are set and auto-detection is enabled (`SWEBASH_AI_AGENT_AUTO_DETECT=true`, the default), typing a keyword in AI mode automatically switches to the matching agent:

```
[AI:shell] > scan this file for vulnerabilities
→ Auto-detected: Switched to Security Scanner (security)
```

## Prompt Design Tips

1. **State the role first**: "You are a ___ embedded in swebash."
2. **List specializations**: What the agent knows about.
3. **Declare tool access**: Tell the agent what tools it has so it doesn't hallucinate capabilities.
4. **Set rules**: Constraints on behavior (severity categories, when to warn, output format).
5. **Keep it focused**: A specialist agent with a clear scope outperforms a generalist.
6. **Use `thinkFirst: true`** for complex tasks where you want the agent to plan before acting.

## Troubleshooting

| Problem | Cause | Fix |
|---------|-------|-----|
| Custom agent not showing | Config file not found | Check file location matches lookup order above |
| Agent loads but no tools work | Global tool config disabled | Check `SWEBASH_AI_TOOLS_*` env vars |
| Invalid YAML silently ignored | Parse error in config | Validate YAML syntax (e.g. `python -c "import yaml; yaml.safe_load(open('agents.yaml'))"`) |
| Agent auto-detects too aggressively | Trigger keywords too common | Use specific, distinctive keywords |
| Override not taking effect | Wrong `id` field | Ensure `id` exactly matches the built-in agent ID |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_AGENTS_CONFIG` | *(none)* | Explicit path to agents YAML file |
| `SWEBASH_AI_DEFAULT_AGENT` | `shell` | Agent activated on startup |
| `SWEBASH_AI_AGENT_AUTO_DETECT` | `true` | Auto-detect agent from input keywords |
