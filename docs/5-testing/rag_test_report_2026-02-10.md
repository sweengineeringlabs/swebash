# RAG Manual Test Report — 2026-02-10

> Test run against `manual_rag_tests.md` using `sbh build --debug` + `sbh run --debug`.
> Build: swebash-ai with `rag-local` feature (default). llmrag v0.1.0 from local registry.

---

## Environment

| Item | Value |
|------|-------|
| Date | 2026-02-10 |
| Branch | `dev` |
| LLM Provider | Anthropic (`claude-sonnet-4-20250514`) |
| API Status | Credits exhausted — LLM calls return HTTP 400 |
| Features | `rag-local` (default) |
| Platform | WSL2 / Linux 6.6.87.2-microsoft-standard-WSL2 |
| Docs Base Dir | `~/.config/swebash` |
| Test Docs | `api.md`, `config.md`, `faq.md` in `docs/rag-test/` |

---

## Agent Configuration (5 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 1 | RAG agent listed | **PASS** | `ragtest — Test agent using RAG docs strategy` shown in `ai agents` |
| 2 | Switch to RAG agent | **PASS** | `Switched to RAG Test Agent (ragtest)`, prompt shows `[AI:ragtest]` |
| 3 | No docs in prompt | **PASS** | No `<documentation>` block in system prompt (RAG strategy injects via tool, not inline) |
| 4 | rag_search hint in prompt | **SKIP** | Requires debug inspection of full system prompt text |
| 5 | RAG tool auto-enabled | **PASS** | RagTool registered for ragtest agent (visible in index build trigger on first chat) |

---

## Startup Indexing (4 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 6 | Index built on switch | **PASS** | `building RAG index agent=ragtest files=2` |
| 7 | Index built successfully | **PASS** | `RAG index built successfully agent=ragtest chunks=2` |
| 8 | Skip rebuild on re-switch | **PASS** | Engine cached — no rebuild on second switch within same session |
| 9 | Missing sources tolerated | **PASS** | `no doc files resolved, skipping index build` — no crash, agent starts normally |

---

## Query-Time Retrieval (6 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 10 | RAG search invoked | **BLOCKED** | Anthropic API credits exhausted — LLM cannot generate tool calls |
| 11 | Results include source | **BLOCKED** | Anthropic API credits exhausted |
| 12 | Multi-file retrieval | **BLOCKED** | Anthropic API credits exhausted |
| 13 | Specific detail lookup | **BLOCKED** | Anthropic API credits exhausted |
| 14 | No result graceful | **BLOCKED** | Anthropic API credits exhausted |
| 15 | Multiple searches | **BLOCKED** | Anthropic API credits exhausted |

---

## Staleness Detection (3 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 16 | Detect file change | **PASS** | Modified `api.md` → `building RAG index` triggered (fingerprint changed) |
| 17 | Detect file addition | **PASS** | Added `faq.md` → `files=3 chunks=3` (was 2 files, 2 chunks) |
| 18 | Detect file deletion | **PASS** | Deleted `config.md` → `files=2 chunks=2` (back to 2 from 3) |

---

## Vector Store Backends (6 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 19 | Memory store (default) | **PASS** | `store=Memory`, index builds, 3 files / 3 chunks |
| 20 | File store | **PASS** | `ragtest.index.json` + `ragtest.fingerprint` created at configured path |
| 21 | File store persists | **PASS** | `index is current (persisted), skipping rebuild` on restart |
| 22 | SQLite store | **PASS** | Correct error: `SQLite vector store requires the 'rag-sqlite' feature`; graceful fallback to preload |
| 23 | SweVecDB store | **SKIP** | Requires running SweVecDB server and `rag-swevecdb` feature |
| 24 | Invalid store fallback | **PASS** | `invalid_store` → falls back to `store=Memory`, works normally |

---

## SweVecDB Backend (11 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 25–35 | All SweVecDB tests | **SKIP** | Requires `rag-swevecdb` feature and running SweVecDB server |

---

## Environment Variable Overrides (5 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 36 | Env overrides YAML store | **PASS** | `SWEBASH_AI_RAG_STORE=file` → `store=File { path: "/tmp/env-rag" }` overrides YAML `memory` |
| 37 | Env overrides chunk size | **PASS** | `SWEBASH_AI_RAG_CHUNK_SIZE=500` → `chunk_size=500`; produced 4 chunks (vs 3 with default 2000) |
| 38 | Env overrides chunk overlap | **PASS** | `SWEBASH_AI_RAG_CHUNK_OVERLAP=50` → `chunk_overlap=50` |
| 39 | Env overrides swevecdb store | **SKIP** | Requires `rag-swevecdb` feature |
| 40 | Env overrides swevecdb endpoint | **SKIP** | Requires `rag-swevecdb` feature |

---

## AWS-RAG End-to-End (7 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 41–47 | All AWS-RAG tests | **SKIP** | Requires AWS CLI docs generated via `sbh gen-aws-docs` and working LLM API |

---

## Error Handling (5 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 48 | Empty query rejected | **BLOCKED** | Requires LLM to call `rag_search` with empty query |
| 49 | Agent with no index | **PASS** | Empty `sources: []` → no index built, agent starts normally |
| 50 | Graceful without rag-local | **SKIP** | Requires separate build with `--no-default-features` |
| 51 | SweVecDB without feature | **PASS** | `SweVecDB vector store requires the 'rag-swevecdb' feature` — graceful fallback to preload |
| 52 | SQLite without feature | **PASS** | `SQLite vector store requires the 'rag-sqlite' feature` — graceful fallback to preload |
| 53 | SweVecDB server down mid-session | **SKIP** | Requires running SweVecDB server |

---

## Summary

| Category | Pass | Blocked | Skip | Total |
|----------|------|---------|------|-------|
| Agent Configuration | 4 | 0 | 1 | 5 |
| Startup Indexing | 4 | 0 | 0 | 4 |
| Query-Time Retrieval | 0 | 6 | 0 | 6 |
| Staleness Detection | 3 | 0 | 0 | 3 |
| Vector Store Backends | 5 | 0 | 1 | 6 |
| SweVecDB Backend | 0 | 0 | 11 | 11 |
| Environment Variable Overrides | 3 | 0 | 2 | 5 |
| AWS-RAG End-to-End | 0 | 0 | 7 | 7 |
| Error Handling | 3 | 1 | 2 | 6 |
| **Total** | **22** | **7** | **24** | **53** |

- **22 PASS** — all infrastructure-level tests pass
- **7 BLOCKED** — require working LLM API (Anthropic credits exhausted)
- **24 SKIP** — require SweVecDB server, `rag-swevecdb`/`rag-sqlite` features, AWS docs, or separate build configurations
- **0 FAIL**

### Blockers for Full Coverage

1. **Anthropic API credits** — replenish to unblock 7 query-time retrieval and error handling tests
2. **SweVecDB server** — start a local instance and rebuild with `rag-swevecdb` feature to unblock 11 + 2 tests
3. **AWS CLI docs** — run `sbh gen-aws-docs` to unblock 7 AWS-RAG tests
4. **No-default-features build** — run `cargo build --no-default-features` to test graceful rag-local fallback
