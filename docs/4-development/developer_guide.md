# swebash Developer Guide

> **TLDR:** Day-to-day development workflow, conventions, and common tasks for swebash contributors.

**Audience**: Contributors, developers

**WHAT**: Day-to-day development guide for working on swebash
**WHY**: Provides a single reference for workspace structure, workflow, conventions, and common tasks
**HOW**: Organized by topic with links to detailed documents

---

## Table of Contents

- [Workspace Structure](#workspace-structure)
- [Development Workflow](#development-workflow)
- [Key Conventions](#key-conventions)
- [Common Tasks](#common-tasks)
- [See Also](#see-also)


## Workspace Structure

```
swebash/
├── features/
│   ├── shell/
│   │   ├── engine/         WASM shell engine (no_std, wasm32 target)
│   │   ├── host/           Native REPL + WASM runtime + AI command interception
│   │   └── readline/       Line editing, history, arrow-key navigation
│   └── ai/                 LLM integration (SEA pattern, depends on rustratify)
├── bin/                    Build/run/test scripts
├── lib/                    Shared script helpers
├── docs/                   Documentation (organized by SDLC phase)
├── sbh                     Launcher (bash)
└── sbh.ps1                Launcher (PowerShell)
```

### Crate Overview

| Crate | Path | Target | Key Traits |
|-------|------|--------|------------|
| `engine` | `features/shell/engine` | `wasm32-unknown-unknown` | `no_std`, builtins, command dispatch |
| `swebash` | `features/shell/host` | Native | REPL loop, wasmtime, host imports |
| `swebash-readline` | `features/shell/readline` | Native | `Readline`, `History` |
| `swebash-ai` | `features/ai` | Native | `AiService`, `AiClient`, tool registry |

## Development Workflow

1. **Setup** — run `./sbh setup && source ~/.bashrc` (one-time). See [Setup Guide](setup_guide.md).
2. **Branch** — create from `main` using `feature/`, `fix/`, `docs/` prefixes. See [CONTRIBUTING](../../CONTRIBUTING.md).
3. **Implement** — edit the appropriate crate. See [Architecture](../3-design/architecture.md) for layer guidance.
4. **Build** — `./sbh build` (release) or `./sbh build --debug`.
5. **Test** — `./sbh test` (all) or `./sbh test <crate>` (engine, host, readline, ai).
6. **Run** — `./sbh run` to launch the shell interactively.
7. **Submit** — open a PR targeting `main`. Fill out the [PR template](../../.github/PULL_REQUEST_TEMPLATE.md).

## Key Conventions

### SEA Layers (ai crate)

The `features/ai/` crate follows the SEA (Software Engineering Architecture) layered pattern:

| Layer | Module | Purpose |
|-------|--------|---------|
| L5 Facade | `lib.rs` | Re-exports, `create_ai_service()` factory |
| L4 Core | `core/` | `DefaultAiService`, feature modules |
| L3 API | `api/` | `AiService` trait (consumer interface) |
| L2 SPI | `spi/` | `AiClient` trait (provider plugin point) |
| L1 Common | `api/types.rs`, `api/error.rs` | Shared types and errors |

New AI functionality goes in L4 Core. Public API changes go in L3 API first, then implement in L4.

### WASM Boundary

- The engine crate is `no_std` — no filesystem, no networking, no allocation beyond what the WASM runtime provides.
- All host capabilities are exposed through explicitly defined imports (`fs`, `io`, `env`, `process`).
- AI commands are intercepted in the host **before** reaching the WASM engine.

### Error Handling

- `create_ai_service()` returns `Option` — AI failures never crash the shell.
- Use `Result<T, E>` with `?` propagation in fallible functions.
- No `.unwrap()` outside of tests.
- Errors print to stderr and return to the prompt.

### Code Style

- Rust 2021 edition idioms.
- Run `cargo clippy` before submitting.
- Keep business logic in core layers; no logic in API/SPI trait definitions.

## Common Tasks

### Adding a Shell Builtin

1. Add the command handler in `features/shell/engine/src/builtins/`.
2. Register it in the builtin dispatch table.
3. Add tests in the engine crate: `./sbh test engine`.
4. Build the WASM module: `./sbh build`.

### Adding an AI Agent

1. Define the agent in YAML (see [Creating Agents](../7-operation/creating_agents.md)).
2. Set the `systemPrompt`, `tools`, and `maxIterations`.
3. Optionally add a `docs` section for pre-loaded documentation context (see [ADR-001](../3-design/ADR-001-agent-doc-context.md)).
4. Test with `./sbh test ai`.

### Adding an Agent Tool

1. Define the tool trait in `features/ai/src/spi/`.
2. Implement in `features/ai/src/core/`.
3. Register in the tool registry.
4. Add unit tests and integration tests: `./sbh test ai`.

### Adding a New LLM Provider

1. Implement `AiClient` (L2 SPI) for the new provider.
2. Add the provider variant to the factory in `create_ai_service()`.
3. Document the required environment variables in [Configuration](../7-operation/configuration.md).
4. Test with `./sbh test ai`.

## See Also

- [Setup Guide](setup_guide.md) — Environment setup, build, troubleshooting
- [Architecture](../3-design/architecture.md) — Three-crate design, data flow
- [Agent Architecture](../3-design/agent_architecture.md) — Agent framework details
- [Test Strategy](../5-testing/testing_strategy.md) — Test coverage and approach
- [Backlog](backlog.md) — Development backlog and task tracking
- [Contributing](../../CONTRIBUTING.md) — Branch, commit, and PR conventions
