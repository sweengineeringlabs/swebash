# Software Requirements Specification: swebash

**Audience**: Developers, architects, project stakeholders

## TLDR

This SRS defines requirements for swebash, a WASM-based Unix-like shell with integrated AI assistance. The shell compiles its command engine to WebAssembly for isolation, runs a native host runtime with multi-tab support and a workspace sandbox, and provides LLM-powered features (translation, explanation, chat, autocomplete) through a pluggable agent system with 11 built-in agents. It covers stakeholder needs, functional requirements for shell operations, AI integration, tab management, and readline editing, non-functional requirements for security and performance, and traceability from stakeholder goals to implementation modules.

**Version**: 1.0
**Date**: 2026-02-10
**Standard**: ISO/IEC/IEEE 29148:2018

---

## 1. Introduction

### 1.1 Purpose

This SRS defines the stakeholder, system, and software requirements for **swebash**, a WASM-based Unix-like shell with AI-powered assistance. The system provides a traditional command-line shell with built-in commands, external command execution, a workspace sandbox, multi-tab sessions, persistent history, readline editing, and integrated LLM features including natural language to command translation, command explanation, conversational chat, and autocomplete suggestions — all orchestrated through a configurable multi-agent system with optional RAG (Retrieval-Augmented Generation) document context.

### 1.2 Scope

swebash is a multi-crate Rust workspace comprising five crates:

- **engine** — WASM shell engine (no_std, `wasm32-unknown-unknown` target) with built-in commands and command parsing
- **host** — Native binary providing the REPL, WASM runtime, host imports, tab management, sandbox enforcement, and AI command interception
- **ai** (swebash-ai) — LLM integration following the SEA pattern, with agents, tools, RAG, and streaming support
- **readline** (swebash-readline) — Terminal line editing with completion, highlighting, hints, and history
- **test** (swebash-test) — Shared test framework with mocks, fixtures, and assertions

swebash depends on the **rustratify** framework for agent infrastructure (`agent-controller`, `chat-engine`, `llm-provider`, `tool`, `react` crates).

swebash does **not**:

- Provide a POSIX-compliant shell (it implements a Unix-like subset)
- Run on bare metal (requires a host OS with a terminal)
- Bundle an LLM model (requires external API keys for AI features)

### 1.3 Definitions and Acronyms

| Term | Definition |
|------|-----------|
| **WASM** | WebAssembly — portable binary format used for the shell engine |
| **SEA** | Stratified Encapsulation Architecture — layered module pattern (SPI/API/Core) |
| **Host import** | A function provided by the native host runtime to the WASM engine via Wasmtime's linker |
| **Sandbox** | Path-based access control layer enforced at the host import boundary |
| **Tab** | An independent shell session with its own WASM instance, CWD, and environment |
| **TabManager** | The struct that owns all open tabs, tracks the active tab, and manages tab lifecycle |
| **Agent** | A specialized AI assistant with its own system prompt, tool access, trigger keywords, and conversation memory |
| **AgentDescriptor** | A rustratify trait defining agent properties: ID, display name, system prompt, tool filter, trigger keywords |
| **ConfigAgent** | A YAML-defined agent wrapping `YamlAgentDescriptor` via composition, adding swebash-specific fields (docs, bypass, iterations) |
| **ChatEngine** | A rustratify component managing conversation history and LLM interactions for a single agent |
| **RAG** | Retrieval-Augmented Generation — technique that retrieves relevant document chunks to augment LLM context |
| **ToolFilter** | Controls which tool categories an agent can access: `All`, `Categories([...])`, or `None` |
| **ToolsConfig** | A `HashMap<String, bool>` mapping tool category names to enabled/disabled state |
| **Readline** | The terminal line-editing layer providing arrow keys, history, hints, tab completion, and syntax highlighting |
| **W3H** | WHO-WHAT-WHY-HOW documentation structure pattern |
| **MoSCoW** | Must / Should / Could / Won't prioritization scheme |
| **ReAct** | Reasoning + Acting agent execution pattern where the LLM alternates between reasoning and tool calls |
| **LLM** | Large Language Model — the AI model generating responses (OpenAI, Anthropic, or Gemini) |

### 1.4 References

| Document | Location |
|----------|----------|
| ISO/IEC/IEEE 29148:2018 | Requirements engineering standard (this document conforms to) |
| rustratify Architecture | `/mnt/c/phd-systems/swe-labs/langboot/rustratify/docs/3-design/architecture.md` |
| swebash Architecture | `../3-design/architecture.md` |
| swebash Agent Architecture | `../3-design/agent_architecture.md` |
| swebash RAG Architecture | `../3-design/rag_architecture.md` |
| ADR-001: Agent Doc Context | `../3-design/ADR-001-agent-doc-context.md` |
| swebash Development Backlog | `../4-development/backlog.md` |
| swe-compliance SRS Template | `/mnt/c/phd-systems/swe-labs/swe-compliance/doc-engine/docs/1-requirements/srs.md` |

---

## 2. Stakeholder Requirements (StRS)

### 2.1 Stakeholders

| Stakeholder | Role | Needs |
|-------------|------|-------|
| Developer | Primary user of the shell | Fast command execution, AI assistance for unfamiliar commands, tab isolation for parallel tasks |
| DevOps engineer | Uses shell for infrastructure tasks | Specialized agents (AWS, Docker, Terraform), tool execution, workspace sandbox for safety |
| AI researcher | Explores LLM integration patterns | Pluggable providers, agent customization via YAML, RAG document context |
| Shell power user | Expects rich line editing | History persistence, tab completion, syntax highlighting, multi-line editing |
| Security-conscious user | Wants controlled access | Workspace sandbox, tool confirmation prompts, per-agent tool filtering |

### 2.2 Operational Scenarios

#### OS-1: Developer daily workflow

A developer launches swebash, opens multiple tabs for different project directories, uses built-in commands for file operations, and invokes `? list all .rs files larger than 1KB` to get an AI-translated shell command with confirmation before execution.

#### OS-2: AI-assisted debugging

A developer encounters an unfamiliar command in a script and types `?? find . -name "*.log" -mtime +7 -delete` to get a detailed explanation. They then switch to the `@review` agent to analyze a code file.

#### OS-3: Multi-tab isolation

A developer opens `tab new /project-a` and `tab new /project-b`, sets different environment variables in each, and switches between them. Each tab has independent CWD, environment, and WASM state.

#### OS-4: Custom agent with documentation

A DevOps engineer generates AWS reference docs with `./sbh gen-aws-docs`, configures the `@awscli` agent in `~/.config/swebash/agents.yaml` with pre-loaded docs, and uses `ai @awscli create an S3 bucket with versioning` for accurate, version-matched guidance.

#### OS-5: Sandbox-protected operations

A user configures the workspace sandbox to restrict the shell to `~/workspace/` in read-only mode. Attempts to write outside the sandbox are blocked. The user runs `workspace rw` to temporarily enable writes for a specific task.

#### OS-6: History search across tabs

A developer runs commands across multiple tabs, then opens `tab history` to search through the shared command history. They find a complex command from an earlier session and re-execute it.

### 2.3 Stakeholder Requirements

| ID | Requirement | Source | Priority | Rationale |
|----|-------------|--------|----------|-----------|
| STK-01 | The shell shall execute built-in commands and external programs within an isolated WASM engine | Core shell need | Must | WASM isolation prevents engine bugs from affecting the host |
| STK-02 | The shell shall provide AI-powered assistance for command translation, explanation, chat, and autocomplete | Developer productivity | Must | Reduces context switching and manual lookup |
| STK-03 | The shell shall support multiple concurrent tabs with independent CWD and environment | Parallel workflow need | Must | Developers work on multiple tasks simultaneously |
| STK-04 | The shell shall enforce a configurable workspace sandbox on all filesystem operations | Security requirement | Must | Prevents accidental writes outside approved directories |
| STK-05 | The shell shall support pluggable LLM providers (OpenAI, Anthropic, Gemini) | Flexibility need | Must | Users have different API access and preferences |
| STK-06 | The shell shall allow custom AI agents defined via YAML configuration | Extensibility need | Should | Different domains need specialized agents |
| STK-07 | The shell shall provide rich line editing with completion, hints, and syntax highlighting | Power user need | Should | Matches expectations from modern shell environments |
| STK-08 | The shell shall persist command history across sessions and share it across tabs | Usability need | Must | Users expect history persistence and searchability |
| STK-09 | The shell shall support agent document context via preload and RAG strategies | AI quality need | Should | Domain-specific docs improve agent response quality |
| STK-10 | The shell shall run on Linux and WSL2 platforms | Platform requirement | Must | Primary development environments |

---

## 3. System Requirements (SyRS)

### 3.1 System Context

```
User Terminal
     │
     ▼
swebash (host binary)
  ├── TabManager → Tab[n] → WasmSession → engine.wasm
  ├── SandboxPolicy → filesystem access control
  ├── swebash-ai → LLM Provider (OpenAI / Anthropic / Gemini)
  ├── swebash-readline → terminal line editing
  └── ~/.config/swebash/ → config files, agent YAML, docs
```

### 3.2 System Functions

| ID | Function | Description |
|----|----------|-------------|
| SYS-01 | Command execution | Parse and execute shell commands via WASM engine or external process spawning |
| SYS-02 | Filesystem operations | Provide sandboxed read/write/list/stat operations to the WASM engine |
| SYS-03 | Tab management | Create, close, switch, and rename independent shell sessions |
| SYS-04 | AI integration | Route AI commands to the appropriate agent and LLM provider |
| SYS-05 | Agent management | Load, register, switch, and auto-detect agents from YAML configuration |
| SYS-06 | Line editing | Provide readline with completion, history, hints, highlighting, and multi-line support |
| SYS-07 | Sandbox enforcement | Intercept all host imports and enforce path-based access control |
| SYS-08 | Configuration | Load settings from environment variables, TOML config files, and YAML agent definitions |

### 3.3 System Constraints

- **Language**: Rust (2021 edition)
- **Engine target**: `wasm32-unknown-unknown` (no_std, no networking)
- **Host target**: Native binary (Linux, WSL2)
- **Async runtime**: tokio (for AI features; WASM calls remain synchronous)
- **WASM runtime**: Wasmtime 29
- **External dependency**: rustratify framework (local cargo registry)

### 3.4 Assumptions and Dependencies

- A terminal emulator supporting ANSI escape codes is available
- LLM API keys are provided for AI features (graceful degradation when absent)
- rustratify crates are published to a local cargo registry
- External dependencies: wasmtime, tokio, crossterm, serde, toml, serde_yaml, anyhow, dirs, dotenvy, tracing

---

## 4. Software Requirements (SRS)

### Requirement Attributes

Each requirement includes:

| Attribute | Description |
|-----------|-------------|
| **ID** | Unique identifier (FR-nnn for functional, NFR-nnn for non-functional) |
| **Priority** | Must / Should / Could / Won't (MoSCoW) |
| **State** | Proposed / Approved / Implemented / Verified |
| **Verification** | Test / Inspection / Analysis / Demonstration |
| **Traces to** | Stakeholder requirement (STK-nn), architecture component |
| **Acceptance criteria** | Condition(s) that prove the requirement is met |

### 4.1 Shell Engine

#### FR-100: Built-in commands

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01 → `engine/src/dispatch.rs` |
| **Acceptance** | All 15 built-in commands execute correctly within the WASM engine and produce expected output |

The engine shall support 15 built-in commands:

| Command | Description |
|---------|-------------|
| `echo` | Output text to stdout |
| `pwd` | Print the virtual working directory |
| `cd` | Change the virtual working directory |
| `ls` | List directory contents |
| `cat` | Concatenate and print file contents |
| `mkdir` | Create directories |
| `rm` | Remove files and directories |
| `cp` | Copy files and directories |
| `mv` | Move or rename files |
| `touch` | Create or update file timestamps |
| `env` | Display environment variables |
| `export` | Set environment variables |
| `head` | Print first N lines of a file |
| `tail` | Print last N lines of a file |
| `workspace` | Manage workspace sandbox (delegated to host) |

#### FR-101: Command parsing

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01 → `engine/src/parser.rs` |
| **Acceptance** | The parser correctly handles: simple commands, quoted strings, pipes, redirects, environment variable expansion, and whitespace |

The engine shall parse command input into structured tokens, handling quoted strings (single and double), variable expansion (`$VAR`), and argument splitting.

#### FR-102: External command execution

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01 → `engine/src/dispatch.rs`, `host/src/spi/imports/process.rs` |
| **Acceptance** | Unrecognized commands are spawned as external processes via `host_spawn`; the exit code is returned; virtual CWD and env are inherited |

When a command is not a built-in, the engine shall invoke `host_spawn` with the command and arguments. The host spawns the process with the tab's virtual CWD and environment overlays.

#### FR-103: WASM isolation

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-01 → `engine/Cargo.toml` |
| **Acceptance** | The engine crate compiles to `wasm32-unknown-unknown` with `crate-type = ["cdylib"]`; it has no `std`, no networking, and no direct OS access |

The shell engine shall compile to WebAssembly with no standard library, no filesystem access, and no networking. All I/O is mediated through host imports.

### 4.2 Host Runtime

#### FR-200: WASM runtime management

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01 → `host/src/spi/runtime.rs` |
| **Acceptance** | The host loads `engine.wasm`, instantiates it with Wasmtime, links all host imports, and calls `shell_init()` followed by `shell_eval()` for each command |

The host shall manage the Wasmtime runtime: compile the WASM module, create stores with `HostState`, link host import functions, and provide `shell_init` / `shell_eval` / `get_input_buf` / `get_input_buf_len` exports.

#### FR-201: Host imports — I/O

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01 → `host/src/spi/imports/io.rs` |
| **Acceptance** | `host_write(ptr, len)` writes to stdout; `host_write_err(ptr, len)` writes to stderr |

#### FR-202: Host imports — Filesystem

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01, STK-04 → `host/src/spi/imports/fs.rs` |
| **Acceptance** | All 5 filesystem imports (`host_read_file`, `host_write_file`, `host_list_dir`, `host_stat_path`, `host_path_exists`) enforce sandbox rules before accessing the OS |

#### FR-203: Host imports — Environment

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01, STK-03 → `host/src/spi/imports/env.rs` |
| **Acceptance** | `host_get_env` checks the per-tab virtual overlay first, then falls back to the process environment; `host_set_env` modifies only the virtual overlay |

Each tab shall maintain its own environment variable overlay. Environment reads check the overlay first, then fall back to the process environment. Environment writes modify only the overlay.

#### FR-204: Host imports — Process

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01, STK-04 → `host/src/spi/imports/process.rs` |
| **Acceptance** | `host_spawn` validates CWD against sandbox, sets virtual CWD and env on the child process, and returns the exit code |

External process spawning shall respect the tab's virtual CWD and environment overlay. On Windows, commands are wrapped with `cmd /C`; on Unix, they are spawned directly.

#### FR-205: Host imports — Workspace

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-04 → `host/src/spi/imports/workspace.rs` |
| **Acceptance** | `host_workspace(cmd)` processes subcommands (`status`, `rw`, `ro`, `allow`, `enable`, `disable`) and returns the response length |

### 4.3 Workspace Sandbox

#### FR-300: Path-based access control

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-04 → `host/src/spi/sandbox.rs` |
| **Acceptance** | Every filesystem host import calls `check_path(policy, path, Read|Write)` before accessing the OS; denied operations return an error message to stderr |

The sandbox shall intercept all filesystem operations at the host import boundary and enforce `ReadOnly` or `ReadWrite` access based on ordered path rules.

#### FR-301: Workspace configuration

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-04 → `host/src/spi/config.rs` |
| **Acceptance** | Config loaded from `~/.config/swebash/config.toml` with `[workspace]` section; `SWEBASH_WORKSPACE` env var overrides config root; default workspace is `~/.local/share/swebash/workspace/` in read-only mode (XDG-compliant) |

Configuration precedence: `SWEBASH_WORKSPACE` env var > `config.toml` > `~/workspace/` default. When the env var is set, the workspace defaults to read-write (backward compatible).

#### FR-302: Runtime sandbox commands

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-04 → `engine/src/builtins/`, `host/src/spi/imports/workspace.rs` |
| **Acceptance** | All 6 workspace subcommands execute correctly: `status`, `rw`, `ro`, `allow PATH [ro|rw]`, `enable`, `disable` |

The `workspace` built-in command shall support runtime sandbox modification:

| Subcommand | Effect |
|------------|--------|
| `workspace` / `workspace status` | Display sandbox status and allowed paths |
| `workspace rw` | Set workspace to read-write mode |
| `workspace ro` | Set workspace to read-only mode |
| `workspace allow PATH [ro\|rw]` | Add an allowed path with specified access mode |
| `workspace enable` | Enable sandbox enforcement |
| `workspace disable` | Disable sandbox enforcement |

#### FR-303: Sandbox path resolution

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-04 → `host/src/spi/sandbox.rs` |
| **Acceptance** | Relative paths are resolved against the tab's virtual CWD; `~` is expanded to the home directory; symlinks are resolved before checking |

### 4.4 Tab System

#### FR-400: Multi-tab shell sessions

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-03 → `host/src/spi/tab.rs` |
| **Acceptance** | Users can create, close, switch, and rename tabs; each shell tab is backed by its own WASM instance with independent CWD, environment, and memory |

#### FR-401: Tab types

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-03 → `host/src/spi/tab.rs` |
| **Acceptance** | Three tab types exist: Shell (WASM-backed), AI (dedicated chat), and HistoryView (searchable history); each type has correct behavior |

| Type | `TabInner` Variant | Engine | Purpose |
|------|--------------------|--------|---------|
| Shell | `Shell(WasmSession)` | Own WASM instance | Regular shell commands |
| AI | `Ai { fallback_cwd }` | None | Dedicated AI chat tab |
| History | `HistoryView { fallback_cwd }` | None | Searchable history browser |

#### FR-402: Tab commands

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-03 → `host/src/main.rs` |
| **Acceptance** | All 7 tab subcommands execute correctly |

| Subcommand | Description |
|------------|-------------|
| `tab` / `tab list` | List all tabs with labels |
| `tab new [PATH]` | Create a new shell tab (optionally in PATH) |
| `tab close` | Close the active tab |
| `tab N` | Switch to tab N (1-based) |
| `tab rename NAME` | Set a custom label for the active tab |
| `tab ai [AGENT]` | Open a dedicated AI tab with the specified agent |
| `tab history` | Open a searchable history browser tab |

#### FR-403: CWD and environment isolation

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-03 → `host/src/spi/tab.rs`, `host/src/spi/imports/env.rs` |
| **Acceptance** | `cd` in tab 1 does not affect tab 2's CWD; `export FOO=bar` in tab 2 is not visible in tab 1; external processes inherit the correct tab's virtual CWD and env |

Each shell tab shall have its own Wasmtime `Store<HostState>` with independent virtual CWD and environment overlay. Mode tabs (AI, History) store only a `fallback_cwd`.

#### FR-404: Tab bar UI

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Demonstration |
| **Traces to** | STK-03 → `host/src/spi/tab_bar.rs` |
| **Acceptance** | Tab bar renders at terminal row 0 when 2+ tabs are open; active tab is bold white, inactive is grey; bar auto-hides when 1 tab remains; labels truncate with `...` when exceeding terminal width |

Tab bar format: `[N:icon:label]` where icon is `>` (Shell), `AI` (AI), or `H` (History).

#### FR-405: Tab keyboard shortcuts

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Demonstration |
| **Traces to** | STK-03, STK-07 → `host/src/main.rs` |
| **Acceptance** | `Ctrl+T` creates a new tab; `Ctrl+PageDown`/`Ctrl+PageUp` switch next/prev; `Alt+1`–`Alt+9` switch to tab N |

#### FR-406: Tab exit behavior

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-03 → `host/src/spi/tab.rs`, `host/src/main.rs` |
| **Acceptance** | `exit` in a non-last tab closes only that tab and switches to the nearest remaining tab; `exit` in the last tab exits the shell cleanly |

#### FR-407: Shared history across tabs

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-03, STK-08 → `host/src/spi/tab.rs` |
| **Acceptance** | Commands entered in any tab are visible in `tab history` and in history search from any other tab; history is stored via `Arc<Mutex<History>>` |

### 4.5 Readline

#### FR-500: Persistent command history

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-08 → `readline/src/core/history.rs` |
| **Acceptance** | Commands saved to `~/.local/state/swebash/history` (XDG-compliant); max 1000 entries with automatic rotation; ignores empty lines, duplicates, and space-prefixed commands; persists across sessions via Drop trait; auto-migrates legacy `~/.swebash_history` |

#### FR-501: Tab completion

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-07 → `readline/src/core/completer.rs` |
| **Acceptance** | Double-space or tab trigger completes built-in commands and file/directory paths; directories show `/` suffix |

#### FR-502: Syntax highlighting

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Demonstration |
| **Traces to** | STK-07 → `readline/src/core/highlighter.rs` |
| **Acceptance** | Built-in commands shown in green; external commands in blue; unknown commands in red; strings in yellow; paths in cyan; operators in magenta |

#### FR-503: History hints

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Demonstration |
| **Traces to** | STK-07 → `readline/src/core/hinter.rs` |
| **Acceptance** | Grey hint text appears below the prompt based on history prefix matching |

#### FR-504: Multi-line editing

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-07 → `readline/src/core/validator.rs` |
| **Acceptance** | Incomplete commands (trailing `\`, unclosed quotes/brackets) trigger a continuation prompt (`...>`) |

#### FR-505: Readline configuration

| Attribute | Value |
|-----------|-------|
| **Priority** | Could |
| **State** | Implemented |
| **Verification** | Demonstration |
| **Traces to** | STK-07 → `readline/src/` |
| **Acceptance** | TOML config file at `~/.swebashrc` controls edit mode, history size, completion, highlighting, hints, and color scheme |

### 4.6 AI Service

#### FR-600: AI command routing

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/api/commands.rs`, `host/src/ai/commands.rs` |
| **Acceptance** | All AI command triggers are recognized and routed to the correct handler |

The host shall intercept AI commands before passing input to the WASM engine:

| Trigger | Command | Action |
|---------|---------|--------|
| `ai ask TEXT` / `? TEXT` | Ask | Translate natural language to shell command |
| `ai explain CMD` / `?? CMD` | Explain | Explain a shell command |
| `ai chat TEXT` | Chat | Conversational assistant |
| `ai suggest` | Suggest | Context-aware autocomplete |
| `ai @AGENT TEXT` | One-shot | Send message to specific agent |
| `ai @AGENT` | Switch | Enter AI mode with agent |
| `@AGENT` | Shorthand switch | Switch agent (AI or shell mode) |
| `ai agents` | List | List available agents |
| `ai status` | Status | Show AI configuration |
| `ai history` | History | Show chat history |
| `ai clear` | Clear | Clear chat history |
| `ai` | Enter mode | Enter AI mode with default agent |

#### FR-601: Natural language translation

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/translate.rs` |
| **Acceptance** | `? list all .rs files` returns a valid shell command; user is prompted `[Y/n/e]` before execution |

The AI shall translate natural language descriptions into shell commands, present the command for confirmation, and execute on approval.

#### FR-602: Command explanation

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/explain.rs` |
| **Acceptance** | `?? find . -name "*.log" -delete` returns a structured explanation of each flag and argument |

#### FR-603: Conversational chat

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/chat.rs` |
| **Acceptance** | `ai chat` maintains conversation history; responses are contextual; history is configurable via `SWEBASH_AI_HISTORY_SIZE` (default 20) |

#### FR-604: Autocomplete suggestions

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/complete.rs` |
| **Acceptance** | `ai suggest` uses CWD listing and recent commands as context to generate completion suggestions |

#### FR-605: AI mode

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `host/src/ai/` |
| **Acceptance** | `ai` enters AI mode with cyan `[AI:agent]` prompt; smart detection routes input to ask/explain/chat; `exit` returns to shell mode |

AI mode shall provide smart intent detection:
- Command patterns (flags, pipes, redirects) → explain
- Action requests (find, list, show) → translate to command
- Questions and conversation → chat

#### FR-606: Graceful degradation without AI

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-05 → `ai/src/lib.rs` |
| **Acceptance** | When no API key is configured, `create_ai_service()` returns `None`; AI commands print `AI is not configured` and return to the prompt; shell functionality is unaffected |

### 4.7 Agent System

#### FR-700: Built-in agents

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02, STK-06 → `ai/src/core/agents/default_agents.yaml`, `ai/src/core/agents/builtins.rs` |
| **Acceptance** | `ai agents` lists all 11 built-in agents with correct properties |

The system shall include 11 built-in agents:

| ID | Name | Tools | Keywords |
|----|------|-------|----------|
| `shell` | Shell Assistant (default) | fs, exec, web | — |
| `review` | Code Reviewer | fs | — |
| `devops` | DevOps Assistant | fs, exec, web | docker, k8s, terraform, deploy, pipeline |
| `git` | Git Assistant | fs, exec | git, commit, branch, merge, rebase |
| `web` | Web Research | web | search, web, lookup, google |
| `security` | Security Analyst | fs, exec | scan, vulnerability, audit, CVE |
| `explain` | Command Explainer | — | — |
| `seaaudit` | SEA Architecture Auditor | fs, exec | sea, audit, architecture, layering |
| `rscagent` | RustScript Assistant | fs, exec | rustscript, rsc, rsx, component |
| `clitester` | CLI Manual Tester | fs, exec | clitester, cli test, shell test |
| `apitester` | API/AI Feature Tester | fs, exec | apitester, api test, ai test |

#### FR-701: YAML agent configuration

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-06 → `ai/src/core/agents/config.rs` |
| **Acceptance** | Agents parse from YAML with defaults merging; per-agent fields override defaults for temperature, maxTokens, tools, thinkFirst, and directives |

Agent YAML schema (per-entry fields):

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique agent identifier |
| `name` | string | Yes | Display name |
| `description` | string | Yes | Human-readable description |
| `systemPrompt` | string | Yes | LLM system prompt |
| `temperature` | float | No | Override default temperature |
| `maxTokens` | u32 | No | Override default max tokens |
| `tools` | map | No | Per-category tool toggles (fs, exec, web, rag) |
| `triggerKeywords` | list | No | Keywords for auto-detection |
| `thinkFirst` | bool | No | Append "explain reasoning" to prompt |
| `directives` | list | No | Override default directives |
| `docs` | object | No | Document context configuration |
| `maxIterations` | int | No | Tool loop iteration limit |
| `bypassConfirmation` | bool | No | Skip tool execution confirmation |

#### FR-702: Agent switching

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/agents/mod.rs` |
| **Acceptance** | `@review` switches to the review agent; `ai @devops hello` sends a one-shot message to devops and restores the previous agent |

#### FR-703: Agent auto-detection

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/agents/mod.rs` |
| **Acceptance** | In AI mode, typing "git commit" auto-switches to the `git` agent; "docker ps" auto-switches to `devops`; unmatched input stays on the current agent; disabled by `SWEBASH_AI_AGENT_AUTO_DETECT=false` |

#### FR-704: User-defined agents

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-06 → `ai/src/core/agents/builtins.rs` |
| **Acceptance** | Agents defined in `~/.config/swebash/agents.yaml` or the path in `SWEBASH_AGENTS_CONFIG` are loaded, merged with built-ins (user overrides win), and accessible via `@agent` |

Multi-layer YAML loading: embedded defaults → project-local `.swebash/agents.yaml` → user config → `SWEBASH_AGENTS_CONFIG`.

#### FR-705: Agent defaults merging

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-06 → `agent-controller::yaml` (rustratify) |
| **Acceptance** | Agents that omit `temperature`, `maxTokens`, `tools`, `thinkFirst`, or `directives` inherit from the YAML `defaults` section; explicit values override defaults |

#### FR-706: ConfigAgent composition

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-06 → `ai/src/core/agents/config.rs` |
| **Acceptance** | `ConfigAgent` wraps `YamlAgentDescriptor` (from rustratify `agent-controller::yaml`) via composition; delegates `AgentDescriptor` trait methods to the base; adds swebash-specific fields (docs, bypass, iterations) |

### 4.8 Agent Tools

#### FR-800: Filesystem tools

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/tools/` |
| **Acceptance** | Agents with `tools.fs: true` can read files, list directories, and query file metadata; agents with `tools.fs: false` cannot |

#### FR-801: Execution tools

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/tools/` |
| **Acceptance** | Agents with `tools.exec: true` can execute shell commands; command timeout is controlled by `SWEBASH_AI_EXEC_TIMEOUT` (default 30s) |

#### FR-802: Web search tools

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/tools/` |
| **Acceptance** | Agents with `tools.web: true` can perform web searches; agents with `tools.web: false` cannot |

#### FR-803: Tool execution confirmation

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-04 → `ai/src/core/tools/` |
| **Acceptance** | When `SWEBASH_AI_TOOLS_CONFIRM=true` (default), tool calls require user confirmation before execution; agents with `bypassConfirmation: true` skip this |

#### FR-804: Tool iteration limits

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/tools/` |
| **Acceptance** | Tool loops are capped at the agent's `maxIterations` (default from `SWEBASH_AI_TOOLS_MAX_ITER`, default 10); exceeding the limit stops the loop |

#### FR-805: Tool result caching

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-02 → `ai/src/core/tools/` |
| **Acceptance** | When `SWEBASH_AI_TOOL_CACHE=true` (default), identical tool calls within the TTL (default 300s) return cached results; max entries default to 200 |

### 4.9 Agent Document Context

#### FR-900: Preload document strategy

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-09 → `ai/src/core/agents/config.rs` |
| **Acceptance** | Agents with `docs.strategy: preload` (default) have their `docs.sources` files loaded at engine creation time and injected into the system prompt within a `<documentation>` block, respecting the `docs.budget` token limit |

#### FR-901: RAG document strategy

| Attribute | Value |
|-----------|-------|
| **Priority** | Could |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-09 → `ai/src/core/rag/` |
| **Acceptance** | Agents with `docs.strategy: rag` have their documents indexed into a vector store at engine creation; relevant chunks are retrieved per query via a `rag_search` tool; falls back to `preload` when RAG is unavailable |

#### FR-902: RAG vector store backends

| Attribute | Value |
|-----------|-------|
| **Priority** | Could |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-09 → `ai/src/core/rag/`, `ai/Cargo.toml` |
| **Acceptance** | RAG supports 4 backends controlled by `SWEBASH_AI_RAG_STORE`: `memory` (default), `file`, `sqlite`, `swevecdb`; each is feature-gated |

### 4.10 LLM Providers

#### FR-1000: Multi-provider support

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-05 → `ai/src/spi/chat_provider.rs` |
| **Acceptance** | Setting `LLM_PROVIDER=anthropic` uses the Anthropic API; `openai` uses OpenAI; `gemini` uses Google Gemini; API keys are read from `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY` respectively |

#### FR-1001: Streaming responses

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Demonstration |
| **Traces to** | STK-02 → `ai/src/core/chat.rs` |
| **Acceptance** | Chat responses stream token-by-token to the terminal as they are received from the LLM |

### 4.11 CLI Launcher

#### FR-1100: sbh launcher script

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Demonstration |
| **Traces to** | STK-01 → `sbh` |
| **Acceptance** | All 5 sbh subcommands work correctly |

| Subcommand | Description |
|------------|-------------|
| `./sbh setup` | One-time environment setup |
| `./sbh build [--debug]` | Build engine WASM and host binary |
| `./sbh run [--release\|--debug]` | Build and launch swebash |
| `./sbh test [suite]` | Run tests (engine, host, readline, ai, scripts, all) |
| `./sbh gen-aws-docs` | Generate AWS reference docs from live CLI help |

### 4.12 Configuration

#### FR-1200: Environment variable configuration

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-05 → `ai/src/spi/config.rs` |
| **Acceptance** | All environment variables listed below are read and applied |

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_AI_ENABLED` | `true` | Enable/disable AI features |
| `LLM_PROVIDER` | `openai` | LLM provider: openai, anthropic, gemini |
| `LLM_DEFAULT_MODEL` | provider-specific | Model ID |
| `SWEBASH_AI_HISTORY_SIZE` | `20` | Chat history message count |
| `SWEBASH_AI_DEFAULT_AGENT` | `shell` | Startup agent |
| `SWEBASH_AI_AGENT_AUTO_DETECT` | `true` | Auto-detect agent from keywords |
| `SWEBASH_AI_TOOLS_FS` | `true` | Enable filesystem tools |
| `SWEBASH_AI_TOOLS_EXEC` | `true` | Enable command execution tools |
| `SWEBASH_AI_TOOLS_WEB` | `true` | Enable web search tools |
| `SWEBASH_AI_TOOLS_CONFIRM` | `true` | Require tool execution confirmation |
| `SWEBASH_AI_TOOLS_MAX_ITER` | `10` | Max tool loop iterations |
| `SWEBASH_AI_FS_MAX_SIZE` | `1048576` | Max file read size (bytes) |
| `SWEBASH_AI_EXEC_TIMEOUT` | `30` | Command timeout (seconds) |
| `SWEBASH_AI_TOOL_CACHE` | `true` | Enable tool result caching |
| `SWEBASH_AI_TOOL_CACHE_TTL` | `300` | Cache TTL (seconds) |
| `SWEBASH_AI_TOOL_CACHE_MAX` | `200` | Max cache entries |
| `SWEBASH_AI_TOOLS_RAG` | `false` | Enable RAG tools |
| `SWEBASH_AI_RAG_STORE` | `memory` | RAG store: memory, file, sqlite, swevecdb |
| `SWEBASH_AI_RAG_CHUNK_SIZE` | `2000` | RAG chunk size (chars) |
| `SWEBASH_AI_RAG_CHUNK_OVERLAP` | `200` | RAG chunk overlap (chars) |
| `SWEBASH_AI_LOG_DIR` | — | Optional JSON log directory |
| `SWEBASH_AI_DOCS_BASE_DIR` | `~/.config/swebash` | Base directory for agent doc resolution |
| `SWEBASH_WORKSPACE` | — | Override workspace root |
| `SWEBASH_AGENTS_CONFIG` | — | Explicit path to agents YAML |
| `OPENAI_API_KEY` | — | OpenAI API key |
| `ANTHROPIC_API_KEY` | — | Anthropic API key |
| `GEMINI_API_KEY` | — | Google Gemini API key |

#### FR-1201: Configuration files

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-04, STK-06, STK-07 → multiple |
| **Acceptance** | All configuration files are loaded from the correct paths |

| File | Format | Purpose |
|------|--------|---------|
| `~/.config/swebash/config.toml` | TOML | Workspace sandbox configuration |
| `~/.swebashrc` | TOML | Readline configuration |
| `~/.local/state/swebash/history` | Text | Persistent command history (XDG-compliant) |
| `~/.config/swebash/agents.yaml` | YAML | User-defined agent overrides |
| `~/.config/swebash/docs/` | Directory | Agent reference documentation |

---

## 5. Non-Functional Requirements

### 5.1 Architecture

#### NFR-100: WASM engine isolation

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-01, STK-04 → `engine/Cargo.toml` |
| **Acceptance** | Engine compiles to `wasm32-unknown-unknown` with no `std`, no networking, no direct filesystem access; all I/O is mediated through host imports |

#### NFR-101: SEA pattern in AI crate

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-02 → `ai/src/` |
| **Acceptance** | AI crate follows SEA layering: L1 Common (`api/types.rs`, `api/error.rs`), L2 SPI (`spi/`), L3 API (`api/`), L4 Core (`core/`), L5 Facade (`lib.rs`) |

#### NFR-102: Composition over inheritance

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-06 → `ai/src/core/agents/config.rs` |
| **Acceptance** | `ConfigAgent` wraps `YamlAgentDescriptor` via composition, not inheritance; trait delegation is explicit |

### 5.2 Security

#### NFR-200: Sandbox at host import boundary

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-04 → `host/src/spi/sandbox.rs` |
| **Acceptance** | All 10 filesystem host imports check `SandboxPolicy` before accessing the OS; the WASM engine cannot bypass sandbox checks because it has no direct OS access |

#### NFR-201: Tool confirmation by default

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-04 → `ai/src/core/tools/` |
| **Acceptance** | `SWEBASH_AI_TOOLS_CONFIRM` defaults to `true`; tool calls prompt the user before execution |

#### NFR-202: Per-agent tool filtering

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-04, STK-06 → `ai/src/core/agents/config.rs` |
| **Acceptance** | Agents only access tools in their `ToolFilter`; the review agent (fs-only) cannot execute commands |

### 5.3 Performance

#### NFR-300: Async AI, synchronous WASM

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-01 → `host/src/main.rs` |
| **Acceptance** | `#[tokio::main]` provides the async runtime for AI calls; WASM `shell_eval()` calls remain synchronous within the event loop |

#### NFR-301: Optimized release build

| Attribute | Value |
|-----------|-------|
| **Priority** | Should |
| **State** | Implemented |
| **Verification** | Inspection |
| **Traces to** | STK-01 → root `Cargo.toml` |
| **Acceptance** | Release profile uses `opt-level = 3`, `lto = true`, `strip = true` for minimal binary size |

### 5.4 Reliability

#### NFR-400: Errors never crash the shell

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-01, STK-02 → `host/src/main.rs`, `ai/src/lib.rs` |
| **Acceptance** | AI failures, WASM traps, I/O errors, and sandbox denials print error messages and return to the prompt; the shell never panics on user input |

#### NFR-401: Graceful AI degradation

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Test |
| **Traces to** | STK-05 → `ai/src/lib.rs` |
| **Acceptance** | When AI is not configured (no API key), all shell features work normally; AI commands print a clear message |

### 5.5 Portability

#### NFR-500: Platform support

| Attribute | Value |
|-----------|-------|
| **Priority** | Must |
| **State** | Implemented |
| **Verification** | Demonstration |
| **Traces to** | STK-10 |
| **Acceptance** | `cargo build` succeeds on Linux and WSL2; `./sbh build` produces a working binary |

---

## 6. External Interface Requirements

### 6.1 Terminal Interface

| Direction | Data | Format |
|-----------|------|--------|
| Input | User keystrokes | Raw terminal mode via crossterm |
| Output | Shell prompt, command output, AI responses | ANSI-colored text to stdout/stderr |
| Output | Tab bar | ANSI escape sequences at terminal row 0 |

### 6.2 WASM Interface

| Direction | Data | Format |
|-----------|------|--------|
| Host → Engine | Command string | UTF-8 bytes written to WASM linear memory |
| Engine → Host | Host import calls | Wasmtime-linked function calls with pointer/length pairs |
| Engine → Host | Output text | `host_write(ptr, len)` / `host_write_err(ptr, len)` |

### 6.3 LLM Provider Interface

| Direction | Data | Format |
|-----------|------|--------|
| Output | Chat completion request | Provider-specific HTTP API (OpenAI, Anthropic, Gemini) |
| Input | Chat completion response | JSON (streaming or complete) |
| Config | API key | Environment variable (`OPENAI_API_KEY`, etc.) |

### 6.4 File System Interface

| Direction | Data | Format |
|-----------|------|--------|
| Input | `~/.config/swebash/config.toml` | TOML workspace configuration |
| Input | `~/.swebashrc` | TOML readline configuration |
| Input | `~/.config/swebash/agents.yaml` | YAML agent definitions |
| Input/Output | `~/.local/state/swebash/history` | Newline-delimited command history (XDG-compliant) |
| Input | `~/.config/swebash/docs/` | Markdown reference documents for agents |

---

## 7. Risk Analysis

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| LLM API key missing or expired | Medium | High | Graceful degradation (NFR-401); clear error messages; shell functions fully without AI |
| WASM engine trap (unreachable, OOM) | High | Low | Trap caught by Wasmtime; error printed; shell continues in current tab |
| Sandbox bypass via symlink or path traversal | High | Low | Paths canonicalized before checking; symlinks resolved (FR-303) |
| Agent YAML schema changes break user configs | Medium | Low | Version field in YAML; swebash-specific fields use `#[serde(flatten)]` extension |
| rustratify crate version incompatibility | High | Medium | Local cargo registry with explicit version pinning; workspace-level dep management |
| Tab resource exhaustion (many WASM instances) | Medium | Low | Each WASM instance is ~2MB; practical limit of ~50 tabs before memory pressure |
| LLM provider rate limiting or timeout | Medium | Medium | Configurable timeout (`SWEBASH_AI_EXEC_TIMEOUT`); tool result caching (FR-805) reduces API calls |

---

## Appendix A: Traceability Matrix

### Stakeholder → System

| STK | SYS |
|-----|-----|
| STK-01 | SYS-01, SYS-02, SYS-07 |
| STK-02 | SYS-04, SYS-05 |
| STK-03 | SYS-03 |
| STK-04 | SYS-07, SYS-08 |
| STK-05 | SYS-04, SYS-08 |
| STK-06 | SYS-05, SYS-08 |
| STK-07 | SYS-06 |
| STK-08 | SYS-03, SYS-06 |
| STK-09 | SYS-04, SYS-05 |
| STK-10 | SYS-01 |

### Stakeholder → Software

| STK | FR / NFR |
|-----|----------|
| STK-01 | FR-100, FR-101, FR-102, FR-103, FR-200-205, NFR-100 |
| STK-02 | FR-600-606, FR-700-706, FR-800-805, FR-1000-1001, NFR-101 |
| STK-03 | FR-400-407 |
| STK-04 | FR-300-303, FR-803, NFR-200, NFR-201, NFR-202 |
| STK-05 | FR-606, FR-1000, NFR-401 |
| STK-06 | FR-701, FR-704, FR-705, FR-706 |
| STK-07 | FR-500-505, NFR-102 |
| STK-08 | FR-500, FR-407 |
| STK-09 | FR-900-902 |
| STK-10 | NFR-500 |

### Software → Architecture

| FR / NFR | Architecture Component |
|----------|----------------------|
| FR-100-103 | `engine/src/dispatch.rs`, `engine/src/parser.rs` |
| FR-200-205 | `host/src/spi/runtime.rs`, `host/src/spi/imports/` |
| FR-300-303 | `host/src/spi/sandbox.rs`, `host/src/spi/config.rs`, `host/src/spi/imports/workspace.rs` |
| FR-400-407 | `host/src/spi/tab.rs`, `host/src/spi/tab_bar.rs`, `host/src/main.rs` |
| FR-500-505 | `readline/src/core/` |
| FR-600-606 | `ai/src/api/commands.rs`, `ai/src/core/chat.rs`, `ai/src/core/explain.rs`, `ai/src/core/translate.rs`, `ai/src/core/complete.rs`, `host/src/ai/` |
| FR-700-706 | `ai/src/core/agents/config.rs`, `ai/src/core/agents/builtins.rs`, `ai/src/core/agents/mod.rs` |
| FR-800-805 | `ai/src/core/tools/` |
| FR-900-902 | `ai/src/core/rag/` |
| FR-1000-1001 | `ai/src/spi/chat_provider.rs` |
| FR-1100 | `sbh`, `bin/` |
| FR-1200-1201 | `ai/src/spi/config.rs`, `host/src/spi/config.rs` |
| NFR-100 | `engine/Cargo.toml` |
| NFR-101-102 | `ai/src/` module structure |
| NFR-200-202 | `host/src/spi/sandbox.rs`, `ai/src/core/tools/` |
| NFR-300-301 | `host/src/main.rs`, root `Cargo.toml` |
| NFR-400-401 | `host/src/main.rs`, `ai/src/lib.rs` |
| NFR-500 | Build system |

---

## Appendix B: Test Coverage

| Suite | Location | Count |
|-------|----------|-------|
| Engine unit tests | `features/shell/engine/src/` | 20 |
| Readline unit tests | `features/shell/readline/src/` | 54 |
| Host integration | `features/shell/host/tests/integration.rs` | 60 |
| Host readline tests | `features/shell/host/tests/readline_tests.rs` | 19 |
| AI unit tests | `features/ai/src/` | 123 |
| AI integration | `features/ai/tests/integration.rs` | 189 |
| **Total automated** | | **465** |

| Manual test suite | Location | Scenarios |
|-------------------|----------|-----------|
| Shell tests | `docs/5-testing/manual_shell_tests.md` | 32 |
| Tab tests | `docs/5-testing/manual_tab_tests.md` | 68 |
| AI tests | `docs/5-testing/manual_ai_tests.md` | 97 |
| RAG tests | `docs/5-testing/manual_rag_tests.md` | 53 |
| sbh launcher tests | `docs/5-testing/manual_sbh_tests.md` | 42 |
| **Total manual** | | **292** |
