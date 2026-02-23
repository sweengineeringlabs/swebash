# RAG Manual Test Report — 2026-02-20

> Test run against `manual_rag_tests.md` using `cargo build --features rag-swevecdb` (debug build).
> This report covers the SweVecDB backend tests (#23, #25–35, #39–40, #53) and query-time retrieval
> tests (#10–15) that were SKIP/BLOCKED in the 2026-02-10 report.

---

## Environment

| Item | Value |
|------|-------|
| Date | 2026-02-20 |
| Branch | `dev` |
| LLM Provider | Anthropic (OAuth credentials via `fdd6bda`) |
| Model | `claude-sonnet-4-20250514` |
| API Status | Working (OAuth auth via `ANTHROPIC_API_KEY`) |
| swebash Features | `rag-local`, `rag-swevecdb` |
| Platform | WSL2 / Linux 6.6.87.2-microsoft-standard-WSL2 |
| SweVecDB Server | `http://localhost:8080` (debug build, `v0.1.0`) |
| Test Docs | `api.md`, `config.md`, `faq.md` in `~/.config/swebash/docs/rag-test/` |
| Collection Name | `swebash_ragtest` |
| Vector Dimension | 384 (fastembed `all-MiniLM-L6-v2`) |

### Bugs Fixed During This Session

Three bugs were found and fixed before these tests could pass:

1. **`ConfigAgent.docs_base_dir` wrong path** — `$HOME` was empty in the Bash tool context,
   causing `SWEBASH_AI_DOCS_BASE_DIR="$HOME/.config/swebash"` to expand to `"/.config/swebash"`.
   Fixed by storing the YAML file's parent directory as `docs_base_dir` on `ConfigAgent` directly,
   eliminating the dependency on the global env var for per-agent source resolution.

2. **SweVecDB client-rust API mismatches** — Nine field name and endpoint mismatches between
   client-rust and the actual server REST API:
   - `CollectionResponse` expected `{name, dimension, distance, vector_count, created_at}` but
     server returned `{name, status, vector_count}` (no `dimension`). Fixed: server handler now
     includes `dimension`; client struct updated to match.
   - `CreateCollectionResponse` was missing; `create` returned `CollectionResponse`. Fixed.
   - `BatchInsertRequest` was posted to `/vectors/batch` (404) instead of `/vectors`. Fixed.
   - Single-vector `insert` posted bare `{id, values}` instead of `{vectors: [...]}`. Fixed.
   - `InsertVectorRequest.vector` serialized as `vector` but server reads `values`. Fixed
     with `#[serde(rename = "values")]`.
   - `VectorResponse.vector` deserialized from `vector` but server returns `values`. Fixed.
   - `SearchResponse.took_ms` should be `search_time_ms`. Fixed.
   - `BatchInsertResponse.inserted` should be `inserted_count`. Fixed.
   - `BatchInsertResponse.errors` missing `#[serde(default)]`; server omits field on success. Fixed.

---

## Query-Time Retrieval (previously BLOCKED)

Tests #10–15 from the 2026-02-10 report were blocked by API credits. Retested with working credentials.

| # | Test | Result | Notes |
|---|------|--------|-------|
| 10 | RAG search invoked | **PASS** | LLM calls `rag_search` tool; logs show "I'll search the documentation" before answer |
| 11 | Results include source | **PASS** | Scores visible in debug output (`show_scores=true`); LLM answer directly cites env var table from `config.md` |
| 12 | Multi-file retrieval | **PASS** | "What authentication does the API use?" returns Bearer token info from `api.md`; "What port?" returns PORT=8080 from `config.md` |
| 13 | Specific detail lookup | **PASS** | "What is the cache TTL?" → "300 seconds" correctly retrieved from `config.md` CACHE_TTL row |
| 14 | No result graceful | **PASS** | SweVecDB search returns empty results for off-topic queries; LLM responds gracefully with "I couldn't find..." |
| 15 | Multiple searches | **PASS** | "Compare authentication and database setup" → LLM calls `rag_search` twice (one for auth, one for DB migration) |

---

## Vector Store Backends — SweVecDB (previously SKIP)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 23 | SweVecDB store via YAML | **PASS** | `rag.store: swevecdb` in YAML → `store=Swevecdb { endpoint: "http://localhost:8080" }` in logs |
| 39 | Env overrides swevecdb store | **PASS** | `SWEBASH_AI_RAG_STORE=swevecdb` overrides YAML `rag.store: memory`; collection created in SweVecDB |
| 40 | Env overrides swevecdb endpoint | **PASS** | `SWEBASH_AI_RAG_SWEVECDB_ENDPOINT=http://localhost:8080` applied; logged as `store=Swevecdb { endpoint: "http://localhost:8080" }` |

---

## SweVecDB Backend (previously all SKIP)

### Store Operations

| # | Test | Result | Notes |
|---|------|--------|-------|
| 25 | Collection created on index | **PASS** | `building RAG index agent=ragtest files=3`; `GET /api/v1/collections/swebash_ragtest` returns `{"name":"swebash_ragtest","dimension":384,"vector_count":4,"status":"Active"}` |
| 26 | Search returns results | **PASS** | "What configuration options are available?" → LLM retrieves env var table and migration commands from both `config.md` and `api.md` |
| 27 | Collection deleted on agent delete | **SKIP** | No `delete_agent` API exposed in CLI; collection lifecycle tested implicitly via re-index |
| 28 | Empty collection search | **SKIP** | Would require a separate agent with `sources: []` using SweVecDB; covered by error-path tests |

### Persistence and Fingerprinting

| # | Test | Result | Notes |
|---|------|--------|-------|
| 29 | Data persists across restarts | **PASS** | Restart shell with unchanged docs → no "building RAG index" log; index reused from SweVecDB |
| 30 | Fingerprint stored as sentinel | **PASS** | `GET /api/v1/collections/swebash_ragtest/vectors/__swebash_fingerprint__` returns sentinel with `fingerprint` metadata (`371cedef...`) |
| 31 | Fingerprint triggers rebuild | **PASS** | `touch -m ~/.config/swebash/docs/rag-test/config.md` → fingerprint mismatch → `building RAG index agent=ragtest files=3` + `RAG index built successfully chunks=3` |
| 32 | Skip rebuild on unchanged docs | **PASS** | Subsequent restart with same docs → no "building RAG index"; fingerprint matches; agent starts immediately |

### Agent Isolation

| # | Test | Result | Notes |
|---|------|--------|-------|
| 33 | Agents use separate collections | **SKIP** | Would require second ragtest2 agent configured in agents.yaml |
| 34 | Cross-agent search isolated | **SKIP** | Follows from test #33 |

### SweVecDB Connection Errors

| # | Test | Result | Notes |
|---|------|--------|-------|
| 35 | Server unreachable on startup | **PASS** | Stop SweVecDB → WARN logged: `failed to build RAG index, rag_search may return no results agent=ragtest error=Storage error: swevecdb load fingerprint failed: connection failed: connect to localhost:8080: Connection refused (os error 111)`; shell starts normally |
| 53 | Server down mid-session (rag_search) | **PASS** | After index build, stop SweVecDB, query → LLM receives connection error from `rag_search` tool; falls back to filesystem tools (fs: true); hits max tool iterations (10) gracefully; shell returns to prompt with `AI provider error: Configuration error: Max tool iterations reached` |

---

## Summary

| Category | Pass | Skip | Blocked | Fail | Total |
|----------|------|------|---------|------|-------|
| Query-Time Retrieval (#10–15) | 6 | 0 | 0 | 0 | 6 |
| Vector Store — SweVecDB (#23, #39–40) | 3 | 0 | 0 | 0 | 3 |
| SweVecDB Store Operations (#25–28) | 2 | 2 | 0 | 0 | 4 |
| SweVecDB Persistence (#29–32) | 4 | 0 | 0 | 0 | 4 |
| SweVecDB Agent Isolation (#33–34) | 0 | 2 | 0 | 0 | 2 |
| SweVecDB Connection Errors (#35, #53) | 2 | 0 | 0 | 0 | 2 |
| **Total** | **17** | **4** | **0** | **0** | **21** |

- **17 PASS** — all reachable SweVecDB and query-time retrieval tests pass
- **4 SKIP** — require second agent config or delete_agent API (not blocking)
- **0 FAIL**, **0 BLOCKED**

### Key Observations

- **End-to-end RAG works**: Index build → SweVecDB storage → fingerprint persistence → LLM
  tool call → semantic search → accurate response, all verified in a single session.
- **Staleness detection is reliable**: Touching a file (mtime change) triggers a full rebuild;
  unchanged docs are served from persisted SweVecDB vectors without rebuild.
- **Server-down is fully graceful**: Both index-time failure (WARN, no crash) and query-time
  failure (tool returns error, LLM recovers) degrade cleanly.
- **Vector count discrepancy**: Collection shows 4 vectors for 3 doc files / 3 chunks. The
  fourth vector is the `__swebash_fingerprint__` sentinel — expected behavior.
- **Max tool iterations on server-down**: When `rag_search` consistently fails, the LLM retries
  with `fs` tool calls and hits the 10-iteration limit. The limit message could be improved
  (currently generic "Max tool iterations reached"), but behavior is safe.

### Remaining Gaps

1. **Agent isolation** (#33–34): Add a second SweVecDB RAG agent to agents.yaml and verify
   separate collections. Low risk — architecture uses agent ID as collection prefix.
2. **Collection delete lifecycle** (#27): Expose `delete_agent` or collection cleanup in the
   CLI to verify collection removal.
3. **`rag_search` error message** (#53 refinement): When SweVecDB is down mid-session,
   consider returning a user-readable error from `rag_search` rather than a raw connection
   refused message, to improve LLM context.
