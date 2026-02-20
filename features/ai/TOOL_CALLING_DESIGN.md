# Tool Calling Design for swebash-ai

## Overview

This document outlines the design for adding tool calling capabilities to the swebash AI service, enabling the LLM to execute file system operations, run shell commands, and perform web searches.

## Architecture (SEA Pattern)

```
┌─────────────────────────────────────────────────────────────────┐
│ L5 Facade (lib.rs)                                              │
│  └─ create_ai_service() - now creates ToolRegistry              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│ L4 Core (core/)                                                 │
│  ├─ DefaultAiService - orchestration                            │
│  ├─ chat.rs - chat logic (ENHANCED with tool loop)              │
│  └─ tools/ (NEW)                                                │
│      ├─ mod.rs - ToolRegistry, ToolExecutor trait               │
│      ├─ loop.rs - tool calling loop orchestration               │
│      ├─ fs.rs - FileSystemTool implementation                   │
│      ├─ exec.rs - CommandExecutorTool implementation            │
│      └─ web.rs - WebSearchTool implementation                   │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│ L3 API (api/)                                                   │
│  ├─ AiService trait - no changes needed                         │
│  └─ types.rs - add ToolEvent for streaming                      │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│ L2 SPI (spi/)                                                   │
│  └─ ChatProviderClient - already supports tool calling          │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│ L1 Common (llm-provider from rustratify)                        │
│  └─ ToolDefinition, ToolCall, ToolChoice - already supported    │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. ToolExecutor Trait (core/tools/mod.rs)

```rust
/// Trait for executable tools that can be called by the LLM
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Get the tool definition (name, description, JSON schema)
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments (JSON string)
    /// Returns JSON string result or error
    async fn execute(&self, arguments: &str) -> ToolResult<String>;

    /// Check if this tool requires user confirmation
    fn requires_confirmation(&self) -> bool {
        false
    }

    /// Get a human-readable description of what this tool call will do
    /// Used for confirmation prompts
    fn describe_call(&self, arguments: &str) -> String;
}

pub type ToolResult<T> = Result<T, ToolError>;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Tool not found: {0}")]
    NotFound(String),
}
```

### 2. ToolRegistry (core/tools/mod.rs)

```rust
/// Registry of available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn ToolExecutor>>,
    /// Configuration for tool execution
    config: ToolConfig,
}

#[derive(Debug, Clone)]
pub struct ToolConfig {
    /// Whether to enable file system tools
    pub enable_fs: bool,
    /// Whether to enable command execution tools
    pub enable_exec: bool,
    /// Whether to enable web search tools
    pub enable_web: bool,
    /// Whether to require confirmation for dangerous operations
    pub require_confirmation: bool,
    /// Maximum number of tool calls per chat turn
    pub max_tool_calls_per_turn: usize,
    /// Maximum number of tool iterations (to prevent infinite loops)
    pub max_iterations: usize,
}

impl ToolRegistry {
    pub fn new(config: ToolConfig) -> Self;

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn ToolExecutor>);

    /// Get all tool definitions for LLM request
    pub fn definitions(&self) -> Vec<ToolDefinition>;

    /// Execute a tool by name
    pub async fn execute(&self, name: &str, arguments: &str) -> ToolResult<String>;

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ToolExecutor>>;
}
```

### 3. Tool Calling Loop (core/tools/loop.rs)

```rust
/// Execute a chat request with tool calling support
pub async fn chat_with_tools(
    engine: &SimpleChatEngine,
    registry: &ToolRegistry,
    request: ChatRequest,
    events: AgentEventSender,
) -> AiResult<ChatResponse> {
    let mut iteration = 0;
    let mut messages = vec![ChatMessage::user(&request.message)];

    loop {
        if iteration >= registry.config.max_iterations {
            return Err(AiError::ToolError("Max iterations reached".into()));
        }

        // Send message to LLM with tool definitions
        let response = engine.send_with_tools(
            messages.last().unwrap().clone(),
            registry.definitions(),
            events.clone()
        ).await?;

        // Check if LLM wants to call tools
        if response.tool_calls.is_empty() {
            // No tool calls, return final response
            return Ok(ChatResponse {
                reply: response.message.content.trim().to_string(),
            });
        }

        // Execute tool calls
        let mut tool_results = Vec::new();
        for tool_call in &response.tool_calls {
            // Emit event that tool is being called
            events.send(AgentEvent::Status(format!(
                "Calling tool: {}",
                tool_call.name
            ))).await?;

            // Check if confirmation is needed
            if let Some(tool) = registry.get(&tool_call.name) {
                if tool.requires_confirmation() && registry.config.require_confirmation {
                    // TODO: Implement confirmation prompt
                    // For now, we'll execute without confirmation
                }
            }

            // Execute tool
            let result = registry.execute(&tool_call.name, &tool_call.arguments).await;

            let result_content = match result {
                Ok(content) => content,
                Err(e) => format!("Error: {}", e),
            };

            tool_results.push(ToolCallResult {
                tool_call_id: tool_call.id.clone(),
                content: result_content,
            });
        }

        // Add tool results to conversation
        messages.push(ChatMessage::tool_results(tool_results));

        iteration += 1;
    }
}
```

## Tool Implementations

### 1. File System Tool (core/tools/fs.rs)

**Tool Definition:**
```json
{
  "name": "filesystem",
  "description": "Read, write, list files and directories. Can read file contents, list directory contents, check if files exist, and get file metadata.",
  "parameters": {
    "type": "object",
    "properties": {
      "operation": {
        "type": "string",
        "enum": ["read", "list", "exists", "metadata"],
        "description": "The operation to perform"
      },
      "path": {
        "type": "string",
        "description": "The file or directory path (absolute or relative to cwd)"
      }
    },
    "required": ["operation", "path"]
  }
}
```

**Capabilities:**
- `read` - Read file contents (text files only, with size limit)
- `list` - List directory contents with file types
- `exists` - Check if a path exists
- `metadata` - Get file size, modified time, permissions

**Safety:**
- Read-only operations (no write/delete for initial version)
- Path validation to prevent directory traversal attacks
- File size limits (e.g., max 1MB per read)
- Only text files (UTF-8 validation)
- Blacklist sensitive paths (/etc/passwd, /etc/shadow, etc.)

### 2. Command Executor Tool (core/tools/exec.rs)

**Tool Definition:**
```json
{
  "name": "execute_command",
  "description": "Execute a shell command and return its output. Use this to run terminal commands, check system status, or perform system operations.",
  "parameters": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "The shell command to execute"
      },
      "timeout_seconds": {
        "type": "integer",
        "description": "Maximum execution time in seconds (default: 30)",
        "default": 30
      }
    },
    "required": ["command"]
  }
}
```

**Capabilities:**
- Execute arbitrary shell commands
- Capture stdout and stderr
- Set execution timeout
- Return exit code

**Safety:**
- Configurable timeout (default 30s, max 300s)
- Command length limits
- Output size limits (max 100KB)
- Requires confirmation for potentially dangerous commands:
  - Commands containing `rm`, `dd`, `mkfs`, `format`
  - Commands with sudo/su
  - Commands modifying system files
- Execution in user context (no privilege escalation)

### 3. Web Search Tool (core/tools/web.rs)

**Tool Definition:**
```json
{
  "name": "web_search",
  "description": "Search the web for information. Returns relevant results with titles, URLs, and snippets.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "The search query"
      },
      "num_results": {
        "type": "integer",
        "description": "Number of results to return (default: 5, max: 10)",
        "default": 5
      }
    },
    "required": ["query"]
  }
}
```

**Implementation Options:**
1. **DuckDuckGo API** (recommended - no API key needed)
2. **Google Custom Search API** (requires API key)
3. **Brave Search API** (requires API key)

**Capabilities:**
- Text search with ranking
- Return title, URL, snippet for each result
- Configurable result count

**Safety:**
- Rate limiting
- Query length limits
- Result count limits
- Content filtering (optional)

## Data Flow

### Tool Calling Loop Flow

```
User Message
    │
    ▼
┌─────────────────────────┐
│ chat_with_tools()       │
│ - Add user message      │
└──────────┬──────────────┘
           │
           ▼
┌─────────────────────────┐
│ LLM Request             │◄───────────┐
│ - Messages              │            │
│ - Tool definitions      │            │
│ - Tool choice: auto     │            │
└──────────┬──────────────┘            │
           │                            │
           ▼                            │
┌─────────────────────────┐            │
│ LLM Response            │            │
│ - Text content?         │            │
│ - Tool calls?           │            │
└──────────┬──────────────┘            │
           │                            │
           ├─[No tool calls]────────────┼─> Return final response
           │                            │
           └─[Has tool calls]           │
                  │                     │
                  ▼                     │
           ┌─────────────────┐          │
           │ For each tool:  │          │
           │ 1. Execute      │          │
           │ 2. Get result   │          │
           └────────┬────────┘          │
                    │                   │
                    ▼                   │
           ┌─────────────────┐          │
           │ Add tool results│          │
           │ to messages     │          │
           └────────┬────────┘          │
                    │                   │
                    └───────────────────┘
                    (loop continues)
```

### Streaming Support

For streaming responses with tools:
1. Stream text content normally
2. When tool call starts, emit `AiEvent::ToolCall(name, args)`
3. While tool executes, emit `AiEvent::ToolExecuting(name)`
4. When tool completes, emit `AiEvent::ToolResult(name, result)`
5. Continue streaming LLM response

## Configuration

### Environment Variables

```bash
# Enable/disable tool categories
SWEBASH_AI_TOOLS_FS=true          # File system tools (default: true)
SWEBASH_AI_TOOLS_EXEC=true        # Command execution (default: true)
SWEBASH_AI_TOOLS_WEB=true         # Web search (default: true)

# Safety settings
SWEBASH_AI_TOOLS_CONFIRM=true     # Require confirmation for dangerous ops (default: true)
SWEBASH_AI_TOOLS_MAX_ITER=10      # Max tool loop iterations (default: 10)

# Tool-specific settings
SWEBASH_AI_FS_MAX_SIZE=1048576    # Max file read size in bytes (default: 1MB)
SWEBASH_AI_EXEC_TIMEOUT=30        # Command timeout in seconds (default: 30)
SWEBASH_AI_WEB_PROVIDER=duckduckgo # Web search provider (default: duckduckgo)
```

### Config Struct (config.rs)

```rust
#[derive(Debug, Clone)]
pub struct AiConfig {
    // ... existing fields ...

    /// Tool configuration
    pub tools: ToolConfig,
}

impl AiConfig {
    pub fn from_env() -> Self {
        // ... existing code ...

        let tools = ToolConfig {
            enable_fs: env::var("SWEBASH_AI_TOOLS_FS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_exec: env::var("SWEBASH_AI_TOOLS_EXEC")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_web: env::var("SWEBASH_AI_TOOLS_WEB")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            require_confirmation: env::var("SWEBASH_AI_TOOLS_CONFIRM")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            max_tool_calls_per_turn: 10,
            max_iterations: env::var("SWEBASH_AI_TOOLS_MAX_ITER")
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        };

        Self {
            // ... existing fields ...
            tools,
        }
    }
}
```

## File Structure

```
ai/
├── src/
│   ├── api/
│   │   ├── mod.rs           # AiService trait
│   │   ├── types.rs         # Add AiEvent::ToolCall variants
│   │   └── error.rs         # Add ToolError variant
│   ├── core/
│   │   ├── mod.rs           # DefaultAiService
│   │   ├── chat.rs          # Update to use chat_with_tools
│   │   └── tools/           # NEW
│   │       ├── mod.rs       # ToolExecutor, ToolRegistry, ToolConfig
│   │       ├── loop.rs      # chat_with_tools() implementation
│   │       ├── fs.rs        # FileSystemTool
│   │       ├── exec.rs      # CommandExecutorTool
│   │       └── web.rs       # WebSearchTool
│   ├── spi/
│   │   └── chat_provider.rs # No changes needed
│   ├── config.rs            # Add ToolConfig
│   └── lib.rs               # Update create_ai_service() to build ToolRegistry
└── Cargo.toml               # Add dependencies: reqwest, serde_json
```

## Security Considerations

### 1. File System Safety
- **Read-only by default** - No write/delete operations in v1
- **Path validation** - Prevent `../` traversal attacks
- **Size limits** - Prevent memory exhaustion
- **Sensitive path blacklist** - Block `/etc/passwd`, SSH keys, etc.
- **UTF-8 validation** - Only read text files

### 2. Command Execution Safety
- **Timeout enforcement** - Prevent infinite loops
- **Output size limits** - Prevent memory exhaustion
- **Confirmation for dangerous commands** - Interactive prompts
- **No privilege escalation** - Run in user context
- **Command sanitization** - Validate input

### 3. Web Search Safety
- **Rate limiting** - Prevent API abuse
- **Query validation** - Length and content limits
- **Result filtering** - Optional content filtering
- **No direct URL fetching** - Only search results (fetch in v2)

### 4. General Safety
- **Iteration limits** - Prevent infinite tool loops
- **Tool call limits** - Max calls per turn
- **Audit logging** - Log all tool executions
- **Error handling** - Graceful failures, clear messages

## API Changes

### Enhanced AiEvent (api/types.rs)

```rust
#[derive(Debug, Clone)]
pub enum AiEvent {
    /// A partial content delta (token chunk)
    Delta(String),
    /// Stream complete — contains the full assembled reply
    Done(String),
    /// Tool call initiated (NEW)
    ToolCallStart { name: String, arguments: String },
    /// Tool execution in progress (NEW)
    ToolExecuting { name: String },
    /// Tool execution completed (NEW)
    ToolCallEnd { name: String, result: String },
}
```

## System Prompt Updates

Add tool usage instructions to `core/prompt.rs`:

```rust
pub fn chat_system_prompt() -> String {
    r#"You are a helpful AI assistant integrated into the swebash shell.

You have access to the following tools:
- filesystem: Read files, list directories, check file existence
- execute_command: Run shell commands and see their output
- web_search: Search the web for information

When you need information from the file system, execute commands, or search online,
use the appropriate tool. Always explain what you're doing and why.

For command execution:
- Prefer safe, read-only commands when possible
- Explain what the command does before executing it
- If a command might be dangerous, describe the risks

Be concise but informative in your responses."#
    .to_string()
}
```

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1)
- [ ] Create `core/tools/mod.rs` with traits and registry
- [ ] Create `core/tools/loop.rs` with tool calling loop
- [ ] Update `AiConfig` with tool configuration
- [ ] Update `create_ai_service()` to create ToolRegistry
- [ ] Add new event types to `AiEvent`

### Phase 2: File System Tool (Week 1)
- [ ] Implement `FileSystemTool` with read/list/exists operations
- [ ] Add path validation and safety checks
- [ ] Write unit tests for path traversal prevention
- [ ] Write integration tests with real file system

### Phase 3: Command Execution Tool (Week 2)
- [ ] Implement `CommandExecutorTool` with timeout
- [ ] Add dangerous command detection
- [ ] Implement confirmation prompt mechanism
- [ ] Write unit tests for command sanitization
- [ ] Write integration tests with safe commands

### Phase 4: Web Search Tool (Week 2)
- [ ] Choose and integrate search provider (DuckDuckGo recommended)
- [ ] Implement `WebSearchTool` with rate limiting
- [ ] Add result parsing and formatting
- [ ] Write unit tests for API integration
- [ ] Write integration tests with real searches

### Phase 5: Streaming & Polish (Week 3)
- [ ] Implement streaming support for tool calls
- [ ] Update system prompts
- [ ] Add comprehensive error handling
- [ ] Add audit logging for tool executions
- [ ] Write end-to-end tests

### Phase 6: Documentation & Examples (Week 3)
- [ ] Write user documentation
- [ ] Create example use cases
- [ ] Add troubleshooting guide
- [ ] Update README with tool calling info

## Testing Strategy

### Unit Tests
- Tool execution logic
- Path validation
- Command sanitization
- JSON schema validation

### Integration Tests
- Full tool calling loop
- Each tool with real backends
- Error handling and recovery
- Iteration limits

### End-to-End Tests
- Real conversations with tool usage
- Multi-tool scenarios
- Streaming with tools
- Edge cases and failure modes

## Metrics & Observability

Track:
- Tool call frequency per tool
- Tool execution duration
- Tool success/failure rates
- Iteration depth distribution
- Token usage with tools vs without

## Future Enhancements (v2)

1. **Write Operations**
   - File creation/modification with user approval
   - Safe sandboxed environment

2. **HTTP Tool**
   - Fetch URLs directly
   - API calls
   - Web scraping (with limits)

3. **Git Operations**
   - Read repository info
   - View diffs
   - Safe git operations

4. **Database Queries**
   - SQL query execution (read-only)
   - Connection management

5. **System Information**
   - Process monitoring
   - System resource usage
   - Network information

6. **Custom Tools**
   - Plugin system for user-defined tools
   - Tool marketplace

## References

- OpenAI Function Calling: https://platform.openai.com/docs/guides/function-calling
- Anthropic Tool Use: https://docs.anthropic.com/claude/docs/tool-use
- LangChain Tools: https://python.langchain.com/docs/modules/agents/tools/
