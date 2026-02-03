# Tool System Migration to Rustratify - Design Document

## Executive Summary

This document outlines the plan to migrate swebash-ai's tool implementations to rustratify, making them reusable across the rustratify ecosystem while following the SEA (Stratified Encapsulation Architecture) pattern.

## Current State Analysis

### swebash-ai Tool System

**Location**: `/home/adentic/swebash/ai/src/core/tools/`

**Structure**:
```
ai/src/core/tools/
├── mod.rs           # ToolExecutor trait, ToolRegistry
├── fs.rs            # FileSystemTool implementation
├── exec.rs          # CommandExecutorTool implementation
└── web.rs           # WebSearchTool implementation
```

**ToolExecutor Trait**:
```rust
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, arguments: &str) -> ToolResult<String>;
    fn requires_confirmation(&self) -> bool { false }
    fn describe_call(&self, arguments: &str) -> String;
}
```

**Characteristics**:
- Uses `&str` for arguments (JSON string)
- Returns `String` results (JSON string)
- Simple `ToolError` enum (6 variants)
- HashMap-based registry
- Configuration via `ToolConfig`
- No risk levels

### rustratify Tool Framework

**Location**: `/home/adentic/rustratify/ai/llm/agent/tool/src/lib.rs`

**Structure**:
```
rustratify/ai/llm/agent/tool/
└── src/
    └── lib.rs       # All components in one file (26KB)
```

**Tool Trait**:
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn risk_level(&self) -> RiskLevel;
    async fn execute(&self, args: Value) -> ToolResult<ToolOutput>;
    fn to_definition(&self) -> ToolDefinition;
    fn default_timeout_ms(&self) -> u64 { 30_000 }
    fn requires_confirmation(&self) -> bool { false }
    fn as_any(&self) -> &dyn Any;
}
```

**Characteristics**:
- Uses `serde_json::Value` for arguments
- Returns structured `ToolOutput` (content, success, error, metadata)
- Comprehensive `ToolError` enum (10 variants)
- Vec-based registry
- **5-tier RiskLevel system** (ReadOnly, LowRisk, MediumRisk, HighRisk, Critical)
- `as_any()` for downcasting

## Comparison Matrix

| Aspect | swebash-ai | rustratify | Decision |
|--------|-----------|-----------|----------|
| **Trait Name** | `ToolExecutor` | `Tool` | Use `Tool` (rustratify standard) |
| **Arguments** | `&str` (JSON) | `Value` | Use `Value` (more structured) |
| **Return Type** | `String` | `ToolOutput` | Use `ToolOutput` (richer) |
| **Error Variants** | 6 | 10 | Use rustratify's (more complete) |
| **Risk Classification** | ❌ None | ✅ 5-tier | Adopt RiskLevel |
| **Timeout** | Per-tool config | Default + override | Use rustratify pattern |
| **Registry Storage** | HashMap | Vec | Keep Vec (simpler iteration) |
| **Downcasting** | ❌ None | ✅ `as_any()` | Adopt (useful for testing) |
| **Module Structure** | Multi-file | Single file | **Expand to SEA pattern** |

## Target Architecture

### Phase 1: Restructure rustratify/ai/llm/agent/tool/ with SEA Pattern

```
rustratify/ai/llm/agent/tool/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs              # Facade (re-exports)
    ├── models.rs           # ToolDefinition, ToolOutput
    ├── errors.rs           # ToolError, ToolResult
    ├── traits.rs           # Tool trait
    ├── registry.rs         # ToolRegistry
    └── providers/          # Concrete tool implementations
        ├── mod.rs
        ├── filesystem.rs   # FileSystemTool (from swebash)
        ├── command.rs      # CommandExecutorTool (from swebash)
        └── web.rs          # WebSearchTool (from swebash)
```

**Note**: rustratify's tool crate is foundational (not domain-specific), so it follows simpler structure rather than full SPI/API/Core/Facade split.

### Phase 2: Update swebash-ai to Consume

```
swebash/ai/
├── Cargo.toml          # Add: tool = { path = "../../rustratify/ai/llm/agent/tool" }
└── src/
    ├── lib.rs          # Use rustratify's create_registry()
    ├── core/
    │   └── tools/      # DELETE this directory
    └── spi/
        └── tool_aware_engine.rs  # Use tool::ToolRegistry
```

## Migration Strategy

### Step 1: Preserve rustratify's Tool Framework (No Breaking Changes)

Keep existing:
- `Tool` trait
- `RiskLevel` enum
- `ToolError` enum
- `ToolOutput` struct
- `ToolRegistry` struct

**Rationale**: These are foundational types used by other rustratify crates.

### Step 2: Add Concrete Tool Providers

Create new files in `rustratify/ai/llm/agent/tool/src/providers/`:

#### filesystem.rs

```rust
use crate::{Tool, ToolOutput, ToolResult, ToolError, RiskLevel};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct FileSystemTool {
    max_size: usize,
}

impl FileSystemTool {
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }

    fn validate_path(&self, path: &str) -> ToolResult<PathBuf> {
        // Path validation logic (from swebash-ai/src/core/tools/fs.rs)
    }
}

#[async_trait]
impl Tool for FileSystemTool {
    fn name(&self) -> &str { "filesystem" }

    fn description(&self) -> &str {
        "Read files, list directories, check existence, and get metadata"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["operation", "path"],
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["read", "list", "exists", "metadata"]
                },
                "path": { "type": "string" }
            }
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::ReadOnly  // File system reads are low risk
    }

    async fn execute(&self, args: Value) -> ToolResult<ToolOutput> {
        // Implementation from swebash-ai
        // Parse args, validate, execute, return ToolOutput
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

#### command.rs

```rust
pub struct CommandExecutorTool {
    timeout: u64,
}

#[async_trait]
impl Tool for CommandExecutorTool {
    fn name(&self) -> &str { "execute_command" }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::HighRisk  // Command execution is high risk
    }

    fn requires_confirmation(&self) -> bool {
        true  // Always confirm command execution
    }

    fn default_timeout_ms(&self) -> u64 {
        self.timeout * 1000
    }

    async fn execute(&self, args: Value) -> ToolResult<ToolOutput> {
        // Implementation with dangerous command blocking
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

#### web.rs

```rust
pub struct WebSearchTool {
    client: reqwest::Client,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::LowRisk  // Web search is low risk (read-only, external)
    }

    async fn execute(&self, args: Value) -> ToolResult<ToolOutput> {
        // DuckDuckGo search implementation
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

### Step 3: Add Factory Function

In `rustratify/ai/llm/agent/tool/src/registry.rs`:

```rust
pub struct ToolConfig {
    pub enable_fs: bool,
    pub enable_exec: bool,
    pub enable_web: bool,
    pub fs_max_size: usize,
    pub exec_timeout: u64,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            enable_fs: true,
            enable_exec: true,
            enable_web: true,
            fs_max_size: 1_048_576,  // 1MB
            exec_timeout: 30,         // 30 seconds
        }
    }
}

/// Create a tool registry with standard tools based on configuration
pub fn create_standard_registry(config: &ToolConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    if config.enable_fs {
        registry.register(Box::new(
            providers::FileSystemTool::new(config.fs_max_size)
        ));
    }

    if config.enable_exec {
        registry.register(Box::new(
            providers::CommandExecutorTool::new(config.exec_timeout)
        ));
    }

    if config.enable_web {
        registry.register(Box::new(
            providers::WebSearchTool::new()
        ));
    }

    registry
}
```

### Step 4: Update swebash-ai

#### ai/Cargo.toml

```toml
[dependencies]
# Tool support from rustratify
tool = { path = "../../rustratify/ai/llm/agent/tool" }

# Remove these (now in tool crate):
# reqwest = { version = "0.11", features = ["json"] }
# url = "2.5"
```

#### ai/src/lib.rs

```rust
use tool::{ToolRegistry, create_standard_registry};

pub async fn create_ai_service() -> AiResult<DefaultAiService> {
    let config = AiConfig::from_env();
    let client = spi::chat_provider::ChatProviderClient::new(&config).await?;
    let llm = client.llm_service();

    let chat_config = chat_engine::ChatConfig { /* ... */ };

    let chat_engine: Arc<dyn chat_engine::ChatEngine> = if config.tools.enabled() {
        // Use rustratify's tool creation
        let tool_config = tool::ToolConfig {
            enable_fs: config.tools.enable_fs,
            enable_exec: config.tools.enable_exec,
            enable_web: config.tools.enable_web,
            fs_max_size: config.tools.fs_max_size,
            exec_timeout: config.tools.exec_timeout,
        };

        let tools = create_standard_registry(&tool_config);

        Arc::new(spi::tool_aware_engine::ToolAwareChatEngine::new(
            llm.clone(),
            chat_config,
            Arc::new(tools),
        ))
    } else {
        Arc::new(chat_engine::SimpleChatEngine::new(llm.clone(), chat_config))
    };

    Ok(DefaultAiService::new(Box::new(client), chat_engine, config))
}
```

#### Delete ai/src/core/tools/

```bash
rm -rf ai/src/core/tools/fs.rs
rm -rf ai/src/core/tools/exec.rs
rm -rf ai/src/core/tools/web.rs
```

Keep only:
- `ai/src/core/tools/mod.rs` - But it becomes a thin wrapper or gets removed entirely

### Step 5: Update ToolAwareChatEngine

#### ai/src/spi/tool_aware_engine.rs

```rust
use tool::{ToolRegistry, ToolOutput};

pub struct ToolAwareChatEngine {
    // ...
    tools: Arc<ToolRegistry>,  // From rustratify
}

impl ToolAwareChatEngine {
    async fn execute_tool_calls(
        &self,
        tool_calls: &[ToolCall],
        events: &AgentEventSender,
    ) -> Vec<(String, String)> {
        let mut results = Vec::new();

        for call in tool_calls {
            // Use rustratify's registry
            match self.tools.get(&call.function.name) {
                Some(tool) => {
                    // Parse JSON arguments to Value
                    let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
                        .unwrap_or_else(|_| json!({}));

                    // Execute tool
                    let result = tool.execute(args).await;

                    // Convert ToolOutput back to JSON string
                    let content = match result {
                        Ok(output) => {
                            serde_json::to_string(&output.content).unwrap_or_default()
                        }
                        Err(e) => {
                            format!(r#"{{"error": "{}"}}"#, e)
                        }
                    };

                    results.push((call.id.clone(), content));
                }
                None => {
                    let error = format!(r#"{{"error": "Tool not found: {}"}}"#, call.function.name);
                    results.push((call.id.clone(), error));
                }
            }
        }

        results
    }
}
```

## Risk Mapping

Define risk levels for each tool:

| Tool | Risk Level | Rationale | Requires Confirmation |
|------|-----------|-----------|---------------------|
| **filesystem** | `ReadOnly` | Read-only operations, path validation, size limits | No |
| **execute_command** | `HighRisk` | Can execute arbitrary commands (with blocklist) | Yes |
| **web_search** | `LowRisk` | External read-only API, no local system access | No |

## Testing Strategy

### Unit Tests (in rustratify)

```rust
// rustratify/ai/llm/agent/tool/src/providers/filesystem.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file() {
        let tool = FileSystemTool::new(1_048_576);
        let args = json!({
            "operation": "read",
            "path": "Cargo.toml"
        });

        let result = tool.execute(args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_path_traversal_blocked() {
        let tool = FileSystemTool::new(1_048_576);
        let args = json!({
            "operation": "read",
            "path": "/etc/passwd"
        });

        let result = tool.execute(args).await;
        assert!(result.is_err());
    }
}
```

### Integration Tests (in swebash-ai)

```rust
// ai/tests/tools_integration_test.rs

#[tokio::test]
async fn test_tool_aware_engine_with_filesystem() {
    let config = AiConfig::from_env();
    let service = create_ai_service().await.unwrap();

    // Test that tools are available and work through the engine
}
```

## Benefits of Migration

### For rustratify
1. ✅ Reusable tool implementations across projects
2. ✅ Standard tool providers for common operations
3. ✅ Risk-based tool classification
4. ✅ Comprehensive test coverage

### For swebash-ai
1. ✅ Less code to maintain (tools move to rustratify)
2. ✅ Automatic improvements when rustratify tools are enhanced
3. ✅ Consistent tool behavior across rustratify ecosystem
4. ✅ Better error handling (ToolOutput structure)

### For Other Projects
1. ✅ Can use rustratify tools out of the box
2. ✅ Can extend with custom tools (implement Tool trait)
3. ✅ Can mix standard + custom tools in registry

## Backward Compatibility

### Breaking Changes
- ❌ swebash-ai's `ToolExecutor` trait is replaced
- ❌ Tool implementations are no longer in swebash-ai
- ❌ `ToolRegistry` API changes (`HashMap` → `Vec`, `Arc<dyn ToolExecutor>` → `Box<dyn Tool>`)

### Migration Path
1. Update all code to use rustratify's `tool` crate
2. Replace `ToolExecutor` with `Tool` trait references
3. Update argument parsing from `&str` to `Value`
4. Update result handling from `String` to `ToolOutput`
5. Test thoroughly

### Consumer Impact
- ✅ **No impact on end users** - tool behavior stays the same
- ✅ **No impact on .env configuration** - same environment variables
- ⚠️ **Minor impact on developers** - must use rustratify's Tool trait

## Implementation Checklist

### Phase 1: rustratify Enhancement
- [ ] Create `rustratify/ai/llm/agent/tool/src/providers/` directory
- [ ] Move & adapt `filesystem.rs` from swebash-ai
- [ ] Move & adapt `command.rs` from swebash-ai
- [ ] Move & adapt `web.rs` from swebash-ai
- [ ] Add `ToolConfig` struct
- [ ] Add `create_standard_registry()` factory function
- [ ] Add unit tests for each provider
- [ ] Update rustratify `Cargo.toml` with dependencies (reqwest, url)
- [ ] Run `cargo test` in rustratify

### Phase 2: swebash-ai Refactor
- [ ] Update `ai/Cargo.toml` to depend on rustratify's `tool` crate
- [ ] Delete `ai/src/core/tools/fs.rs`
- [ ] Delete `ai/src/core/tools/exec.rs`
- [ ] Delete `ai/src/core/tools/web.rs`
- [ ] Update `ai/src/lib.rs` to use `create_standard_registry()`
- [ ] Update `ai/src/spi/tool_aware_engine.rs` to use rustratify types
- [ ] Update integration tests
- [ ] Run `cargo test` in swebash-ai
- [ ] Test end-to-end with actual LLM calls

### Phase 3: Documentation
- [ ] Update rustratify tool crate README
- [ ] Update swebash-ai CONFIGURATION.md
- [ ] Document tool extension points
- [ ] Add examples of custom tool implementation

## Timeline

- **Phase 1** (rustratify): ~3-4 hours
- **Phase 2** (swebash-ai): ~2-3 hours
- **Phase 3** (docs): ~1 hour
- **Testing & iteration**: ~2 hours

**Total**: ~8-10 hours

## Open Questions

1. Should we keep `ToolConfig` in swebash-ai or move to rustratify?
   - **Decision**: Move to rustratify as part of `create_standard_registry()`

2. Should rustratify's tool crate follow full SEA pattern (spi/api/core/facade)?
   - **Decision**: No - it's a foundational utility crate, not a domain service

3. What about custom tools users might want to add?
   - **Answer**: They implement `tool::Tool` trait and register with `ToolRegistry`

4. Should we version the migration?
   - **Answer**: No - do it atomically to avoid maintaining two systems

## Conclusion

This migration consolidates tool implementations in rustratify while making swebash-ai leaner and more maintainable. The rustratify Tool trait provides better structure (RiskLevel, ToolOutput) than swebash-ai's ToolExecutor, making this a clear improvement.
