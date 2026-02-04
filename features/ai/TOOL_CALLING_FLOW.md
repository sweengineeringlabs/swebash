# Tool Calling Flow Diagrams

## Component Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                          Host Application                             │
│                    (swebash shell / REPL)                            │
└────────────────────────────────┬─────────────────────────────────────┘
                                 │
                                 │ AiService::chat(request)
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│                        DefaultAiService                               │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    chat_with_tools()                          │   │
│  │  (orchestrates tool calling loop)                            │   │
│  └──────────────────────────────────────────────────────────────┘   │
└────────────┬─────────────────────────────────────┬───────────────────┘
             │                                     │
             │                                     │
             ▼                                     ▼
┌────────────────────────┐            ┌──────────────────────────────┐
│   SimpleChatEngine     │            │      ToolRegistry            │
│  (from chat crate)     │            │  ┌────────────────────────┐  │
│                        │            │  │  ToolExecutor impls    │  │
│  - Manages messages    │            │  │  ┌──────────────────┐  │  │
│  - Context window      │            │  │  │ FileSystemTool   │  │  │
│  - Memory              │            │  │  └──────────────────┘  │  │
│  - LLM interaction     │            │  │  ┌──────────────────┐  │  │
│                        │            │  │  │ CommandExecTool  │  │  │
└────────────┬───────────┘            │  │  └──────────────────┘  │  │
             │                        │  │  ┌──────────────────┐  │  │
             │                        │  │  │ WebSearchTool    │  │  │
             ▼                        │  │  └──────────────────┘  │  │
┌────────────────────────┐            │  └────────────────────────┘  │
│    LlmService          │            └──────────────────────────────┘
│  (llm-provider)        │
│                        │
│  - OpenAI              │
│  - Anthropic           │
│  - Gemini              │
└────────────────────────┘
```

## Detailed Tool Calling Flow

```
┌─────────┐
│  User   │
│ Message │
└────┬────┘
     │
     ▼
┌─────────────────────────────────────────────┐
│ Step 1: Initialize Tool Loop                │
│                                             │
│  • Create message list                      │
│  • Set iteration counter = 0                │
│  • Load tool definitions from registry      │
└──────────────────┬──────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────┐
│ Step 2: Send to LLM with Tools              │
│                                             │
│  Request:                                   │
│   {                                         │
│     "model": "gpt-4o",                      │
│     "messages": [...conversation...],       │
│     "tools": [                              │
│       {                                     │
│         "name": "filesystem",               │
│         "description": "...",               │
│         "parameters": { JSONSchema }        │
│       },                                    │
│       ...                                   │
│     ],                                      │
│     "tool_choice": "auto"                   │
│   }                                         │
└──────────────────┬──────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────┐
│ Step 3: LLM Response                        │
│                                             │
│  Response:                                  │
│   {                                         │
│     "content": "Let me check that file...", │
│     "tool_calls": [                         │
│       {                                     │
│         "id": "call_abc123",                │
│         "name": "filesystem",               │
│         "arguments": "{                     │
│           \"operation\": \"read\",          │
│           \"path\": \"/etc/hosts\"          │
│         }"                                  │
│       }                                     │
│     ]                                       │
│   }                                         │
└──────────────────┬──────────────────────────┘
                   │
                   ▼
                ┌──┴──┐
                │ Has │  No ────────────┐
                │tool │                 │
                │calls│                 │
                │  ?  │                 │
                └──┬──┘                 │
                   │Yes                 │
                   ▼                    │
┌─────────────────────────────────────────────┐     │
│ Step 4: Execute Tools                       │     │
│                                             │     │
│  For each tool_call:                        │     │
│                                             │     │
│  1. Validate tool exists                    │     │
│  2. Parse arguments JSON                    │     │
│  3. Check if confirmation needed            │     │
│  4. Execute tool                            │     │
│     ┌─────────────────────────┐             │     │
│     │  FileSystemTool         │             │     │
│     │  .execute(args)         │             │     │
│     │                         │             │     │
│     │  • Validate path        │             │     │
│     │  • Check permissions    │             │     │
│     │  • Read file            │             │     │
│     │  • Return content       │             │     │
│     └─────────────────────────┘             │     │
│  5. Collect result                          │     │
│     {                                       │     │
│       "tool_call_id": "call_abc123",        │     │
│       "content": "127.0.0.1 localhost..."   │     │
│     }                                       │     │
└──────────────────┬──────────────────────────┘     │
                   │                                │
                   ▼                                │
┌─────────────────────────────────────────────┐     │
│ Step 5: Add Tool Results to Conversation    │     │
│                                             │     │
│  messages.push({                            │     │
│    "role": "tool",                          │     │
│    "tool_call_id": "call_abc123",           │     │
│    "content": "127.0.0.1 localhost..."      │     │
│  });                                        │     │
└──────────────────┬──────────────────────────┘     │
                   │                                │
                   ▼                                │
┌─────────────────────────────────────────────┐     │
│ Step 6: Check Iteration Limit               │     │
│                                             │     │
│  iteration++                                │     │
│  if iteration >= max_iterations:            │     │
│    return error                             │     │
│  else:                                      │     │
│    goto Step 2 (loop)                       │     │
└──────────────────┬──────────────────────────┘     │
                   │                                │
                   └────────────────────────────────┘
                                                     │
                                                     ▼
                                          ┌──────────────────┐
                                          │ Step 7: Final    │
                                          │ Response         │
                                          │                  │
                                          │ Return content   │
                                          │ to user          │
                                          └──────────────────┘
```

## Example: Multi-Tool Conversation

```
User: "What's the current directory and what files are in it?"

Step 1: LLM receives message + tool definitions
├─> LLM thinks: "I need to execute 'pwd' and 'ls'"
└─> Returns: tool_calls: [execute_command("pwd"), execute_command("ls -la")]

Step 2: Execute tools in parallel
├─> execute_command("pwd")
│   └─> Result: "/home/user/projects/swebash"
└─> execute_command("ls -la")
    └─> Result: "total 48\ndrwxr-xr-x 5 user user 4096..."

Step 3: Send tool results back to LLM
├─> LLM receives both results
└─> LLM synthesizes response

Step 4: Final response
└─> "You're currently in `/home/user/projects/swebash`. Here are the files:
     - Cargo.toml (main project manifest)
     - src/ (source code directory)
     - README.md (documentation)
     ..."
```

## Tool Execution Details

### FileSystemTool Execution Flow

```
Tool Call: filesystem("read", "/home/user/config.toml")
    │
    ▼
┌──────────────────────────────┐
│ 1. Parse Arguments           │
│    operation = "read"         │
│    path = "/home/user/..."   │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 2. Validate Path             │
│    • Resolve to absolute     │
│    • Check for ../ traversal │
│    • Check blacklist         │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 3. Check File                │
│    • Exists?                 │
│    • Readable?               │
│    • Size < limit?           │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 4. Read Content              │
│    • Open file               │
│    • Read bytes              │
│    • Validate UTF-8          │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 5. Return Result             │
│    Success: file content     │
│    Error: descriptive msg    │
└──────────────────────────────┘
```

### CommandExecutorTool Execution Flow

```
Tool Call: execute_command("find . -name '*.rs' | wc -l")
    │
    ▼
┌──────────────────────────────┐
│ 1. Parse Arguments           │
│    command = "find..."       │
│    timeout = 30 (default)    │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 2. Safety Check              │
│    • Length < limit?         │
│    • Contains dangerous?     │
│    • Needs confirmation?     │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 3. Spawn Process             │
│    • Use tokio::process      │
│    • Set timeout             │
│    • Capture stdout/stderr   │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 4. Wait with Timeout         │
│    • Max 30 seconds          │
│    • Kill if exceeded        │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 5. Return Result             │
│    {                         │
│      "exit_code": 0,         │
│      "stdout": "42\n",       │
│      "stderr": "",           │
│      "duration_ms": 245      │
│    }                         │
└──────────────────────────────┘
```

### WebSearchTool Execution Flow

```
Tool Call: web_search("rust async programming", 5)
    │
    ▼
┌──────────────────────────────┐
│ 1. Parse Arguments           │
│    query = "rust async..."   │
│    num_results = 5           │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 2. Validate Query            │
│    • Length < 500 chars?     │
│    • Valid characters?       │
│    • Rate limit OK?          │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 3. Call Search API           │
│    • DuckDuckGo API          │
│    • HTTP GET request        │
│    • Parse JSON response     │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 4. Format Results            │
│    [                         │
│      {                       │
│        "title": "...",       │
│        "url": "...",         │
│        "snippet": "..."      │
│      },                      │
│      ...                     │
│    ]                         │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│ 5. Return JSON               │
│    Pretty-printed for LLM    │
└──────────────────────────────┘
```

## Error Handling Flow

```
Tool Execution Error
    │
    ▼
┌─────────────────────────────────┐
│ Catch ToolError                 │
│                                 │
│  • InvalidArguments             │
│  • ExecutionFailed              │
│  • PermissionDenied             │
│  • NotFound                     │
└────────┬────────────────────────┘
         │
         ▼
┌─────────────────────────────────┐
│ Format Error Message            │
│                                 │
│  {                              │
│    "error": true,               │
│    "type": "PermissionDenied",  │
│    "message": "Cannot read..."  │
│  }                              │
└────────┬────────────────────────┘
         │
         ▼
┌─────────────────────────────────┐
│ Send to LLM as Tool Result      │
│                                 │
│  LLM sees the error and can:    │
│  • Retry with different args    │
│  • Try alternative approach     │
│  • Explain error to user        │
└─────────────────────────────────┘
```

## Streaming Events Timeline

```
Time  Event
─────────────────────────────────────────────────────────────
0ms   User sends: "Check if package.json exists"
      │
10ms  ├─> ChatStreamEvent::Delta("Let me check")
20ms  ├─> ChatStreamEvent::Delta(" that file")
30ms  ├─> ChatStreamEvent::Delta(" for you")
      │
40ms  ├─> ChatStreamEvent::ToolCallStart
      │    { name: "filesystem", args: "..." }
      │
50ms  ├─> ChatStreamEvent::ToolExecuting
      │    { name: "filesystem" }
      │
200ms ├─> ChatStreamEvent::ToolCallEnd
      │    { name: "filesystem", result: "true" }
      │
250ms ├─> ChatStreamEvent::Delta("Yes, package")
260ms ├─> ChatStreamEvent::Delta(".json exists")
270ms ├─> ChatStreamEvent::Delta(" in the current")
280ms ├─> ChatStreamEvent::Delta(" directory.")
      │
300ms └─> ChatStreamEvent::Done("...")
```

## Concurrent Tool Execution

When LLM requests multiple independent tools:

```
LLM requests: [tool_A, tool_B, tool_C]
    │
    ├─────────┬─────────┬─────────
    │         │         │
    ▼         ▼         ▼
┌────────┐ ┌────────┐ ┌────────┐
│Tool A  │ │Tool B  │ │Tool C  │
│Execute │ │Execute │ │Execute │
│(async) │ │(async) │ │(async) │
└───┬────┘ └───┬────┘ └───┬────┘
    │          │          │
    │          │          │
    └──────────┴──────────┘
               │
               ▼
    join_all(futures) ────> All results ready
               │
               ▼
    Send all results to LLM in single message
```

Benefits:
- Faster overall execution
- Better user experience
- Efficient token usage
