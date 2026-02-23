# Tool Calling Architecture - Correct SEA Pattern

## SEA Pattern Layers (Rustratify Standard)

```
L4: Facade      lib.rs - re-exports public API
L3: Core        core/ - ALL concrete implementations
L2: API         api/ - trait contracts (consumer interface)
L1: SPI         spi/ - extension points + foundation types
```

## Key Principles

1. **SPI = Extensibility** (many implementations possible)
2. **API = Abstraction** (clean consumer interface)
3. **Core = Implementation** (concrete types)
4. **Facade = Entry point** (re-exports)

## Current swebash-ai Architecture

```
ai/
├── src/
│   ├── lib.rs            # L4 Facade - re-exports, factory
│   ├── api/              # L2 API
│   │   ├── mod.rs        # AiService trait (consumer interface)
│   │   ├── types.rs      # Request/Response types
│   │   └── error.rs      # AiError, AiResult
│   ├── spi/              # L1 SPI
│   │   ├── mod.rs        # Re-exports chat_engine::ChatEngine
│   │   └── chat_provider.rs  # ChatProviderClient (wraps llm-provider)
│   ├── core/             # L3 Core
│   │   ├── mod.rs        # DefaultAiService (implements AiService)
│   │   ├── chat.rs       # chat() function delegates to engine
│   │   ├── translate.rs  # translate() implementation
│   │   ├── explain.rs    # explain() implementation
│   │   ├── complete.rs   # autocomplete() implementation
│   │   └── prompt.rs     # System prompts
│   └── config.rs         # AiConfig (shared)
```

## Problem: ChatEngine is External SPI

`ChatEngine` trait comes from `chat_engine` crate (rustratify), not defined in swebash-ai.

```rust
// From rustratify/ai/llm/agent/chat
pub trait ChatEngine {
    async fn send(&self, message: ChatMessage, events: AgentEventSender) -> ChatResult<ChatResponse>;
    async fn send_streaming(&self, ...) -> ChatResult<ChatResponse>;
    fn memory(&self) -> Arc<dyn MemoryManager>;
    fn config(&self) -> ChatConfig;
    // ...
}

// Implementations (also in rustratify)
pub struct SimpleChatEngine { ... }
```

**Current state:**
- `ChatEngine` is an external SPI (from rustratify)
- `SimpleChatEngine` is an external provider (from rustratify)
- swebash-ai **uses** this SPI but doesn't define it

## Correct Approach: Add ToolAwareChatEngine Provider

Since `ChatEngine` is the SPI (extension point), we add a **new provider implementation** in swebash-ai.

### Architecture with Tool Support

```
┌──────────────────────────────────────────────────────────────────┐
│ L4: Facade (lib.rs)                                              │
│  └─ create_ai_service() - Factory decides which engine           │
└──────────────────────────────────────────────────────────────────┘
                              │
┌──────────────────────────────────────────────────────────────────┐
│ L3: Core (core/)                                                 │
│  ├─ DefaultAiService - implements AiService API                  │
│  │   └─ engine: Arc<dyn ChatEngine>  ← uses SPI                  │
│  └─ tools/ (NEW)                                                 │
│      ├─ mod.rs - ToolRegistry, ToolExecutor trait               │
│      ├─ fs.rs - FileSystemTool                                  │
│      ├─ exec.rs - CommandExecutorTool                           │
│      └─ web.rs - WebSearchTool                                  │
└──────────────────────────────────────────────────────────────────┘
                              │
┌──────────────────────────────────────────────────────────────────┐
│ L2: API (api/)                                                   │
│  └─ AiService trait - consumer interface                         │
└──────────────────────────────────────────────────────────────────┘
                              │
┌──────────────────────────────────────────────────────────────────┐
│ L1: SPI (spi/)                                                   │
│  ├─ ChatProviderClient (existing)                               │
│  └─ tool_aware_engine.rs (NEW)                                  │
│      └─ ToolAwareChatEngine: ChatEngine ← implements external SPI│
└──────────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │                   │
┌───────────────────┴────────┐  ┌──────┴─────────────────┐
│  External Crate            │  │  External Crate        │
│  chat_engine::ChatEngine   │  │  llm_provider          │
│  (SPI trait - rustratify)  │  │  (Tool support types)  │
│                            │  │                        │
│  SimpleChatEngine          │  │  ToolDefinition        │
│  (Provider - no tools)     │  │  ToolCall              │
└────────────────────────────┘  └────────────────────────┘
```

## File Structure

```
ai/
├── src/
│   ├── lib.rs                      # L4 Facade
│   ├── api/                        # L2 API (Consumer Interface)
│   │   ├── mod.rs                  # AiService trait
│   │   ├── types.rs                # Request/Response types
│   │   └── error.rs                # AiError, AiResult
│   ├── spi/                        # L1 SPI (Extension Points)
│   │   ├── mod.rs                  # Re-exports
│   │   ├── chat_provider.rs        # Existing
│   │   └── tool_aware_engine.rs    # NEW: ToolAwareChatEngine provider
│   ├── core/                       # L3 Core (Implementation)
│   │   ├── mod.rs                  # DefaultAiService
│   │   ├── chat.rs                 # Delegates to engine
│   │   ├── translate.rs
│   │   ├── explain.rs
│   │   ├── complete.rs
│   │   ├── prompt.rs
│   │   └── tools/                  # NEW: Tool implementations
│   │       ├── mod.rs              # ToolRegistry, ToolExecutor trait
│   │       ├── fs.rs               # FileSystemTool
│   │       ├── exec.rs             # CommandExecutorTool
│   │       └── web.rs              # WebSearchTool
│   └── config.rs                   # Shared config
```

## Layer Breakdown

### L1: SPI (Extension Points)

**Purpose**: Extension points for pluggable implementations

**spi/tool_aware_engine.rs** (NEW):
```rust
use chat_engine::{ChatEngine, ChatMessage, ChatResponse, ChatConfig};
use llm_provider::{LlmService, ToolDefinition, ToolCall, ToolChoice};

/// Tool-aware implementation of ChatEngine SPI
///
/// This is a PROVIDER that implements the external ChatEngine trait
/// from rustratify's chat_engine crate. It adds tool calling support
/// while maintaining all standard ChatEngine capabilities.
pub struct ToolAwareChatEngine {
    // Standard ChatEngine state
    conversation_id: String,
    llm: Arc<dyn LlmService>,
    memory: Arc<InMemoryManager>,
    context: RwLock<ContextWindow>,
    config: RwLock<ChatConfig>,
    state: RwLock<EngineState>,
    conversation_state: RwLock<ConversationState>,
    metrics: RwLock<ToolAwareMetrics>,

    // Tool-specific additions
    tools: Arc<ToolRegistry>,
}

#[async_trait]
impl ChatEngine for ToolAwareChatEngine {
    async fn send(
        &self,
        message: ChatMessage,
        events: AgentEventSender,
    ) -> ChatResult<ChatResponse> {
        // Implementation with tool calling loop
        // See full implementation in TOOL_CALLING_PROVIDER_PATTERN.md
    }

    // ... other ChatEngine methods ...
}
```

**Why SPI?**
- Multiple implementations of ChatEngine possible (SimpleChatEngine, ToolAwareChatEngine, future variants)
- Allows third parties to add custom engines
- Runtime swapping based on config
- Classic provider pattern

### L2: API (Consumer Interface)

**Purpose**: Clean, stable interface for consumers

**api/mod.rs** (UNCHANGED):
```rust
#[async_trait]
pub trait AiService: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse>;
    async fn chat_streaming(&self, ...) -> AiResult<Receiver<AiEvent>>;
    async fn translate(&self, ...) -> AiResult<TranslateResponse>;
    async fn explain(&self, ...) -> AiResult<ExplainResponse>;
    async fn autocomplete(&self, ...) -> AiResult<AutocompleteResponse>;
    async fn is_available(&self) -> bool;
    async fn status(&self) -> AiStatus;
}
```

**Why API?**
- Consumers use this interface
- ONE implementation (DefaultAiService)
- High-level operations
- Stable contract

### L3: Core (Implementation)

**Purpose**: ALL concrete implementations

**core/mod.rs**:
```rust
/// Implementation of AiService API
pub struct DefaultAiService {
    client: Box<dyn AiClient>,
    config: AiConfig,
    /// Uses ChatEngine SPI - doesn't know which provider
    chat_engine: Arc<dyn ChatEngine>,
}

impl DefaultAiService {
    pub fn new(
        client: Box<dyn AiClient>,
        chat_engine: Arc<dyn ChatEngine>,  // ← Polymorphic
        config: AiConfig,
    ) -> Self {
        Self { client, chat_engine, config }
    }
}

#[async_trait]
impl AiService for DefaultAiService {
    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        self.ensure_ready().await?;
        let message = ChatMessage::user(&request.message);
        let (events, _) = react::event_stream(64);

        // Delegates to ChatEngine SPI (could be Simple or ToolAware)
        self.chat_engine.send(message, events).await
            .map_err(|e| AiError::Provider(e.to_string()))
    }

    // ... other methods ...
}
```

**core/tools/mod.rs** (NEW):
```rust
/// SPI for tool implementations (internal extensibility)
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, arguments: &str) -> ToolResult<String>;
    fn requires_confirmation(&self) -> bool { false }
    fn describe_call(&self, arguments: &str) -> String;
}

/// Registry for managing tool providers
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolExecutor>>,
    config: ToolConfig,
}

impl ToolRegistry {
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    pub async fn execute(&self, name: &str, args: &str) -> ToolResult<String> {
        let tool = self.tools.get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(args).await
    }
}
```

**Why Core?**
- Contains DefaultAiService (implements API)
- Contains ToolRegistry (concrete implementation)
- Contains tool implementations (FileSystemTool, etc.)
- Business logic lives here

### L4: Facade (Entry Point)

**Purpose**: Single entry point, re-exports public API

**lib.rs**:
```rust
// Re-export API (consumer interface)
pub use api::{AiService, AiError, AiResult};
pub use api::types::{
    ChatRequest, ChatResponse, TranslateRequest, ExplainRequest, ...
};

// Re-export Core implementation (what consumers actually use)
pub use core::DefaultAiService;
pub use config::AiConfig;

/// Factory: decides which ChatEngine provider to use
pub async fn create_ai_service() -> AiResult<DefaultAiService> {
    let config = AiConfig::from_env();

    if !config.enabled {
        return Err(AiError::NotConfigured("AI disabled".into()));
    }

    let client = spi::chat_provider::ChatProviderClient::new(&config).await?;
    let llm = client.llm_service();

    let chat_config = ChatConfig {
        model: config.model.clone(),
        temperature: 0.5,
        max_tokens: 1024,
        system_prompt: Some(core::prompt::chat_system_prompt()),
        max_history: config.history_size,
        enable_summarization: false,
    };

    // Factory pattern - decide which ChatEngine provider
    let chat_engine: Arc<dyn ChatEngine> = if config.tools.enabled() {
        // Use tool-aware provider
        let tools = core::tools::create_tool_registry(&config);
        Arc::new(spi::ToolAwareChatEngine::new(llm.clone(), chat_config, tools))
    } else {
        // Use simple provider (no tools)
        Arc::new(SimpleChatEngine::new(llm.clone(), chat_config))
    };

    Ok(DefaultAiService::new(Box::new(client), chat_engine, config))
}
```

**Why Facade?**
- Single import path for consumers: `use swebash_ai::*;`
- Hides internal structure
- Factory function decides implementation

## Consumer Usage

```rust
// Consumer code (in host crate)
use swebash_ai::{create_ai_service, AiService, ChatRequest};

#[tokio::main]
async fn main() -> Result<()> {
    // Factory creates appropriate engine based on config
    let ai_service = create_ai_service().await?;

    // Use API - don't care about implementation
    let response = ai_service.chat(ChatRequest {
        message: "What files are in the current directory?".to_string(),
    }).await?;

    println!("{}", response.reply);
    Ok(())
}
```

**Consumer doesn't know:**
- Whether SimpleChatEngine or ToolAwareChatEngine is used
- How tools are executed
- Internal implementation details

**Consumer only sees:**
- `AiService` trait (API)
- Request/Response types
- Factory function

## Key Design Points

### 1. ToolAwareChatEngine is in SPI Layer

**Why SPI?**
- It's a **provider implementation** of the `ChatEngine` trait
- Multiple implementations coexist (Simple, ToolAware, future variants)
- Enables runtime selection based on config
- Classic extensibility pattern

### 2. ToolRegistry is in Core Layer

**Why Core?**
- It's a **concrete implementation**, not a trait
- Used internally by ToolAwareChatEngine
- Not exposed to consumers
- Business logic, not extension point

### 3. ToolExecutor is Internal SPI

**Why internal SPI?**
- Allows multiple tool implementations (FileSystem, Command, Web)
- Contained within swebash-ai (not exposed to consumers)
- Classic plugin pattern for tools

### 4. No Changes to API Layer

**Why unchanged?**
- Tool calling is implementation detail
- API contract remains stable
- Consumers don't need to know about tools
- Breaking no existing code

## Benefits of This Architecture

✅ **Correct SEA Pattern**
- L1 SPI: Extension points (ToolAwareChatEngine implements ChatEngine)
- L2 API: Consumer contracts (AiService)
- L3 Core: Implementations (DefaultAiService, ToolRegistry, tools)
- L4 Facade: Entry point (create_ai_service, re-exports)

✅ **Extensibility through SPI**
- ChatEngine trait allows multiple providers
- ToolExecutor trait allows multiple tool types
- Runtime selection via factory

✅ **Abstraction through API**
- Clean AiService interface for consumers
- Implementation details hidden
- Tool calling transparent to users

✅ **Provider Pattern**
- SimpleChatEngine: No tools
- ToolAwareChatEngine: With tools
- Future: StreamingToolEngine, CachedToolEngine, etc.

✅ **Infrastructure Rule Compliance**
- No tracing/logging in SEA layers
- No retry/circuit breaker logic
- No caching implementation
- Pure structure, delegate infrastructure to rustboot

## Comparison: Before vs After

### Before (Incorrect)
```
ToolAwareChatEngine as wrapper in Core ❌
└─> Breaks SPI pattern
└─> Not a proper provider
└─> Composition instead of implementation
```

### After (Correct)
```
ToolAwareChatEngine as SPI provider ✅
└─> Implements ChatEngine trait
└─> Sits alongside SimpleChatEngine
└─> Selected by factory at runtime
└─> Proper provider pattern
```

## Summary

| Aspect | Value |
|--------|-------|
| **Pattern** | Provider (SPI implementation) |
| **Location** | spi/tool_aware_engine.rs |
| **Implements** | chat_engine::ChatEngine (external SPI) |
| **Used by** | DefaultAiService (Core) |
| **Selected by** | create_ai_service() factory (Facade) |
| **Coexists with** | SimpleChatEngine (another provider) |
| **Tools in** | core/tools/ (internal implementation) |
| **Consumer sees** | AiService API only |

This follows the Rustratify SEA pattern correctly:
- SPI = Extensibility (multiple ChatEngine providers)
- API = Abstraction (clean AiService interface)
- Core = Implementation (DefaultAiService, tools)
- Facade = Entry point (factory, re-exports)
