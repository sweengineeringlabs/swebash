# RAG Architecture

> **TLDR:** Retrieval-Augmented Generation subsystem — chunking, local ONNX embeddings, vector storage, and tool-based semantic search for agent documentation.

**Audience**: Developers, architects

**WHAT**: Design of the RAG subsystem in `features/ai/src/core/rag/` and its integration with the agent framework
**WHY**: Agents with large or growing documentation corpora benefit from semantic retrieval rather than full pre-loading (see [ADR-001](ADR-001-agent-doc-context.md) Option C)
**HOW**: Documents are chunked, embedded locally via ONNX, stored in a pluggable vector store, and retrieved at query time through the `rag_search` tool

---

## Table of Contents

- [Overview](#overview)
- [Strategy Comparison: Preload vs RAG](#strategy-comparison-preload-vs-rag)
- [Architecture Diagram](#architecture-diagram)
- [Data Flow](#data-flow)
- [Component Reference](#component-reference)
- [SPI Contracts](#spi-contracts)
- [Configuration](#configuration)
- [End-to-End Pipeline Example: @awscli Agent](#end-to-end-pipeline-example-awscli-agent)
- [Staleness Detection](#staleness-detection)
- [Scaling Characteristics](#scaling-characteristics)
- [See Also](#see-also)


## Overview

The RAG subsystem provides an alternative documentation strategy for agents whose corpora exceed practical context-window budgets. Where the default `preload` strategy reads files at startup and bakes them into the system prompt, the `rag` strategy builds a vector index at startup and provides a `rag_search` tool the LLM invokes on demand.

### Module layout

```
features/ai/src/
├── core/rag/
│   ├── mod.rs              # Module root
│   ├── index.rs            # RagIndexManager — orchestration
│   ├── chunker.rs          # Sentence-aware text splitting
│   ├── embeddings.rs       # FastEmbedProvider (ONNX) + ApiEmbeddingProvider (placeholder)
│   ├── stores.rs           # InMemory, File, SQLite vector stores + VectorStoreConfig
│   └── tool.rs             # RagTool — rag_search tool for agents
├── spi/
│   └── rag.rs              # EmbeddingProvider + VectorStore traits, DocChunk, SearchResult
└── core/agents/
    ├── config.rs           # ConfigAgent (wraps YamlAgentDescriptor), DocsStrategy, DocsConfig, RagYamlConfig
    └── builtins.rs         # create_rag_manager(), YAML+env config merge

# Generic YAML types live in rustratify:
agent-controller/src/
└── yaml.rs                 # AgentEntry<Ext>, AgentDefaults, ToolsConfig, YamlAgentDescriptor
```

### Relationship to ADR-001

ADR-001 chose Option B (pre-load with token budget) as the default strategy and identified Option C (RAG) as a "viable upgrade path." This subsystem implements Option C. Both strategies coexist — each agent independently declares `strategy: preload` (default) or `strategy: rag` in its YAML `docs` block, and the framework handles both transparently.


## Strategy Comparison: Preload vs RAG

Use this table to decide which strategy fits an agent's documentation profile.

| Criterion | `preload` | `rag` |
|-----------|-----------|-------|
| **Best for** | Small-to-moderate corpora (< 30 files, < 12k tokens) | Large or growing corpora (> 30 files, > 12k tokens) |
| **Token usage** | Full content in system prompt (budget-capped) | Only retrieved chunks enter context |
| **Query-time cost** | Zero — docs always in context | One `rag_search` tool call per lookup |
| **Infrastructure** | None | Embedding model + vector store |
| **Freshness** | Per-session (loaded at engine init) | Per-session (indexed at engine init, fingerprint-checked) |
| **Reliability** | Excellent — docs always visible to LLM | Good — LLM must invoke `rag_search`; may miss if not called |
| **Feature flags** | None required | `rag-local` (embeddings), optionally `rag-sqlite` (persistence) |

**Guideline**: Start with `preload`. Switch to `rag` when the doc corpus exceeds the agent's token budget or when per-query relevance filtering becomes valuable.


## Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                      WRITE PATH (Indexing)                       │
│                                                                  │
│  Markdown docs (any source)                                      │
│       │                                                          │
│  agents.yaml (docs.sources globs)                                │
│       │                                                          │
│       ▼                                                          │
│  ┌──────────────────────────────────────────────────────┐        │
│  │            RagIndexManager.ensure_index()             │        │
│  │                                                      │        │
│  │  1. resolve_sources()     glob ──▶ file paths        │        │
│  │  2. compute_fingerprint() SHA-256(path+mtime+size)   │        │
│  │  3. if stale:                                        │        │
│  │     a. store.delete_agent()                          │        │
│  │     b. chunker::chunk_text()  ──▶ Vec<DocChunk>      │        │
│  │     c. embedder.embed()       ──▶ Vec<Vec<f32>>      │        │
│  │     d. store.upsert()                                │        │
│  │  4. update fingerprint cache                         │        │
│  └──────────────────────────────────────────────────────┘        │
│                          │                                       │
│              ┌───────────┼───────────┐                           │
│              ▼           ▼           ▼                           │
│         ┌────────┐ ┌──────────┐ ┌──────────┐                    │
│         │Chunker │ │Embedder  │ │VectorStore│                    │
│         │2000/200│ │BGE-small │ │Memory|File│                    │
│         │chars   │ │384-dim   │ │|SQLite    │                    │
│         └────────┘ └──────────┘ └──────────┘                    │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                      READ PATH (Query)                           │
│                                                                  │
│  User message ──▶ LLM ──▶ calls rag_search(query)               │
│                                                                  │
│  ┌──────────────────────────────────────────────────────┐        │
│  │                   RagTool.execute()                    │        │
│  │                                                      │        │
│  │  1. RagIndexManager.search(agent_id, query, top_k)   │        │
│  │     a. embedder.embed([query])  ──▶ query_vec        │        │
│  │     b. store.search(query_vec, agent_id, top_k)      │        │
│  │        ──▶ brute-force cosine similarity              │        │
│  │        ──▶ Vec<SearchResult> (sorted desc by score)   │        │
│  │  2. Format results as text block                      │        │
│  │     "--- Result 1 (score: 0.823, source: ...) ---"   │        │
│  └──────────────────────────────────────────────────────┘        │
│                          │                                       │
│                          ▼                                       │
│  LLM receives chunk text ──▶ generates answer                   │
└──────────────────────────────────────────────────────────────────┘
```


## Data Flow

### Indexing Pipeline (Write Path)

Triggered once per agent at engine startup (or on agent switch) via `RagIndexManager::ensure_index()`.

```
docs.sources           base_dir
  ["docs/*.md"]        /home/user/project
       │                    │
       ▼                    ▼
┌─────────────────────────────────┐
│  1. resolve_sources()           │
│     glob patterns ──▶ file list │
└────────────────┬────────────────┘
                 │  Vec<(PathBuf, String)>
                 ▼
┌─────────────────────────────────┐
│  2. compute_fingerprint()       │
│     SHA-256(path + mtime + size)│
└────────────────┬────────────────┘
                 │  fingerprint hex string
                 ▼
        ┌────────────────┐
        │ 3. fingerprint │──── match ────▶ SKIP (return Ok)
        │    == cached?  │
        └───────┬────────┘
                │ mismatch (or first run)
                ▼
┌─────────────────────────────────┐
│  4. store.delete_agent()        │
│     clear stale chunks          │
└────────────────┬────────────────┘
                 ▼
┌─────────────────────────────────┐
│  5. chunker::chunk_text()       │
│     sentence-aware splitting    │
│     2000 chars / 200 overlap    │
└────────────────┬────────────────┘
                 │  Vec<DocChunk>
                 ▼
┌─────────────────────────────────┐
│  6. embedder.embed(texts)       │
│     BAAI/bge-small-en-v1.5      │
│     batch ──▶ 384-dim vectors   │
└────────────────┬────────────────┘
                 │  Vec<Vec<f32>>
                 ▼
┌─────────────────────────────────┐
│  7. store.upsert(chunks, embs)  │
│     Memory | File | SQLite      │
└────────────────┬────────────────┘
                 ▼
┌─────────────────────────────────┐
│  8. cache fingerprint           │
│     index_state[agent_id] = fp  │
└─────────────────────────────────┘
```

### Query Pipeline (Read Path)

Triggered each time the LLM calls the `rag_search` tool during a conversation.

```
User message
     │
     ▼
┌──────────┐     tool call: rag_search({ "query": "..." })
│   LLM    │────────────────────────────┐
└──────────┘                            │
     ▲                                  ▼
     │                   ┌──────────────────────────────┐
     │                   │  1. RagTool.execute()         │
     │                   └──────────────┬───────────────┘
     │                                  ▼
     │                   ┌──────────────────────────────┐
     │                   │  2. embedder.embed([query])   │
     │                   │     ──▶ 384-dim query vector  │
     │                   └──────────────┬───────────────┘
     │                                  ▼
     │                   ┌──────────────────────────────┐
     │                   │  3. store.search(             │
     │                   │       query_vec,              │
     │                   │       agent_id,               │
     │                   │       top_k)                  │
     │                   │     brute-force cosine sim    │
     │                   │     ──▶ Vec<SearchResult>     │
     │                   └──────────────┬───────────────┘
     │                                  ▼
     │                   ┌──────────────────────────────┐
     │                   │  4. Format results            │
     │                   │  "--- Result 1 (score: 0.82,  │
     │                   │   source: ref.md) ---"        │
     │  tool result      │  {chunk content}              │
     └───────────────────┤                               │
                         └──────────────────────────────┘
```


## Component Reference

### RagIndexManager

**File**: `features/ai/src/core/rag/index.rs`

Orchestrates building, refreshing, and querying per-agent RAG indexes. Owns an `EmbeddingProvider`, a `VectorStore`, and a `ChunkerConfig`. Tracks per-agent fingerprints in an `Arc<RwLock<HashMap<String, String>>>`.

| Method | Signature | Purpose |
|--------|-----------|---------|
| `new` | `(embedder, store, chunker_config) -> Self` | Construct manager with injected dependencies |
| `ensure_index` | `(&self, agent_id, doc_sources, base_dir) -> AiResult<()>` | Build or skip-if-current the agent's index |
| `search` | `(&self, agent_id, query, top_k) -> AiResult<Vec<SearchResult>>` | Query the agent's index |

### Chunker

**File**: `features/ai/src/core/rag/chunker.rs`

Splits source text into overlapping `DocChunk`s that respect sentence boundaries via `unicode-segmentation`. Falls back to raw character splitting for text with no detectable sentence boundaries.

| Item | Detail |
|------|--------|
| `ChunkerConfig::default()` | `chunk_size: 2000`, `overlap: 200` (characters) |
| Algorithm | Accumulate whole sentences until `chunk_size` reached; rewind by `overlap` chars to sentence boundary for next chunk |
| Fallback | `chunk_raw()` — character-based splitting when `unicode_sentences()` returns one oversized segment |
| Chunk ID format | `{agent_id}:{source_path}:{byte_offset}` |

### EmbeddingProvider (FastEmbedProvider)

**File**: `features/ai/src/core/rag/embeddings.rs`

Local ONNX inference via the `fastembed` crate. Gated behind the `rag-local` feature flag (enabled by default).

| Item | Detail |
|------|--------|
| Model | `BAAI/bge-small-en-v1.5` |
| Dimensions | 384 |
| Runtime | ONNX (CPU) via `fastembed` crate |
| Download | Model auto-downloaded and cached on first use |
| API provider | `ApiEmbeddingProvider` exists as a placeholder for phase 2 (external APIs) |

### VectorStore

**File**: `features/ai/src/core/rag/stores.rs`

Three pluggable backends, all using brute-force cosine similarity (sufficient for agent doc corpora of 100–500 chunks).

| Backend | Feature gate | Persistence | Storage |
|---------|-------------|-------------|---------|
| `InMemoryVectorStore` | *(always)* | Ephemeral — lost on restart | `HashMap<agent_id, Vec<StoredEntry>>` in `RwLock` |
| `FileVectorStore` | *(always)* | Per-agent JSON files at `{store_dir}/{agent_id}.index.json` | JSON-serialized chunks + embeddings |
| `SqliteVectorStore` | `rag-sqlite` | Single SQLite database with `chunks` table | Embeddings as JSON arrays, indexed by `agent_id` |

`VectorStoreConfig` is a serde-tagged enum (`memory`, `file`, `sqlite`) with a `build()` method that constructs the appropriate backend.

### RagTool

**File**: `features/ai/src/core/rag/tool.rs`

Implements the rustratify `Tool` trait so agents can invoke `rag_search` via the standard tool calling mechanism.

| Item | Detail |
|------|--------|
| Tool name | `rag_search` |
| Risk level | `ReadOnly` |
| Confirmation | Not required |
| Timeout | 30 seconds |
| Parameters | `{ "query": string }` (required) |
| Output format | `--- Result N (score: 0.XXX, source: path) ---\n{content}` |
| Empty result | `"No relevant documentation found for your query."` |


## SPI Contracts

**File**: `features/ai/src/spi/rag.rs`

The SPI module declares trait interfaces and data types consumed by the `core::rag` implementations.

### Traits

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: &[String]) -> AiResult<Vec<Vec<f32>>>;
    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(&self, chunks: &[DocChunk], embeddings: &[Vec<f32>]) -> AiResult<()>;
    async fn search(&self, query_embedding: &[f32], agent_id: &str, top_k: usize) -> AiResult<Vec<SearchResult>>;
    async fn delete_agent(&self, agent_id: &str) -> AiResult<()>;
    async fn has_index(&self, agent_id: &str) -> AiResult<bool>;
}
```

### Data types

| Type | Fields | Purpose |
|------|--------|---------|
| `DocChunk` | `id`, `content`, `source_path`, `byte_offset`, `agent_id` | A chunk of text extracted from a documentation file |
| `SearchResult` | `chunk: DocChunk`, `score: f32` | A matched chunk with its cosine similarity score |


## Configuration

### YAML config

Agent-level and global RAG settings are declared in `agents.yaml`.

**Global RAG block** (top-level):

```yaml
rag:
  store: sqlite            # memory | file | sqlite
  path: .swebash/rag.db   # path for file/sqlite backends
  chunk_size: 2000         # chunk size in characters
  chunk_overlap: 200       # overlap between chunks in characters
```

**Per-agent docs block**:

```yaml
agents:
  - id: my-agent
    docs:
      strategy: rag        # preload (default) | rag
      budget: 12000        # token budget (preload only)
      top_k: 5             # results per RAG query (rag only, default: 5)
      sources:
        - "docs/ref/*.md"  # glob patterns resolved relative to base_dir
```

When `strategy: rag` is set, the framework auto-enables the `rag` tool category for that agent and appends a `rag_search` usage hint to the system prompt.

### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `SWEBASH_AI_RAG_STORE` | `memory` | Vector store backend: `memory`, `file`, `sqlite` |
| `SWEBASH_AI_RAG_STORE_PATH` | `.swebash/rag` | Path for file/sqlite backends |
| `SWEBASH_AI_RAG_CHUNK_SIZE` | `2000` | Chunk size in characters |
| `SWEBASH_AI_RAG_CHUNK_OVERLAP` | `200` | Overlap between chunks in characters |
| `SWEBASH_AI_TOOLS_RAG` | `false` | Global RAG tool enable (per-agent `strategy: rag` overrides this) |
| `SWEBASH_AI_DOCS_BASE_DIR` | *(cwd)* | Base directory for resolving agent doc source globs |

Environment variables override YAML values when set.

### Feature flags

| Flag | Cargo feature | Dependencies | Purpose |
|------|---------------|--------------|---------|
| `rag-local` | `default` | `fastembed` | Enable local ONNX embedding via FastEmbed |
| `rag-sqlite` | opt-in | `rusqlite` (bundled) | Enable SQLite vector store backend |

Without `rag-local`, agents configured with `strategy: rag` fall back to `preload` behavior.


## End-to-End Pipeline Example: @awscli Agent

This section uses the `@awscli` agent as a concrete example of the full doc-generation-to-retrieval pipeline. The same pattern applies to any agent — substitute your own doc generation script and source globs.

### Step-by-step walkthrough

| Phase | Step | Detail |
|-------|------|--------|
| **Doc generation** | 1 | Generate documentation (e.g., `./sbh gen-aws-docs` for the awscli agent) |
| | 2 | Script produces markdown files to a known directory (e.g., `~/.config/swebash/docs/aws/`) |
| **Agent config** | 3 | Define the agent in `agents.yaml` with `docs.sources` globs pointing to the generated files |
| | 4 | Set `docs.strategy: rag` (or `preload` for smaller corpora) |
| **Startup indexing** | 5 | `create_default_registry()` in `builtins.rs` parses all YAML layers (embedded defaults, project-local, user config) |
| | 6 | `create_rag_manager()` initializes `FastEmbedProvider` + chosen `VectorStore` backend |
| | 7 | For each agent with `strategy: rag`, `RagIndexManager::ensure_index()` resolves globs, fingerprints files, chunks, embeds, and upserts |
| **Query-time retrieval** | 8 | User sends a message to the agent |
| | 9 | LLM sees `rag_search` tool in its available tools and system prompt hint |
| | 10 | LLM calls `rag_search({ "query": "..." })` |
| | 11 | `RagTool` embeds query, performs cosine search, returns top-k chunks |
| | 12 | LLM uses retrieved chunks to generate an accurate answer |
| **Refresh cycle** | 13 | User regenerates docs (e.g., after a CLI upgrade) |
| | 14 | On next swebash startup, `ensure_index()` detects changed fingerprint and rebuilds index |


## Staleness Detection

The `RagIndexManager` uses SHA-256 fingerprinting to avoid unnecessary re-indexing.

### Fingerprint inputs

For each file resolved from `docs.sources` globs:

| Input | Source | Purpose |
|-------|--------|---------|
| Relative path | `strip_prefix(base_dir)` | Detect file additions/removals |
| File size | `fs::metadata().len()` | Detect content changes |
| Modification time | `fs::metadata().modified()` (as UNIX epoch seconds) | Detect edits |

All three values are fed into a SHA-256 hasher in order, producing a single hex digest per agent.

### Re-indexing triggers

| Trigger | Mechanism |
|---------|-----------|
| New file added to glob match | Path changes fingerprint |
| File deleted from glob match | Fewer paths changes fingerprint |
| File content modified | mtime and/or size changes fingerprint |
| Agent switched or swebash restarted | `ensure_index()` called, compares cached vs computed fingerprint |
| First run (no cached fingerprint) | Always builds index |


## Scaling Characteristics

| Parameter | Value | Notes |
|-----------|-------|-------|
| Embedding model | BAAI/bge-small-en-v1.5 | 33M parameters, CPU inference |
| Vector dimensions | 384 | Compact; cosine similarity is fast |
| Chunk size | 2000 chars (default) | ~500 tokens per chunk |
| Chunk overlap | 200 chars (default) | Ensures context continuity at chunk boundaries |
| Target corpus | 100–500 chunks per agent | Brute-force cosine is sufficient at this scale |
| Search complexity | O(n) per query | Linear scan over agent's chunks; no ANN index needed |
| Indexing latency | ~1–5s for typical agent corpus | Dominated by embedding time (CPU-bound) |
| Query latency | < 100ms | Embedding one query + linear scan of 100–500 vectors |
| Model download | ~25 MB (one-time) | Cached in `.fastembed_cache/` |
| Memory (in-memory store) | ~1.5 KB per chunk (384 × 4 bytes + metadata) | ~750 KB for 500 chunks |

For corpora beyond 500 chunks, consider upgrading to an ANN index (e.g., `hnswlib`). The `VectorStore` trait makes this a drop-in replacement.


## See Also

- [ADR-001: Agent Documentation Context Strategy](ADR-001-agent-doc-context.md) — Decision that established preload as default and RAG as upgrade path
- [Agent Architecture](agent_architecture.md) — Agent framework design: prompts, tools, memory isolation
- [@awscli Agent](../7-operations/awscli_agent.md) — User-facing setup guide for the AWS Cloud agent
- [Creating Agents](../7-operations/creating_agents.md) — YAML schema and examples for custom agents
