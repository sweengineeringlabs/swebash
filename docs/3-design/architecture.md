# swebash Architecture

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
- **host** runs natively, provides the REPL, filesystem, I/O, and process imports to the WASM engine. It also intercepts AI commands before they reach the engine.
- **ai** (swebash-ai) provides all LLM functionality, isolated behind traits.

## Data Flow

```
User Input
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
     WASM memory write → shell_eval() → engine dispatch
       ↓
     Host imports (fs, io, env, process) → stdout/stderr
```

## SEA Layers (ai/ crate)

The ai crate follows the SEA (Software Engineering Architecture) pattern:

| Layer | Module | Purpose |
|-------|--------|---------|
| L5 Facade | `lib.rs` | Re-exports, `create_ai_service()` factory |
| L4 Core | `core/` | `DefaultAiService`, feature modules |
| L3 API | `api/` | `AiService` trait (consumer interface) |
| L2 SPI | `spi/` | `AiClient` trait (provider plugin point) |
| L1 Common | `api/types.rs`, `api/error.rs` | Shared types and errors |

## Key Design Decisions

1. **Host-side only**: AI runs in the native host, not in WASM. The engine is no_std with no networking.
2. **tokio + wasmtime coexistence**: `#[tokio::main]` provides the async runtime. WASM calls remain synchronous.
3. **Errors never crash the shell**: `create_ai_service()` returns `Option`. AI failures print an error and return to the prompt.
4. **Single isolation file**: Only `spi/llm_provider.rs` imports from `llm-provider`. Everything else uses `AiClient`.
