# swebash Architecture

> **TLDR:** Three-crate architecture (engine + host + ai) with WASM isolation and SEA-layered AI integration.

**Audience**: Developers, architects

## Table of Contents

- [Overview](#overview)
- [Crate Relationships](#crate-relationships)
- [Data Flow](#data-flow)
- [SEA Layers (ai/ crate)](#sea-layers-ai-crate)
- [Tab System](#tab-system)
- [Key Design Decisions](#key-design-decisions)


## Overview

swebash is a WASM-based Unix-like shell with three crates:

```
swebash/
  engine/    WASM shell engine (no_std, wasm32 target)
  host/      Native REPL, WASM runtime, host imports, AI command interception
  ai/        swebash-ai: LLM integration (SEA pattern)
```

## Crate Relationships

```
host (native binary)
  ├── wasmtime → engine.wasm (shell logic, builtins, dispatch)
  └── swebash-ai (AI features)
        └── llm-provider (cross-workspace path dep)
```

- **engine** compiles to `wasm32-unknown-unknown`. It has no networking, no std, and no knowledge of AI.
- **host** runs natively, provides the REPL, tab management, filesystem, I/O, and process imports to the WASM engine. It also intercepts AI commands before they reach the engine.
- **ai** (swebash-ai) provides all LLM functionality, isolated behind traits.

## Data Flow

```
User Input
  │
  ├─ Tab command? ("tab new", "tab 2", "tab close", ...)
  │    ↓
  │  parse_tab_command() → handle_tab_command()
  │    ↓
  │  TabManager → create/close/switch tabs
  │
  ├─ AI command? ("ai ask ...", "? ...", "?? ...")
  │    ↓
  │  host/src/ai/commands.rs → parse_ai_command()
  │    ↓
  │  host/src/ai/mod.rs → handle_ai_command()
  │    ↓
  │  swebash-ai::AiService → LLM provider → response
  │    ↓
  │  host/src/ai/output.rs → formatted output
  │
  └─ Regular command? ("ls", "cat", "echo ...")
       ↓
     Active tab's WasmSession → WASM memory write → shell_eval()
       ↓
     Host imports (fs, io, env, process)
       ↓
     Sandbox policy check (SandboxPolicy → check_path)
       ↓
     Allowed → OS syscall → stdout/stderr
     Denied → stderr error message, -1 return
```

### Sandbox Layer

The workspace sandbox sits between the WASM engine's host imports and the OS filesystem:

```
engine (WASM)
  ↓  host_read_file / host_write_file / host_set_cwd / ...
host imports (fs.rs, env.rs, process.rs)
  ↓  check_path(policy, path, Read|Write)
sandbox.rs → resolve_path → check_access(SandboxPolicy)
  ↓  allowed
std::fs / std::env / std::process
```

Policy is loaded at startup from `~/.config/swebash/config.toml` (with `SWEBASH_WORKSPACE` env var override) and stored in `HostState.sandbox`. The `workspace` builtin communicates with the host via `host_workspace` to modify the policy at runtime.

## SEA Layers (ai/ crate)

The ai crate follows the SEA (Software Engineering Architecture) pattern:

| Layer | Module | Purpose |
|-------|--------|---------|
| L5 Facade | `lib.rs` | Re-exports, `create_ai_service()` factory |
| L4 Core | `core/` | `DefaultAiService`, feature modules |
| L3 API | `api/` | `AiService` trait (consumer interface) |
| L2 SPI | `spi/` | `AiClient` trait (provider plugin point) |
| L1 Common | `api/types.rs`, `api/error.rs` | Shared types and errors |

## Tab System

The host manages multiple concurrent shell sessions via a `TabManager`. Each shell tab is backed by its own WASM engine instance, providing full CWD and environment isolation between tabs.

### Tab Types

| Type | `TabInner` Variant | Engine | Purpose |
|------|--------------------|--------|---------|
| Shell | `Shell(WasmSession)` | Own WASM instance | Regular shell commands |
| AI | `Ai { fallback_cwd }` | None | Dedicated AI chat tab |
| History | `HistoryView { fallback_cwd }` | None | Searchable history browser |

### Architecture

```
TabManager
  ├── tabs: Vec<Tab>         (ordered tab list)
  ├── active: usize          (index of currently focused tab)
  └── history: Arc<Mutex<History>>  (shared across all tabs)

Tab
  ├── id: TabId              (unique monotonic identifier)
  ├── inner: TabInner        (Shell | Ai | HistoryView)
  ├── label: String          (custom rename label)
  ├── multiline_buffer       (per-tab partial input)
  ├── recent_commands        (per-tab AI context)
  ├── ai_mode: bool          (shell tabs can enter AI mode)
  └── ai_agent_id: String    (active agent for this tab)
```

Shell tabs (`TabInner::Shell`) own a `WasmSession` containing the Wasmtime `Store<HostState>` and `Instance`, so each tab has its own virtual CWD, environment variables, and WASM memory. Mode tabs (AI, History) store only a `fallback_cwd` for prompt display.

### Tab Bar

When 2+ tabs are open, a tab bar renders at terminal row 0 using ANSI scroll region control (`CSI 2;H r`). Each tab shows an icon (`>` for shell, `AI` for AI, `H` for history) and either a custom label or the abbreviated CWD. The bar truncates with `...` when labels exceed terminal width.

Source: `host/src/spi/tab.rs`, `host/src/spi/tab_bar.rs`

## Key Design Decisions

1. **Host-side only**: AI runs in the native host, not in WASM. The engine is no_std with no networking.
2. **tokio + wasmtime coexistence**: `#[tokio::main]` provides the async runtime. WASM calls remain synchronous.
3. **Errors never crash the shell**: `create_ai_service()` returns `Option`. AI failures print an error and return to the prompt.
4. **Single isolation file**: Only `spi/llm_provider.rs` imports from `llm-provider`. Everything else uses `AiClient`.
5. **Sandbox at the host import layer**: Access control is enforced in the host runtime, not in the WASM engine. The engine cannot bypass sandbox checks because it has no direct OS access. See [Workspace Sandbox](workspace_sandbox.md).
6. **Tab isolation via separate WASM instances**: Each shell tab gets its own Wasmtime `Store<HostState>` and `Instance`, so CWD, environment variables, and WASM memory are fully isolated. Mode tabs (AI, History) are lightweight and share no WASM state.
