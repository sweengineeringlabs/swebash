# Glossary

> **TLDR:** Definitions of domain terms used across swebash documentation.

**Audience**: All team members

**WHAT**: Definitions of domain terms used across swebash documentation
**WHY**: Ensures consistent terminology and reduces ambiguity for new contributors and users
**HOW**: Alphabetical term list with concise definitions

---

## Table of Contents

- [A](#a)
- [C](#c)
- [H](#h)
- [L](#l)
- [O](#o)
- [P](#p)
- [R](#r)
- [S](#s)
- [T](#t)
- [W](#w)
- [Y](#y)


## A

**AccessMode**
An enum (`ReadOnly` or `ReadWrite`) that controls what level of filesystem access is granted within a sandbox path rule.

**Agent**
A specialized AI assistant with its own system prompt, tool access, and conversation memory. Each agent is optimized for a specific domain (e.g., code review, Git operations, DevOps).

**Agent auto-detection**
A feature that automatically switches to the most relevant agent based on keywords detected in user input. Controlled by `SWEBASH_AI_AGENT_AUTO_DETECT`.

**Agent ID**
A unique lowercase identifier for an agent (e.g., `shell`, `review`, `git`). Used with the `@` prefix to switch agents (`@review`).

**AgentDefaults**
Default values (temperature, maxTokens, tools, thinkFirst, directives) applied to agents that omit optional fields. Defined in rustratify's `agent-controller::yaml` module.

**AgentDescriptor**
A Rust trait from rustratify's `agent-controller` crate that defines an agent's properties: ID, display name, system prompt, tool filter, and trigger keywords.

**AgentManager**
The internal registry that holds all loaded agents, creates chat engines on demand, and handles agent switching.

**AI mode**
An interactive REPL mode where all input is sent to the active AI agent. Entered via `ai` or `ai @<agent>`. Exited with `exit`.

## C

**ChatEngine**
A rustratify component (`SimpleChatEngine` or `ToolAwareChatEngine`) that manages conversation history and LLM interactions for a single agent.

**ConfigAgent**
A YAML-defined agent parsed from `default_agents.yaml` or a user config file. Wraps `YamlAgentDescriptor` (from `agent-controller::yaml`) via composition and adds swebash-specific fields: docs context, `bypassConfirmation`, and `maxIterations`. Implements `AgentDescriptor`.

## H

**Host**
The native Rust binary (`features/shell/host`) that runs the interactive REPL, manages AI mode, and delegates shell commands to the WASM engine.

## L

**LLM (Large Language Model)**
The AI model that generates responses. swebash supports OpenAI, Anthropic, and Gemini providers.

**LlmService**
A rustratify trait (`llm_provider::LlmService`) that abstracts LLM completion calls. Implemented differently by each provider.

## O

**One-shot agent chat**
Using `ai @review check main.rs` from shell mode: temporarily switches agent, sends the message, then restores the previous agent.

## P

**Provider**
An LLM backend service (OpenAI, Anthropic, Gemini). Configured via `LLM_PROVIDER` environment variable.

## R

**ReAct (Reasoning + Acting)**
An agent execution pattern where the LLM alternates between reasoning about the problem and taking actions via tool calls. Implemented by rustratify's `react` crate.

**Readline**
The terminal line-editing layer (`features/shell/readline`) built on crossterm. Provides arrow key navigation, history, hints, and tab completion.

**rustratify**
The external Rust framework that provides swebash's agent infrastructure: `chat-engine`, `llm-provider`, `agent-controller`, `tool`, and `react` crates.

## S

**SandboxPolicy**
A Rust struct in `host/src/spi/state.rs` that controls which filesystem paths the shell may access and how. Contains a workspace root, ordered path rules, and an enabled flag. Stored in `HostState` and checked by every filesystem host import.

**SEA (Stratified Encapsulation Architecture)**
An architectural pattern that organizes code into distinct layers (L4 core infrastructure, L5 domain) with strict dependency direction. Used by the `seaaudit` agent.

**Shell mode**
The default mode where input is either interpreted as a shell command (via WASM engine) or prefixed with `ai` to invoke AI.

**System prompt**
The hidden instruction given to the LLM before any user messages. Defines the agent's role, expertise, tool access, and behavioral rules.

## T

**Tab**
A single session within the shell. Each tab has its own ID, label, multiline buffer, recent commands, AI mode flag, and active agent. Shell tabs are backed by a WASM engine instance; mode tabs (AI, History) are lightweight with only a fallback CWD.

**`tab` command**
A shell builtin that manages tabs. Subcommands: `tab list` (or just `tab`), `tab new [path]`, `tab close`, `tab rename <name>`, `tab ai [agent]`, `tab history`, `tab <N>` (switch).

**TabBar**
A UI component rendered at terminal row 0 when 2+ tabs are open. Shows each tab as `[N:icon:label]` with the active tab in bold white and inactive tabs in grey. Implemented in `host/src/spi/tab_bar.rs`.

**TabId**
A `u32` unique identifier assigned to each tab by `TabManager`. Monotonically increasing; never reused within a session.

**TabInner**
An enum encoding tab content type: `Shell(WasmSession)` for WASM-backed shell tabs, `Ai { fallback_cwd }` for AI chat tabs, and `HistoryView { fallback_cwd }` for history browser tabs.

**TabKind**
A lightweight `Copy + Eq` discriminant (`Shell`, `Ai`, `HistoryView`) derived from `TabInner`. Used for matching tab type without borrowing the inner data.

**TabManager**
The struct in `host/src/spi/tab.rs` that owns all open tabs, tracks the active tab index, and provides methods for creating, closing, switching, and navigating tabs. Holds a shared `Arc<Mutex<History>>` for cross-tab history.

**thinkFirst**
An agent YAML option that appends "explain your reasoning before acting" to the system prompt, making the agent plan before using tools.

**Tool**
A capability available to an agent: filesystem access (`fs`), command execution (`exec`), or web search (`web`). Tools are registered in a `ToolRegistry`.

**ToolFilter**
Controls which tool categories an agent can access. `All` enables everything; `Categories(["fs"])` restricts to filesystem only. Intersected with global tool config.

**ToolsConfig**
A `HashMap<String, bool>` in `agent-controller::yaml` that maps tool category names (e.g., `fs`, `exec`, `web`, `rag`) to enabled/disabled state. Supports any category name — not hardcoded to specific categories.

**Trigger keywords**
Words that cause agent auto-detection. When a user types "scan this file" and the `security` agent has `triggerKeywords: [scan]`, swebash auto-switches to it.

## W

## Y

**YamlAgentDescriptor**
A concrete `AgentDescriptor` implementation in rustratify's `agent-controller::yaml` module. Built from YAML config with defaults merging, tool filter computation, directives block, and thinkFirst suffix. `ConfigAgent` wraps this via composition.

**Workspace**
The root directory for shell operations, controlled by the sandbox policy. Defaults to `~/workspace/`, overridable via `SWEBASH_WORKSPACE` env var or `~/.config/swebash/config.toml`.

**`workspace` command**
A shell builtin that displays and modifies the sandbox policy at runtime. Supports subcommands: `rw`, `ro`, `allow`, `enable`, `disable`.

**Workspace Sandbox**
A path-based access control layer in the host runtime that intercepts every filesystem host import. Enforces read-only or read-write access based on ordered path rules. Configured via TOML config file and `workspace` command.

**W3H (WHO-WHAT-WHY-HOW)**
The documentation structure pattern used across swebash docs. Every document declares its audience (WHO), scope (WHAT), motivation (WHY), and implementation (HOW).

**WASM engine**
The WebAssembly-based shell engine (`features/shell/engine`) compiled as a `cdylib`. Executes traditional shell commands independently of the AI layer.
