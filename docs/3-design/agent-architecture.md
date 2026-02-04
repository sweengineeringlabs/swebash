# Agent Architecture

Specialized AI agents, each with its own system prompt, tool access, and conversation memory.

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
│  - ShellAgent, ReviewAgent, DevOpsAgent,    │
│    GitAgent                                 │
│  - create_default_registry()                │
├─────────────────────────────────────────────┤
│  rustratify primitives                      │
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

To add a custom agent, implement the `Agent` trait and register it in `create_default_registry()`:

```rust
struct MyAgent;

impl Agent for MyAgent {
    fn id(&self) -> &str { "my-agent" }
    fn display_name(&self) -> &str { "My Agent" }
    fn description(&self) -> &str { "Does something specific" }
    fn system_prompt(&self) -> String { "You are...".to_string() }
    fn tool_filter(&self) -> ToolFilter { ToolFilter::None }
    fn trigger_keywords(&self) -> Vec<&str> { vec!["mykey"] }
}
```
