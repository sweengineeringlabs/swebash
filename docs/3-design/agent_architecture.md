# Agent Architecture

> **TLDR:** Multi-agent framework with isolated memory, tool filters, and YAML-configurable system prompts.

**Audience**: Developers, architects

Specialized AI agents, each with its own system prompt, tool access, and conversation memory.

## Table of Contents

- [Overview](#overview)
- [Built-in Agents](#built-in-agents)
- [Usage](#usage)
- [Architecture](#architecture)
- [Key Design Decisions](#key-design-decisions)
- [Configuration](#configuration)
- [Extending](#extending)


## Overview

The agent framework routes chat conversations to purpose-built agents. Each agent has:

- **System prompt** — defines the agent's expertise and behavior
- **Tool filter** — controls which tool categories (fs, exec, web) are available
- **Isolated memory** — each agent maintains its own conversation history via a dedicated `ChatEngine`

Agents compose existing rustratify primitives (`ChatEngine`, `ToolRegistry`, `LlmService`) without introducing new dependencies.

## Built-in Agents

| ID | Name | Tools | Keywords | Purpose |
|----|------|-------|----------|---------|
| `shell` | Shell Assistant | All | *(default)* | General-purpose shell assistant |
| `review` | Code Reviewer | fs only | review, audit | Code review for bugs, security, style |
| `devops` | DevOps Assistant | All | docker, k8s, terraform, deploy, pipeline | Infrastructure and deployment help |
| `git` | Git Assistant | fs + exec | git, commit, branch, merge, rebase | Git operations and branching strategies |
| `web` | Web Agent | web only | search, web, browse | Web search and summarization |
| `seaaudit` | SEA Audit Agent | fs + exec | sea, audit, architecture | SEA compliance auditing |
| `rscagent` | RustScript Agent | fs + exec | rsc, rustscript, component | RustScript framework development |
| `docreview` | Documentation Reviewer | fs only | docs, documentation, review docs | Documentation review against SEA framework |
| `clitester` | CLI Tester | fs + exec | clitest, manual test | CLI and shell manual test scenarios |
| `apitester` | API Tester | fs + exec | apitest, api test | AI and agent manual test scenarios |

## Usage

### Shell mode

```
ai agents                        # list all agents
ai @review check main.rs         # one-shot: chat with review agent, then return
ai @git                          # switch to git agent and enter AI mode
```

### AI mode

```
@review                          # switch to review agent
@review check this file           # one-shot message to review agent
agents                           # list agents
exit                             # return to shell
```

### Prompt

The AI mode prompt shows the active agent:

```
[AI:shell] >                     # default agent
[AI:review] >                    # after switching to review
[AI:git] >                       # after switching to git
```

## Architecture

```
┌─────────────────────────────────────────────┐
│  Host REPL (main.rs)                        │
│  - Parses @agent syntax                     │
│  - Tracks ai_agent_id for prompt            │
│  - Delegates to handle_ai_command()         │
├─────────────────────────────────────────────┤
│  AiService trait (api/mod.rs)               │
│  - switch_agent(id)                         │
│  - current_agent() -> AgentInfo             │
│  - list_agents() -> Vec<AgentInfo>          │
│  - chat() / chat_streaming()                │
├─────────────────────────────────────────────┤
│  DefaultAiService (core/mod.rs)             │
│  - agents: AgentRegistry                    │
│  - active_agent: RwLock<String>             │
│  - active_engine() -> ChatEngine            │
│  - auto_detect_and_switch(input)            │
├─────────────────────────────────────────────┤
│  AgentRegistry (core/agents/mod.rs)         │
│  - register(Box<dyn Agent>)                 │
│  - engine_for(id) -> Arc<ChatEngine>        │
│  - detect_agent(input) -> Option<&str>      │
│  - clear_agent(id) / clear_all()            │
├─────────────────────────────────────────────┤
│  Built-in Agents (core/agents/builtins.rs)  │
│  - 10 agents loaded from YAML              │
│  - create_default_registry()                │
│  - Multi-layer: embedded → project → user   │
├─────────────────────────────────────────────┤
│  ConfigAgent (core/agents/config.rs)        │
│  - Wraps YamlAgentDescriptor (composition)  │
│  - Adds swebash-specific fields:            │
│    docs, bypassConfirmation, maxIterations  │
├─────────────────────────────────────────────┤
│  rustratify primitives                      │
│  - agent-controller::yaml module            │
│    (AgentEntry, AgentDefaults, ToolsConfig, │
│     YamlAgentDescriptor)                    │
│  - ChatEngine (SimpleChatEngine /           │
│    ToolAwareChatEngine)                     │
│  - ToolRegistry + create_standard_registry  │
│  - LlmService                              │
└─────────────────────────────────────────────┘
```

## Key Design Decisions

### Lazy engine creation

Chat engines are created on first use and cached for the session. This avoids allocating resources for agents that are never used while ensuring fast subsequent access.

### Tool filter composability

An agent's `ToolFilter` is intersected with the global `ToolConfig` from environment variables. An agent cannot enable tools that are globally disabled — it can only further restrict.

```
Global: fs=true, exec=true, web=true
Review: Only { fs: true, exec: false, web: false }
Effective: fs=true, exec=false, web=false
```

### One-shot agent chat

`ai @review check main.rs` temporarily switches to the review agent, sends the message, then restores the previous agent. This provides targeted expertise without disrupting the user's current agent context.

### Memory isolation

Each agent has its own `ChatEngine` instance with independent conversation history. Switching agents does not lose context — you can switch back and resume where you left off.

## Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `SWEBASH_AI_DEFAULT_AGENT` | `shell` | Agent activated on startup |
| `SWEBASH_AI_AGENT_AUTO_DETECT` | `true` | Auto-detect agent from input keywords |

## Extending

Agents are defined in YAML — no Rust code changes required. See [Creating Agents](../7-operation/creating_agents.md) for the full schema.

```yaml
# ~/.config/swebash/agents.yaml
version: 1
agents:
  - id: my-agent
    name: My Agent
    description: Does something specific
    triggerKeywords: [mykey]
    tools:
      fs: true
      exec: false
      web: false
    systemPrompt: |
      You are a specialist in...
```

### Internal architecture

Agents are parsed from YAML into `ConfigAgent`, which wraps rustratify's `YamlAgentDescriptor` (from the `agent-controller` crate's `yaml` feature module) via composition. Generic YAML parsing, defaults merging, tool filter computation, and prompt augmentation (directives, thinkFirst) live in rustratify. Swebash-specific concerns — docs loading, RAG integration, `bypassConfirmation`, `maxIterations` — remain in `ConfigAgent`.

Key types:

| Type | Crate | Purpose |
|------|-------|---------|
| `YamlAgentDescriptor` | `agent-controller::yaml` | Generic `AgentDescriptor` built from YAML with defaults merging |
| `AgentEntry<Ext>` | `agent-controller::yaml` | Generic per-agent YAML entry, extensible via `#[serde(flatten)]` |
| `AgentDefaults` | `agent-controller::yaml` | Default values (temperature, maxTokens, tools, thinkFirst, directives) |
| `ToolsConfig` | `agent-controller::yaml` | `HashMap<String, bool>` — generic tool category toggles |
| `ConfigAgent` | `swebash-ai` | Wraps `YamlAgentDescriptor` + swebash-specific fields (docs, bypass, iterations) |
| `SwebashAgentsYaml` | `swebash-ai` | Swebash YAML root — adds `rag` config and extended defaults |
| `SwebashAgentExt` | `swebash-ai` | Extension fields for `AgentEntry` (docs, bypassConfirmation, maxIterations) |
