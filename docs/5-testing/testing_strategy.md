# swebash Testing Strategy

> **TLDR:** 8-category test pyramid covering unit through security, with shared
> swebash-test framework, feature-gated expensive tests, and every-commit CI.

**Audience**: Developers, Contributors

## Test Organization

Tests follow the rustboot testing strategy (8 categories) with consistent
file naming conventions enforced by `swebash_test::naming`.

### Test Categories

| Category | Location | Naming | Gate | CI Cadence |
|---|---|---|---|---|
| Unit | src/*.rs (inline #[cfg(test)]) | `<fn>_<scenario>` | -- | Every commit |
| Feature | src/*.rs (inline #[cfg(test)]) | `<feature>_<behavior>` | -- | Every commit |
| Integration | tests/`<crate>`_int_test.rs | descriptive | -- | Every commit |
| Stress | tests/`<crate>`_stress_test.rs | `stress_test_*` | stress | Nightly |
| Performance | tests/`<crate>`_perf_test.rs | `perf_*` | perf | Nightly |
| Load | tests/`<crate>`_load_test.rs | `load_*` | load | Weekly |
| E2E | tests/`<crate>`_e2e_test.rs | `e2e_*` | live | Integration gate |
| Security | tests/`<crate>`_security_test.rs | `security_*` | -- | Every commit |

### Testing Pyramid

```
             /----\          E2E (live feature gate)
            /  Load  \        Load (load feature gate)
           /   Perf   \      Performance (perf feature gate)
          /   Stress    \     Stress (stress feature gate)
         /   Security    \    Security (every commit)
        /   Integration   \   Integration (every commit)
       /     Feature       \  Feature (every commit)
      /        Unit         \ Unit (every commit)
```

### Quality Attributes (ISO 25010)

| Attribute | Test Category | swebash Coverage |
|---|---|---|
| Functional correctness | Unit, Feature, Integration | Translate, explain, chat, autocomplete, shell commands |
| Fault tolerance | Integration, E2E | Error chain, error propagation, service-level error tests |
| Recoverability | E2E | Service remains usable after error |
| Performance | Performance, Load | LLM response times, streaming throughput |
| Security | Security | Prompt injection, API key leak, input validation |
| Configurability | Integration | Config, YAML parsing, project-local, env var tests |
| Modularity | Integration | Agent framework, delegate tests |

## Per-Crate Test Structure

### swebash-ai (features/ai/)

```
features/ai/
├── src/            # Unit + Feature tests (inline #[cfg(test)])
└── tests/
    ├── ai_int_test.rs        # Integration (config, chat, agents, YAML, RAG)
    ├── ai_e2e_test.rs        # E2E (tool invocation, RAG E2E, SweVecDB)
    ├── ai_stress_test.rs     # Stress (concurrent sessions, rapid switching)
    ├── ai_perf_test.rs       # Performance (response times, throughput)
    ├── ai_security_test.rs   # Security (prompt injection, input validation)
    └── ai_load_test.rs       # Load (sustained volumes, memory growth)
```

### swebash host (features/shell/host/)

```
features/shell/host/
├── src/            # Unit + Feature tests (inline #[cfg(test)])
└── tests/
    ├── host_int_test.rs      # Integration (commands, fs, env, history)
    └── host_readline_int_test.rs  # Integration (readline, editing, AI mode)
```

### Shell Scripts (bin/tests/)

```
bin/tests/
├── runner.sh               # Master test runner
├── e2e/sbh.test.sh         # E2E shell tests
└── feature/                # Feature-specific shell tests
```

## Shared Test Framework

All crates depend on `swebash-test` (dev-dependency) for shared infrastructure:

```rust
use swebash_test::prelude::*;

// Mocks
let service = create_mock_service_fixed("Hello");

// Streaming
let (deltas, done) = collect_stream_events(&mut rx).await;
assert_no_duplication(&deltas, &done.unwrap());

// Fixtures
let dir = ScopedTempDir::new("my_test").unwrap();
dir.write_file("config.yaml", "enabled: true").unwrap();

// Latency
assert_latency_p99(&samples, Duration::from_secs(2));
```

See `docs/3-design/test_framework_architecture.md` for framework details.

## Running Tests

```bash
# All tests (every-commit categories)
cargo test --workspace

# Specific crate
cargo test -p swebash-ai

# Specific category
cargo test -p swebash-ai --test ai_int_test
cargo test -p swebash-ai --test ai_security_test

# Feature-gated categories
cargo test -p swebash-ai --test ai_stress_test --features swebash-test/stress
cargo test -p swebash-ai --test ai_perf_test --features swebash-test/perf
cargo test -p swebash-ai --test ai_load_test --features swebash-test/load
cargo test -p swebash-ai --test ai_e2e_test --features swebash-test/live

# Shell script tests
bash bin/tests/runner.sh
```

## Naming Conventions

```
# Function names:
Unit/Feature/Integration:  descriptive_name (e.g., chat_returns_reply)
Stress:                    stress_test_<description>
Performance:               perf_<description>
Load:                      load_<description>
E2E:                       e2e_<description>
Security:                  security_<description>

# File names:
tests/<crate>_int_test.rs
tests/<crate>_stress_test.rs
tests/<crate>_perf_test.rs
tests/<crate>_load_test.rs
tests/<crate>_e2e_test.rs
tests/<crate>_security_test.rs
```

## Benchmarks

Use Criterion for micro-benchmarks:

```
features/ai/benches/
└── ai_bench.rs     # LLM mock throughput, RAG index build time
```

```bash
cargo bench -p swebash-ai
```

## Fuzzing

Use cargo-fuzz for input parsing boundaries:

```
features/ai/fuzz/
└── fuzz_targets/
    ├── fuzz_translate_parse.rs    # Fuzz translate response parsing
    └── fuzz_chat_input.rs         # Fuzz chat request handling
```

```bash
cargo +nightly fuzz run fuzz_translate_parse -p swebash-ai
```

## CI Integration

```
Every commit:   Unit + Feature + Integration + Security
Nightly:        + Stress + Performance
Weekly:         + Load
Integration:    + E2E (requires API keys / live services)
```
