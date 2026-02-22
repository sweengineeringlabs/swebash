# Performance Audit: Wasmtime JIT Compilation Baseline

**Date:** 2026-02-22
**Type:** Baseline
**Purpose:** Establish pre-optimization metrics for wasmtime module caching

## Environment

| Parameter | Value |
|-----------|-------|
| Platform | Windows 10 (MINGW64) |
| wasmtime | 29 (default config, no caching) |
| Engine WASM size | 26 KB |
| Test parallelism | Default (multi-threaded) |
| Test count | 94 integration tests |

## Single Process Startup

| Run | Time (ms) |
|-----|-----------|
| 1 | 789 |
| 2 | 758 |
| 3 | 689 |
| 4 | 724 |
| 5 | 744 |
| **Mean** | **741** |
| **Min** | 689 |
| **Max** | 789 |

## Integration Test Suite

| Run | Time (s) |
|-----|----------|
| 1 | 24.50 |
| 2 | 25.41 |
| 3 | 24.83 |
| **Mean** | **24.91** |

## Methodology

```bash
# Single process startup
time (echo "exit" | target/release/swebash)

# Test suite
time cargo test --release -p swebash
```

## Observations

- JIT compilation accounts for ~700ms per process spawn
- 94 tests running in parallel partially amortize JIT overhead across CPU cores
- Total test time dominated by cumulative JIT compilation
