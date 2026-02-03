# Revised Tool Calling Architecture

## The Problem

Original design had tool calling **outside** SimpleChatEngine, which causes:
1. Tool messages not tracked in conversation memory
2. Context window management bypassed
3. Broken abstraction - SimpleChatEngine should own conversation state

## The Solution: ToolAwareChatEngine Wrapper

Create a wrapper that extends SimpleChatEngine's functionality while preserving all its memory and context management.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     DefaultAiService                          │
│  ┌────────────────────────────────────────────────────────┐  │
│  │            ToolAwareChatEngine (NEW)                   │  │
│  │                                                         │  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │         SimpleChatEngine (from rustratify)       │  │  │
│  │  │  - Memory management                             │  │  │
│  │  │  - Context window                                │  │  │
│  │  │  - LLM interaction                               │  │  │
│  │  └──────────────────────────────────────────────────┘  │  │
│  │                                                         │  │
│  │  + ToolRegistry                                        │  │
│  │  + Tool calling loop                                   │  │
│  │  + Tool message management                             │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## ToolAwareChatEngine Implementation

```rust
/// Wrapper around SimpleChatEngine that adds tool calling support
/// while preserving all memory and context management functionality.
pub struct ToolAwareChatEngine {
    /// The underlying chat engine (handles memory, context, LLM calls)
    engine: Arc<SimpleChatEngine>,
    /// Registry of available tools
    tools: Arc<ToolRegistry>,
    /// LLM service (needed for tool-aware requests)
    llm: Arc<dyn LlmService>,
}

impl ToolAwareChatEngine {
    pub fn new(
        engine: SimpleChatEngine,
        tools: ToolRegistry,
        llm: Arc<dyn LlmService>,
    ) -> Self {
        Self {
            engine: Arc::new(engine),
            tools: Arc::new(tools),
            llm,
        }
    }

    /// Send a message with tool calling support
    pub async fn send(
        &self,
        message: ChatMessage,
        events: AgentEventSender,
    ) -> AiResult<ChatResponse> {
        // Start with user message
        let user_msg = Self::to_llm_message(&message);

        // Add to engine's context so it's tracked
        self.engine.memory().add_message(message.clone()).await?;

        let mut iteration = 0;

        loop {
            if iteration >= self.tools.config.max_iterations {
                return Err(AiError::ToolError("Max iterations reached".into()));
            }

            // Build messages from engine's context (includes all history + tool messages)
            let messages = self.build_messages_from_engine();

            // Create completion request WITH tools
            let request = CompletionRequest {
                model: self.engine.config().model.clone(),
                messages,
                temperature: Some(self.engine.config().temperature),
                max_tokens: Some(self.engine.config().max_tokens),
                tools: Some(self.tools.definitions()),
                tool_choice: Some(ToolChoice::Auto),
                ..Default::default()
            };

            // Call LLM
            let response = self.llm.complete(request).await?;

            // Check if there are tool calls
            if response.tool_calls.is_empty() {
                // No tool calls - final response
                let content = response.content.unwrap_or_default();

                // Add assistant response to engine's memory
                let assistant_msg = ChatMessage::assistant(&content);
                self.engine.memory().add_message(assistant_msg).await?;

                return Ok(ChatResponse {
                    reply: content.trim().to_string(),
                });
            }

            // Execute tool calls
            let mut tool_results = Vec::new();
            for tool_call in &response.tool_calls {
                events.send(AgentEvent::Status(format!(
                    "Calling tool: {}",
                    tool_call.name
                ))).await?;

                let result = self.tools
                    .execute(&tool_call.name, &tool_call.arguments)
                    .await;

                let result_content = match result {
                    Ok(content) => content,
                    Err(e) => format!("Error: {}", e),
                };

                tool_results.push((tool_call.id.clone(), result_content));
            }

            // Add tool call results to engine's context as proper messages
            for (tool_call_id, content) in tool_results {
                let tool_msg = Message {
                    role: Role::Tool,
                    content: MessageContent::Text(content),
                    name: None,
                    tool_call_id: Some(tool_call_id),
                    tool_calls: vec![],
                    cache_control: None,
                };

                // Add to engine's context (this is key!)
                self.add_message_to_engine(tool_msg).await?;
            }

            iteration += 1;
        }
    }

    /// Build messages from engine's context
    /// This ensures we use the engine's memory/context window
    fn build_messages_from_engine(&self) -> Vec<Message> {
        // Access engine's context window
        self.engine.context.read().messages().cloned().collect()
    }

    /// Add a message to engine's context
    /// This ensures tool messages are tracked in memory and token counts
    async fn add_message_to_engine(&self, message: Message) -> AiResult<()> {
        // Use engine's add_to_context method
        self.engine.add_to_context(message)
            .map_err(|e| AiError::Provider(e.to_string()))
    }

    fn to_llm_message(msg: &ChatMessage) -> Message {
        Message {
            role: match msg.role {
                ChatRole::System => Role::System,
                ChatRole::User => Role::User,
                ChatRole::Assistant => Role::Assistant,
            },
            content: MessageContent::Text(msg.content.clone()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
            cache_control: None,
        }
    }

    /// Delegate to underlying engine
    pub fn memory(&self) -> Arc<dyn MemoryManager> {
        self.engine.memory()
    }

    pub fn config(&self) -> ChatConfig {
        self.engine.config()
    }

    pub async fn new_conversation(&self) -> AiResult<()> {
        self.engine.new_conversation()
            .await
            .map_err(|e| AiError::Provider(e.to_string()))
    }
}
```

## Key Benefits

### 1. ✅ Memory Consistency
- All messages (user, assistant, tool) go through SimpleChatEngine's memory
- Tool calls are part of conversation history
- Can be exported, imported, persisted

### 2. ✅ Context Window Management
- SimpleChatEngine's context window tracks ALL tokens including tool messages
- Automatic truncation when context is full
- Token counting is accurate

### 3. ✅ Clean Abstraction
- SimpleChatEngine remains the single source of truth
- ToolAwareChatEngine is a thin wrapper that adds functionality
- No code duplication

### 4. ✅ Non-invasive
- SimpleChatEngine from rustratify is unchanged
- Can upgrade rustratify without conflicts
- If rustratify adds tool support later, easy to migrate

## Updated Data Flow

```
User: "What files are in the current directory?"
    │
    ▼
┌───────────────────────────────────────────────┐
│ ToolAwareChatEngine.send()                    │
│                                               │
│ 1. Add user message to engine.memory()       │
│    └─> SimpleChatEngine tracks it            │
└────────────┬──────────────────────────────────┘
             │
             ▼
┌───────────────────────────────────────────────┐
│ 2. Build messages from engine.context         │
│    └─> Includes ALL history with tokens      │
└────────────┬──────────────────────────────────┘
             │
             ▼
┌───────────────────────────────────────────────┐
│ 3. Create CompletionRequest WITH tools        │
│    └─> tools: [filesystem, execute, search]  │
└────────────┬──────────────────────────────────┘
             │
             ▼
┌───────────────────────────────────────────────┐
│ 4. LLM returns tool_calls                     │
│    └─> execute_command("ls -la")             │
└────────────┬──────────────────────────────────┘
             │
             ▼
┌───────────────────────────────────────────────┐
│ 5. Execute tool via ToolRegistry              │
│    └─> CommandExecutorTool.execute()         │
└────────────┬──────────────────────────────────┘
             │
             ▼
┌───────────────────────────────────────────────┐
│ 6. Add tool result to engine.context          │
│    └─> SimpleChatEngine tracks it!           │
│    └─> Token count updated                   │
│    └─> Message in memory                     │
└────────────┬──────────────────────────────────┘
             │
             ▼
         [Loop back to step 2]
             │
             ▼
┌───────────────────────────────────────────────┐
│ 7. Final response (no tool calls)             │
│    └─> Add to engine.memory()                │
│    └─> Return to user                        │
└───────────────────────────────────────────────┘
```

## Integration with DefaultAiService

```rust
pub struct DefaultAiService {
    client: Box<dyn AiClient>,
    config: AiConfig,
    // OLD: chat_engine: Arc<SimpleChatEngine>,
    // NEW: Tool-aware wrapper
    chat_engine: Arc<ToolAwareChatEngine>,
}

impl DefaultAiService {
    pub fn new(
        client: Box<dyn AiClient>,
        simple_engine: SimpleChatEngine,
        tools: ToolRegistry,
        llm: Arc<dyn LlmService>,
        config: AiConfig,
    ) -> Self {
        // Wrap SimpleChatEngine with tool support
        let chat_engine = ToolAwareChatEngine::new(
            simple_engine,
            tools,
            llm,
        );

        Self {
            client,
            config,
            chat_engine: Arc::new(chat_engine),
        }
    }
}

#[async_trait]
impl AiService for DefaultAiService {
    async fn chat(&self, request: ChatRequest) -> AiResult<ChatResponse> {
        self.ensure_ready().await?;

        let message = ChatMessage::user(&request.message);
        let (events, _stream) = react::event_stream(64);

        // Tool calling happens inside the engine!
        self.chat_engine.send(message, events).await
    }

    // ... other methods unchanged ...
}
```

## Updated Factory (lib.rs)

```rust
pub async fn create_ai_service() -> AiResult<DefaultAiService> {
    let config = AiConfig::from_env();

    if !config.enabled {
        return Err(AiError::NotConfigured(
            "AI features disabled (SWEBASH_AI_ENABLED=false)".into(),
        ));
    }

    if !config.has_api_key() {
        return Err(AiError::NotConfigured(format!(
            "No API key found for provider '{}'. Set the appropriate environment variable.",
            config.provider
        )));
    }

    // Create the SPI client (initializes the LLM provider)
    let client = spi::chat_provider::ChatProviderClient::new(&config).await?;
    let llm_service = client.llm_service();

    // Build the chat engine from the shared LLM service
    let chat_config = chat_engine::ChatConfig {
        model: config.model.clone(),
        temperature: 0.5,
        max_tokens: 1024,
        system_prompt: Some(core::prompt::chat_system_prompt()),
        max_history: config.history_size,
        enable_summarization: false,
    };
    let simple_engine = chat_engine::SimpleChatEngine::new(
        llm_service.clone(),
        chat_config,
    );

    // Create tool registry
    let tools = core::tools::create_tool_registry(&config);

    // Wrap with tool support
    Ok(DefaultAiService::new(
        Box::new(client),
        simple_engine,
        tools,
        llm_service,
        config,
    ))
}
```

## Accessing Engine Internals

Since SimpleChatEngine fields might be private, we have two options:

### Option A: Use Public API Only
```rust
// Access through public methods
impl ToolAwareChatEngine {
    fn build_messages_from_engine(&self) -> Vec<Message> {
        // Use memory API
        let messages = self.engine.memory()
            .get_all_messages()
            .await
            .unwrap_or_default();

        messages.into_iter()
            .map(|msg| Self::to_llm_message(&msg))
            .collect()
    }
}
```

### Option B: Add Accessor Methods to SimpleChatEngine (Upstream)
```rust
// In rustratify/ai/llm/agent/chat/src/core/simple.rs
impl SimpleChatEngine {
    /// Get all messages from context (for tool-aware wrappers)
    pub fn get_context_messages(&self) -> Vec<Message> {
        self.context.read().messages().cloned().collect()
    }

    /// Add a message to context (for tool-aware wrappers)
    pub fn add_to_context(&self, message: Message) -> ChatResult<()> {
        // ... existing implementation ...
    }
}
```

**Recommendation**: Option B is cleaner - add these methods to rustratify since it's a local dependency.

## File Structure (Updated)

```
ai/
├── src/
│   ├── core/
│   │   ├── mod.rs              # DefaultAiService
│   │   ├── chat.rs             # Thin wrapper, delegates to ToolAwareChatEngine
│   │   ├── engine.rs (NEW)     # ToolAwareChatEngine implementation
│   │   └── tools/
│   │       ├── mod.rs          # ToolExecutor, ToolRegistry
│   │       ├── fs.rs           # FileSystemTool
│   │       ├── exec.rs         # CommandExecutorTool
│   │       └── web.rs          # WebSearchTool
│   └── lib.rs                  # Updated factory
```

## Comparison: Before vs After

### Before (Broken)
```
User Message
    │
    ▼
chat_with_tools() ──┐
    │               │
    ├─> SimpleChatEngine.send() (only text messages)
    │   └─> Memory: [user, assistant, user, assistant]
    │
    └─> ToolRegistry.execute() (outside engine)
        └─> Tool messages NOT in engine memory ❌
```

### After (Fixed)
```
User Message
    │
    ▼
ToolAwareChatEngine.send()
    │
    ├─> Add user message to engine.memory() ✓
    ├─> Build request with tools from engine.context ✓
    ├─> Execute tools
    ├─> Add tool messages to engine.context ✓
    └─> Add final response to engine.memory() ✓

SimpleChatEngine.memory():
  [user, tool_call, tool_result, assistant, user, ...] ✓
```

## Benefits Summary

| Aspect | Old Design | New Design |
|--------|-----------|------------|
| Tool messages in memory | ❌ No | ✅ Yes |
| Token counting accurate | ❌ No | ✅ Yes |
| Context window respected | ❌ No | ✅ Yes |
| Can export conversation | ❌ Incomplete | ✅ Complete |
| Single source of truth | ❌ Split | ✅ Unified |
| Modifies rustratify | ✅ No | ⚠️ Minor (optional) |

## Migration Path

1. **Phase 1**: Implement ToolAwareChatEngine wrapper using public API only
2. **Phase 2**: Test and validate memory/context management works
3. **Phase 3**: Optionally contribute accessor methods to rustratify
4. **Phase 4**: If rustratify adds native tool support, migrate away from wrapper

This approach is:
- ✅ Clean and maintainable
- ✅ Respects SimpleChatEngine's responsibilities
- ✅ Non-invasive to rustratify
- ✅ Easy to migrate away from later
