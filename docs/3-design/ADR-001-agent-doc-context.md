# ADR-001: Agent Documentation Context Strategy

> **TLDR:** Decision to inject documentation files into agent system prompts via a `docs` YAML config block.

**Audience**: Developers, architects
**Status:** Accepted (Implemented)
**Date:** 2026-02-05
**Authors:** Architecture Team
**Reviewers:** —

## Table of Contents

- [Context](#context)
- [Decision Drivers](#decision-drivers)
- [Options Considered](#options-considered)
- [Decision](#decision)
- [Implementation Outline](#implementation-outline)
- [Consequences](#consequences)
- [Migration Path](#migration-path)


## Context

Swebash AI agents are defined in YAML with a static `systemPrompt` that is embedded
at compile time via `include_str!()`. When an agent needs to reference project
documentation (e.g. `@rscagent` consulting RustScript's 212 doc files), there are
three possible approaches with very different tradeoffs.

The `@rscagent` currently uses approach A (fs tool reads) with doc paths listed in
the system prompt. This ADR evaluates all three and recommends a path forward.

## Decision Drivers

1. **Token efficiency** — avoid injecting irrelevant content into the context window.
2. **Iteration budget** — agents have `maxIterations` limits (10–25); doc reads
   should not consume iterations meant for actual work.
3. **Infrastructure cost** — swebash is a shell tool, not a cloud service;
   heavy dependencies (vector DB, embedding model) are disproportionate.
4. **Freshness** — documentation changes frequently; stale answers are harmful.
5. **Reliability** — the agent must consistently consult docs, not skip reads
   at the LLM's discretion.
6. **Simplicity** — fewer moving parts = easier to test, debug, and maintain.

## Options Considered

### A. `fs: true` — LLM reads docs via filesystem tool at runtime

The agent's system prompt lists doc paths and instructs the LLM to read them
before answering. Each file read is a tool call that consumes one iteration.

| Criterion | Assessment |
|-----------|------------|
| Token efficiency | Poor — entire files enter context even if one paragraph is relevant |
| Iteration budget | Poor — each read burns 1 of N max iterations |
| Infrastructure | None — uses existing tool system |
| Freshness | Excellent — always reads latest file |
| Reliability | Weak — LLM may skip reads, pick wrong files, or read too many |
| Simplicity | Excellent — zero new code |

### B. `docs` field — pre-load doc content at engine creation

A new `docs` YAML field (list of paths/globs) declares documentation sources.
At engine init, matching files are read, truncated to a token budget, and
prepended to the system prompt. No tool calls required at query time.

```yaml
- id: rscagent
  docs:
    budget: 8000          # max tokens for doc context
    sources:
      - docs/COMPONENTS.md
      - docs/ROUTING.md
      - doc/guide/04-signals.md
      - crates/compiler/*/README.md
```

| Criterion | Assessment |
|-----------|------------|
| Token efficiency | Moderate — full files loaded but capped by budget; no per-query relevance filtering |
| Iteration budget | Excellent — zero iterations consumed |
| Infrastructure | Low — file reads at init, token counting, truncation |
| Freshness | Good — loaded at engine creation; stale only within a session |
| Reliability | Excellent — docs always in context, no LLM discretion |
| Simplicity | Good — ~200 lines of new code (loader + budget logic) |

### C. RAG — retrieval-augmented generation with embeddings

Documentation is chunked, embedded, and stored in a vector index.
At query time, the user's message is embedded and the top-K relevant
chunks are injected into the conversation context.

| Criterion | Assessment |
|-----------|------------|
| Token efficiency | Excellent — only relevant chunks injected |
| Iteration budget | Excellent — zero iterations consumed |
| Infrastructure | Heavy — embedding model, vector store, indexing pipeline, chunk management |
| Freshness | Moderate — requires re-indexing when docs change |
| Reliability | Good — deterministic retrieval, but semantic search can miss exact keyword matches |
| Simplicity | Poor — significant new subsystem (~1000+ lines, external dependencies) |

## Decision

**Option B: `docs` field with token-budgeted pre-loading.**

Rationale:

- Eliminates the two main weaknesses of option A (iteration burn and unreliable reads)
  without the infrastructure weight of option C.
- The doc corpus is moderate (~212 files across RustScript, SEA docs). Per-agent,
  the relevant subset is 10–30 files — well within context window limits when
  budget-capped.
- Token budgeting provides a natural pressure valve: if a doc set grows too large,
  the budget forces curation of the most important sources rather than silent
  degradation.
- RAG (option C) remains a viable upgrade path if the corpus grows past context
  window limits or if cross-project doc search becomes a requirement.

## Implementation Outline

1. Add `docs` field to the swebash extension type `SwebashAgentExt` (flattened into `AgentEntry<SwebashAgentExt>` via `#[serde(flatten)]`):
   ```rust
   // in swebash-ai config.rs
   pub struct SwebashAgentExt {
       pub docs: Option<DocsConfig>,
       pub bypass_confirmation: Option<bool>,
       pub max_iterations: Option<usize>,
   }

   pub struct DocsConfig {
       pub budget: usize,             // max characters
       pub strategy: DocsStrategy,    // Preload (default) or Rag
       pub sources: Vec<String>,      // paths or globs, relative to base_dir
       pub top_k: usize,             // RAG results per query (default: 5)
   }
   ```

   Generic agent fields (`id`, `name`, `description`, `systemPrompt`, `tools`, `temperature`, etc.) are defined in `agent_controller::yaml::AgentEntry<Ext>`.

2. In `ConfigAgent::from_entry_with_base_dir()`, resolve globs, read files, truncate to
   budget, and prepend to `system_prompt` via a prompt modifier callback passed to
   `YamlAgentDescriptor::from_entry_with_prompt_modifier()`.

3. Token counting: uses a character-based heuristic (budget is in characters, not tokens).
   Heuristic is sufficient for budget enforcement.

4. If a source path doesn't exist, log a warning and skip — fail-open,
   consistent with agent system design.

5. Tests: docs loaded, docs missing, docs over budget, glob expansion, RAG strategy auto-enables rag tool category.

## Consequences

### Positive
- Agents always have documentation in context — no skipped reads.
- Zero tool iterations consumed for doc access.
- Simple to configure per-agent via YAML.
- No external dependencies.

### Negative
- Doc content is static for the engine's lifetime (until agent switch or restart).
- Token budget requires manual curation of source lists per agent.
- Large docs are truncated, potentially losing tail content.

### Risks
- Context window pressure: if `budget` is set too high, leaves less room for
  conversation history. Mitigated by making budget explicit and per-agent.
- Path drift: doc paths change and YAML isn't updated. Mitigated by fail-open
  + warning logs, and could add a `rsc check-agent-docs` validation command later.

## Migration Path

1. Implement option B.
2. Migrate `@rscagent` from inline doc path list to `docs` field.
3. If needed later, option C (RAG) can replace the doc loading layer without
   changing the YAML schema — `sources` could point to a vector index instead
   of file paths.
