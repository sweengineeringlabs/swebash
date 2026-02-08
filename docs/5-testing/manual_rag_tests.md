# Manual RAG Tests

> **TLDR:** Manual test checklist for RAG integration: indexing, query-time retrieval, staleness detection, vector store backends, and error handling.

**Audience**: Developers, QA

**WHAT**: Manual test procedures for the RAG (Retrieval-Augmented Generation) subsystem
**WHY**: Validates the end-to-end RAG pipeline from agent config through indexing to query-time retrieval
**HOW**: Step-by-step test tables with expected outcomes across 8 test groups (38 test cases)

> Requires the `rag-local` feature (enabled by default). See [Manual Testing Hub](manual_testing.md) for general prerequisites.

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Agent Configuration](#agent-configuration)
- [Startup Indexing](#startup-indexing)
- [Query-Time Retrieval](#query-time-retrieval)
- [Staleness Detection](#staleness-detection)
- [Vector Store Backends](#vector-store-backends)
- [Environment Variable Overrides](#environment-variable-overrides)
- [AWS-RAG End-to-End](#aws-rag-end-to-end)
- [Error Handling](#error-handling)

---

## Prerequisites

1. Generate test documentation (or use existing markdown files):

```bash
# Option A: Use AWS docs if available
./sbh gen-aws-docs

# Option B: Create minimal test docs
mkdir -p ~/.config/swebash/docs/rag-test
cat > ~/.config/swebash/docs/rag-test/api.md << 'EOF'
# API Reference

## Authentication
All API requests require a Bearer token in the Authorization header.
Use `POST /auth/login` with username and password to obtain a token.
Tokens expire after 3600 seconds.

## Endpoints

### GET /users
Returns a paginated list of users. Supports `?page=N&limit=N` query params.

### POST /users
Creates a new user. Required fields: `name`, `email`, `role`.

### DELETE /users/:id
Deletes a user by ID. Requires admin role.
EOF

cat > ~/.config/swebash/docs/rag-test/config.md << 'EOF'
# Configuration Guide

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | HTTP listen port |
| `DATABASE_URL` | required | PostgreSQL connection string |
| `LOG_LEVEL` | `info` | Logging verbosity: debug, info, warn, error |
| `CACHE_TTL` | `300` | Redis cache TTL in seconds |

## Database Setup

Run `./migrate up` to apply pending migrations.
Run `./migrate down` to roll back the last migration.
EOF
```

2. Define a RAG-enabled test agent in `~/.config/swebash/agents.yaml`:

```yaml
version: 1
rag:
  store: memory                # memory | file | sqlite
agents:
  - id: ragtest
    name: RAG Test Agent
    description: Test agent using RAG docs strategy
    triggerKeywords: [ragtest]
    tools:
      fs: true
      exec: false
      web: false
    docs:
      strategy: rag
      top_k: 5
      sources:
        - "docs/rag-test/*.md"
    systemPrompt: |
      You are a test agent with access to API and configuration documentation.
      Use the rag_search tool to look up specific details before answering.
```

3. Set environment:

```bash
set -a && source .env && set +a
export LLM_PROVIDER=anthropic
export SWEBASH_AI_DOCS_BASE_DIR=~/.config/swebash
```

---

## Agent Configuration

| Test | Steps | Expected |
|------|-------|----------|
| RAG agent listed | `ai agents` | Lists `ragtest` with description "Test agent using RAG docs strategy" |
| Switch to RAG agent | `ai` then `@ragtest` | Prints "Switched to RAG Test Agent (ragtest)", prompt changes to `[AI:ragtest] >` |
| No docs in prompt | Switch to `@ragtest`, inspect system prompt via debug logging | System prompt does **not** contain `<documentation>` block; does **not** contain raw doc file content |
| rag_search hint in prompt | Inspect `@ragtest` system prompt | Prompt contains `rag_search` usage hint appended after the agent's own system prompt |
| RAG tool auto-enabled | Check `@ragtest` tool filter via debug logging | Tool categories include `rag` and `fs` (auto-enabled by `strategy: rag`) |

## Startup Indexing

| Test | Steps | Expected |
|------|-------|----------|
| Index built on switch | Switch to `@ragtest`, observe stderr/tracing output | Log line: `building RAG index` with `agent=ragtest` and file count |
| Index built successfully | Same as above | Log line: `RAG index built successfully` with chunk count |
| Skip rebuild on re-switch | Switch to `@ragtest`, then `@shell`, then `@ragtest` again | Second switch logs `index is current, skipping rebuild` (no rebuild) |
| Missing sources tolerated | Change `docs.sources` to a non-existent glob, restart | Agent starts normally; logs `no doc files resolved, skipping index build` |

## Query-Time Retrieval

| Test | Steps | Expected |
|------|-------|----------|
| RAG search invoked | `@ragtest` then "what authentication method does the API use?" | LLM calls `rag_search` tool; response references Bearer tokens from api.md |
| Results include source | Observe tool call output in debug logging or verbose mode | Results formatted as `--- Result N (score: X.XXX, source: ...) ---` with chunk content |
| Multi-file retrieval | "what port does the server listen on?" | LLM calls `rag_search`; response references PORT=8080 from config.md |
| Specific detail lookup | "how do I delete a user?" | Response includes `DELETE /users/:id` and admin role requirement from api.md |
| No result graceful | "what is the weather today?" | `rag_search` returns "No relevant documentation found"; LLM handles gracefully |
| Multiple searches | "compare the authentication and database setup steps" | LLM may call `rag_search` multiple times to gather info from both files |

## Staleness Detection

| Test | Steps | Expected |
|------|-------|----------|
| Detect file change | While shell is running: modify `api.md` (add a new endpoint), then switch away from `@ragtest` and back | Logs `building RAG index` (fingerprint changed due to mtime/size) |
| Detect file addition | Add a new `faq.md` to `docs/rag-test/`, switch away and back to `@ragtest` | Index rebuilds; new file content is now searchable |
| Detect file deletion | Delete `config.md` from `docs/rag-test/`, switch away and back | Index rebuilds with fewer chunks; config.md content no longer returned |

## Vector Store Backends

| Test | Steps | Expected |
|------|-------|----------|
| Memory store (default) | Set `rag.store: memory` in YAML, restart, use `@ragtest` | RAG search works; index lost on shell restart (must rebuild) |
| File store | Set `rag.store: file` and `rag.path: /tmp/rag-test` in YAML, restart, use `@ragtest` | RAG search works; `/tmp/rag-test/ragtest.index.json` file created |
| File store persists | With file store configured, restart shell, switch to `@ragtest` | Logs `index is current, skipping rebuild` if docs unchanged (loaded from file) |
| SQLite store | Set `rag.store: sqlite` and `rag.path: /tmp/rag-test.db` in YAML (requires `rag-sqlite` feature), restart | RAG search works; SQLite database file created at specified path |
| Invalid store fallback | Set `rag.store: invalid` in YAML, restart | Falls back to memory store; agent works normally |

## Environment Variable Overrides

| Test | Steps | Expected |
|------|-------|----------|
| Env overrides YAML store | Set `SWEBASH_AI_RAG_STORE=file` and `SWEBASH_AI_RAG_STORE_PATH=/tmp/env-rag`, restart | File store used regardless of YAML `rag.store` value |
| Env overrides chunk size | Set `SWEBASH_AI_RAG_CHUNK_SIZE=500`, restart, switch to `@ragtest` | Index built with smaller chunks (more chunks than default) |
| Env overrides chunk overlap | Set `SWEBASH_AI_RAG_CHUNK_OVERLAP=50`, restart | Chunks have less overlap (visible in chunk count changes) |

## AWS-RAG End-to-End

> Requires `~/.config/swebash/agents.yaml` with the `awscli` agent configured with `strategy: rag` and AWS reference docs generated via `./sbh gen-aws-docs`.

| Test | Steps | Expected |
|------|-------|----------|
| AWS agent with RAG | Change `awscli` agent's `docs.strategy` to `rag` in `agents.yaml`, restart | Agent starts; index builds from 3 AWS doc files |
| Service lookup | `@awscli` then "what subcommands does the S3 service have?" | LLM calls `rag_search`; response includes S3 subcommands from services_reference.md |
| IaC pattern lookup | "how do I deploy a CloudFormation stack?" | Response includes CloudFormation deploy patterns from iac_patterns.md |
| Troubleshooting lookup | "how do I debug an AccessDenied error?" | Response includes auth diagnostics from troubleshooting.md |
| Cross-file query | "compare the CDK and SAM deployment workflows" | LLM calls `rag_search` (possibly multiple times); synthesizes answer from iac_patterns.md chunks |
| Refresh after doc regen | Re-run `./sbh gen-aws-docs`, restart shell, switch to `@awscli` | Index rebuilds (fingerprint changed); updated content searchable |
| Preload-to-RAG comparison | Test same query with `strategy: preload` then `strategy: rag` | Both produce correct answers; RAG uses `rag_search` tool call, preload has docs inline |

## Error Handling

| Test | Steps | Expected |
|------|-------|----------|
| Empty query rejected | Call `rag_search` with empty query (via prompt injection test) | Tool returns error: "'query' must not be empty" |
| Agent with no index | Create agent with `strategy: rag` but empty `sources: []`, switch to it, ask a question | `rag_search` returns "No relevant documentation found" gracefully |
| Graceful without rag-local | Build without `rag-local` feature (`cargo build --no-default-features`), configure RAG agent | Agent falls back to preload behavior; warning logged about RAG being disabled |

---

## See Also

- [Manual Testing Hub](manual_testing.md) — prerequisites and setup
- [Manual AI Tests](manual_ai_tests.md) — AI feature tests (non-RAG)
- [RAG Architecture](../3-design/rag_architecture.md) — RAG subsystem design
- [Creating Agents](../7-operation/creating_agents.md) — agent YAML schema
- [Configuration](../7-operation/configuration.md) — environment variable reference
