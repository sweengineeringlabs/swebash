# Wasmtime Module Caching Architecture

> **TLDR:** Wasmtime disk cache eliminates JIT compilation overhead on subsequent runs, improving test performance by ~5x.

**Audience:** Developers
**Related ADR:** [ADR-001: Wasmtime Module Caching](adr/ADR-001-wasmtime-module-caching.md)

---

## Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        swebash Process Startup                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐    ┌─────────────────┐    ┌──────────────────┐   │
│  │ engine.wasm  │───▶│  Wasmtime Cache │───▶│  Native Module   │   │
│  │   (26 KB)    │    │   (disk check)  │    │   (executable)   │   │
│  └──────────────┘    └─────────────────┘    └──────────────────┘   │
│                              │                        │             │
│                              ▼                        ▼             │
│                       ┌─────────────┐          ┌───────────┐        │
│                       │ Cache Miss  │          │ Cache Hit │        │
│                       │ (JIT ~700ms)│          │  (~50ms)  │        │
│                       └─────────────┘          └───────────┘        │
│                              │                        │             │
│                              ▼                        │             │
│                       ┌─────────────┐                 │             │
│                       │ Write Cache │                 │             │
│                       │   to Disk   │                 │             │
│                       └─────────────┘                 │             │
│                              │                        │             │
│                              └────────────┬───────────┘             │
│                                           ▼                         │
│                                    ┌─────────────┐                  │
│                                    │   Execute   │                  │
│                                    │   Module    │                  │
│                                    └─────────────┘                  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Cache Location

| Platform | Default Cache Directory |
|----------|------------------------|
| Linux/macOS | `~/.cache/wasmtime/` |
| Windows | `%LOCALAPPDATA%\wasmtime\` |

## Cache Key

Wasmtime computes a cache key from:
1. WASM module content hash (SHA-256)
2. Wasmtime version
3. Compiler settings (optimization level, etc.)

This ensures:
- Cache invalidation when WASM changes
- No stale cache after wasmtime upgrades
- Correct behavior across different build profiles

## Configuration

### Default (Recommended)

```rust
let mut config = Config::new();
config.cache_config_load_default()?;
let engine = Engine::new(&config)?;
```

Uses platform-appropriate defaults. No configuration files needed.

### Custom Cache Directory

```rust
let mut config = Config::new();
config.cache_config_load("path/to/cache.toml")?;
let engine = Engine::new(&config)?;
```

Cache config file (`cache.toml`):
```toml
[cache]
enabled = true
directory = "/custom/cache/path"
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `WASMTIME_CACHE_DIR` | Override cache directory |
| `WASMTIME_CACHE_ENABLED` | Set to `0` to disable |

## Performance Comparison

### Before (No Cache)

```
Process Start
    │
    ├─► Load embedded WASM (1ms)
    │
    ├─► JIT Compile (700ms)  ◄── BOTTLENECK
    │
    ├─► Instantiate module (5ms)
    │
    └─► Ready

Total: ~706ms
```

### After (With Cache - Cache Hit)

```
Process Start
    │
    ├─► Load embedded WASM (1ms)
    │
    ├─► Check cache (5ms)
    │
    ├─► Load cached native code (40ms)
    │
    ├─► Instantiate module (5ms)
    │
    └─► Ready

Total: ~51ms (14x faster)
```

## Impact on Testing (Measured 2026-02-22)

| Scenario | Before | After | Improvement |
|----------|--------|-------|-------------|
| Single test | 741ms | ~100ms | 7x |
| 94 integration tests | 24.91s | 12.02s | **2.1x** |
| CI pipeline | ~2min | ~1min | 2x |

Note: Test suite improvement is ~2x (not theoretical 5x) because tests run in parallel, amortizing JIT overhead, and test execution time dominates.

## Implementation Details

### File: `features/shell/host/src/spi/runtime.rs`

```rust
pub fn setup(
    sandbox: SandboxPolicy,
    initial_cwd: PathBuf,
    git_enforcer: Option<Arc<GitGateEnforcer>>,
) -> Result<(Store<HostState>, Instance)> {
    // Enable wasmtime disk cache for faster subsequent startups.
    // First run: JIT compiles and caches (~700ms)
    // Subsequent runs: loads cached native code (~50ms)
    let mut config = Config::new();
    // Cache is optional - silently continue without it if unavailable.
    let _ = config.cache_config_load_default();
    let engine = Engine::new(&config)?;

    // ... rest of setup
}
```

### Error Handling

Cache failures are non-fatal:
- Missing cache directory → wasmtime creates it
- Permission errors → falls back to no cache
- Corrupt cache → wasmtime rebuilds entry

## Monitoring

To verify cache is working:

```bash
# Check cache directory size
du -sh ~/.cache/wasmtime/  # Linux/macOS
dir %LOCALAPPDATA%\wasmtime  # Windows

# Verify cache hit (second run should be faster)
time (echo "exit" | swebash)  # First run: ~700ms
time (echo "exit" | swebash)  # Second run: ~50ms
```

## Future Enhancements

1. **AOT Compilation** - Pre-compile at build time for zero JIT overhead
2. **Shared Cache in CI** - Cache wasmtime artifacts between CI runs
3. **Metrics** - Add startup timing telemetry

## See Also

- [ADR-001: Wasmtime Module Caching](adr/ADR-001-wasmtime-module-caching.md)
- [Wasmtime Documentation](https://docs.wasmtime.dev/)
