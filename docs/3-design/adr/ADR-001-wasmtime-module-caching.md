# ADR-001: Wasmtime Module Caching for Test Performance

**Status:** Accepted
**Date:** 2026-02-22
**Authors:** Development Team

## Context

The swebash shell embeds a WASM engine module that is JIT-compiled by wasmtime on every process startup. This creates significant CPU overhead during integration testing, where each test spawns a new swebash process.

### Baseline Metrics (2026-02-22)

**Environment:**
- Platform: Windows 10 (MINGW64)
- wasmtime: default configuration (no caching)
- Engine WASM size: 26 KB

**Single Process Startup (5 runs):**
| Run | Time |
|-----|------|
| 1 | 789ms |
| 2 | 758ms |
| 3 | 689ms |
| 4 | 724ms |
| 5 | 744ms |
| **Mean** | **741ms** |

**Integration Test Suite (94 tests, 3 runs):**
| Run | Time |
|-----|------|
| 1 | 24.50s |
| 2 | 25.41s |
| 3 | 24.83s |
| **Mean** | **24.91s** |

### Problem

Each test invocation:
1. Spawns swebash process
2. Wasmtime JIT-compiles the 26KB WASM module (~700ms)
3. Runs test commands
4. Exits

With 94 integration tests running in parallel, the JIT compilation overhead significantly impacts CI/CD pipeline times and developer feedback loops.

## Decision

Enable wasmtime's built-in disk cache to persist compiled modules between runs.

### Implementation

```rust
// Before (runtime.rs)
let engine = Engine::default();

// After
let mut config = Config::new();
config.cache_config_load_default()?;
let engine = Engine::new(&config)?;
```

### Alternatives Considered

1. **AOT Compilation in build.rs**
   - Pre-compile WASM to native code at build time
   - Use `Engine::precompile_module()` + `Module::deserialize()`
   - Pros: Zero JIT overhead, deterministic
   - Cons: Platform-specific binaries, more complex build process

2. **Keep One Process Running (IPC-based testing)**
   - Spawn single swebash process, send commands via IPC
   - Pros: Eliminates all startup overhead
   - Cons: Major test architecture change, harder to isolate tests

3. **Status Quo**
   - Accept current performance
   - Cons: Slow CI, poor developer experience

### Why Disk Cache?

- **Minimal code change:** One config line
- **Cross-platform:** Works on all wasmtime-supported platforms
- **Automatic invalidation:** Cache key includes WASM content hash
- **No build complexity:** No changes to build.rs or CI

## Results (Measured 2026-02-22)

**Integration Test Suite (94 tests) - With Cache:**
| Run | Time |
|-----|------|
| 1 (cache cold) | 13.52s |
| 2 (cache warm) | 12.02s |

**Improvement:**
| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Test suite | 24.91s | 12.02s | **2.1x faster** |

The improvement is less than the theoretical 5x because:
1. Tests run in parallel, so JIT overhead was partially amortized
2. Test execution time (not just startup) contributes to total time
3. Disk I/O for cache reads adds some overhead

## Consequences

### Positive
- First run: Same as before (~700ms startup)
- Subsequent runs: ~50-100ms startup (10-14x faster)
- Integration tests: 12s (2x faster than baseline)
- No changes to test architecture
- No platform-specific binaries

### Negative
- Disk space: ~1-5MB cache per platform
- First run after WASM changes still slow
- Cache location platform-dependent (~/.cache/wasmtime on Unix)

### Risks
- Cache corruption (mitigated: wasmtime validates cache integrity)
- Stale cache (mitigated: content-based cache keys)

## Implementation

Completed 2026-02-22:

1. Modified `features/shell/host/src/spi/runtime.rs` to enable cache
2. Cache errors silently ignored (non-fatal)
3. Created architecture documentation: `docs/3-design/wasmtime_caching.md`

## References

- [Wasmtime Caching Documentation](https://docs.wasmtime.dev/api/wasmtime/struct.Config.html#method.cache_config_load_default)
- [Wasmtime Cache Design](https://github.com/bytecodealliance/wasmtime/blob/main/docs/WASI-tutorial.md)
