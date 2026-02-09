# swebash-test Framework Architecture

> **TLDR:** Reusable test framework crate following rustboot-test patterns and
> rustratify Single-Crate Flat SEA architecture.

**Audience**: Developers, Contributors

## Overview

swebash-test is a workspace-level test infrastructure crate that provides
shared mock objects, RAII fixtures, streaming assertions, security scanners,
and naming conventions for all swebash crates. It follows the rustboot-test
pattern adapted for the AI/LLM/shell domain.

## Architecture

Single-Crate Flat SEA (infrastructure utility):

```
swebash-test/src/
├── lib.rs        # Module declarations + prelude
├── error.rs      # TestError enum (framework errors)
├── mock.rs       # AI mock infrastructure
├── fixture.rs    # RAII temp directories + scoped fixtures
├── naming.rs     # Test category conventions (8 categories)
├── assert.rs     # Performance + AI-specific assertions
├── retry.rs      # Exponential backoff utilities
├── security.rs   # Security payload scanner
└── stream.rs     # ChatStreamEvent test helpers
```

## Design Decisions

1. **Flat SEA** (no spi/api/core subdirs): < 2000 lines, infrastructure crate
2. **Depends on swebash-ai**: provides domain-specific mocks (MockAiClient, etc.)
3. **Feature-gated categories**: stress/perf/load/live gate expensive test infra
4. **Prelude pattern**: `use swebash_test::prelude::*` for common test imports

## Module Responsibilities

### mock.rs — AI Mock Infrastructure
- `MockAiClient`: returns fixed "mock" responses
- `ErrorMockAiClient`: returns `AiError::Provider` for error path testing
- `MockEmbedder`: deterministic 8-dim vectors for RAG tests
- Service builders: `create_mock_service()`, `create_mock_service_fixed()`,
  `create_mock_service_error()`, `create_mock_service_full_error()`
- `mock_config()`: standard `AiConfig` for mock-backed tests
- `MockRecorder`: call tracking for mock inspection

### fixture.rs — RAII Test Resources
- `ScopedTempDir`: auto-cleaned temp dirs with helper methods
- `ScopedFixture<T>`: generic RAII wrapper with cleanup callback

### stream.rs — Streaming Assertions
- `collect_stream_events()`: drain `ChatStreamEvent` receiver
- `assert_no_duplication()`: verify delta concat == done text
- `assert_done_event_contains()`: verify Done payload
- `assert_no_events_after_done()`: verify clean stream termination

### assert.rs — Performance + AI Assertions
- Latency: `assert_latency_p95()`, `assert_latency_p99()`
- Throughput: `assert_throughput_above()`
- Consistency: `assert_eventually_consistent()`
- AI-specific: `assert_ai_error_format()`, `assert_setup_error()`

### naming.rs — Test Category Conventions
8 categories (Unit, Feature, Integration, Stress, Performance, Load, E2E,
Security) with file naming, function prefix, feature gate, and CI cadence.

### security.rs — AI Security Testing
Prompt injection, API key leak, input validation, DoS payloads with
`SecurityScanner` trait for running payloads against targets.

### retry.rs — Backoff Utilities
Sync and async retry with exponential backoff for flaky test environments.

## Dependency Graph

```
swebash-test
  ├── swebash-ai (domain types: AiClient, AiError, ChatStreamEvent)
  ├── llm-provider[testing] (MockLlmService, MockBehaviour)
  ├── agent-controller[testing] (mock agent infrastructure)
  ├── tokio (async runtime)
  └── tempfile (RAII temp directories)
```

Consumer crates add `swebash-test` as `[dev-dependencies]` only.

## Relationship to rustboot-test

| rustboot-test module | swebash-test adaptation |
|---|---|
| mock/ (recorder) | mock.rs (AI-domain mocks + recorder) |
| fixture/ (scoped, tempdir) | fixture.rs (flattened) |
| naming/ (categories, validate) | naming.rs (same 8 categories) |
| assert/ (latency, throughput, consistency) | assert.rs (+ AI assertions) |
| retry.rs | retry.rs (direct port) |
| security/ (types) | security.rs (AI payloads) |
| server.rs, http.rs, sse.rs | Not needed (no HTTP server) |
