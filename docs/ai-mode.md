# AI Mode Architecture

## Overview

AI Mode is an interactive mode in swebash that allows natural conversation with AI without repeatedly typing the `ai` prefix. It combines smart intent detection with conversational fallback to provide a seamless experience.

## User Experience

### Entering AI Mode

```bash
~/swebash/> ai
Entered AI mode. Type 'exit' or 'quit' to return to shell.
[AI Mode] >
```

### Using AI Mode

```bash
[AI Mode] > how do I compress a directory?
AI: You can use tar with gzip compression:
    tar -czf archive.tar.gz directory/

[AI Mode] > tar -xzf archive.tar.gz
AI: This extracts a gzipped tar archive:
    - tar: archive tool
    - -x: extract
    - -z: decompress gzip
    - -f: specify filename

[AI Mode] > find files larger than 100MB
AI: find . -size +100M -type f
Execute? [Y/n/e]:

[AI Mode] > exit
Exited AI mode.
~/swebash/>
```

### Prompt Indicators

- **Shell mode**: `~/swebash/> ` (green, shows current directory)
- **AI mode**: `[AI Mode] > ` (cyan, indicates AI mode active)
- **Multi-line**: `...> ` (continues for both modes)

## Architecture

### Components

```
┌─────────────────────────────────────────────────────┐
│                    REPL Loop                        │
│                                                     │
│  ┌─────────────┐    ┌──────────────────────────┐  │
│  │  AI Mode    │───>│  parse_ai_mode_command() │  │
│  │  (flag)     │    └──────────────────────────┘  │
│  └─────────────┘              │                    │
│                                │                    │
│                    ┌───────────▼──────────────┐    │
│                    │   Smart Detection        │    │
│                    │                          │    │
│                    │  1. Explicit subcommands │    │
│                    │  2. Command patterns     │    │
│                    │  3. Action verbs         │    │
│                    │  4. Chat fallback        │    │
│                    └───────────┬──────────────┘    │
│                                │                    │
│                    ┌───────────▼──────────────┐    │
│                    │   AiCommand enum         │    │
│                    │                          │    │
│                    │   - Ask (translate)      │    │
│                    │   - Explain              │    │
│                    │   - Chat                 │    │
│                    │   - Status, etc.         │    │
│                    └───────────┬──────────────┘    │
│                                │                    │
│                    ┌───────────▼──────────────┐    │
│                    │   handle_ai_command()    │    │
│                    └──────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

### State Management

The REPL maintains an `ai_mode: bool` flag:

```rust
let mut ai_mode = false;

loop {
    // Determine prompt based on mode
    let prompt = if ai_mode {
        "[AI Mode] > "
    } else {
        "~/path/> "
    };

    // Route commands based on mode
    if ai_mode {
        let ai_cmd = parse_ai_mode_command(&input);
        // Handle in AI mode...
    } else {
        // Check for AI triggers...
    }
}
```

### Mode Transitions

```
┌──────────────┐
│  Shell Mode  │
└──────┬───────┘
       │ Type "ai"
       ▼
┌──────────────┐
│   AI Mode    │
└──────┬───────┘
       │ Type "exit" or "quit"
       ▼
┌──────────────┐
│  Shell Mode  │
└──────────────┘
```

**Key Behaviors:**
- `ai` command in shell mode → Enter AI mode
- `exit` in AI mode → Return to shell mode
- `exit` in shell mode → Quit swebash
- AI mode state persists across multi-line input

## Smart Detection

### Detection Priority

1. **Explicit Subcommands** (highest priority)
   - `ask <text>` → Ask (translate to command)
   - `explain <cmd>` → Explain
   - `chat <text>` → Chat
   - `status`, `suggest`, `history`, `clear` → Respective commands

2. **Exit Commands**
   - `exit`, `quit` → Exit AI mode

3. **Command Patterns** (smart detection)
   - Known commands: `ls`, `grep`, `tar`, etc.
   - Flags: ` -x`, ` --flag`
   - Pipes/redirects: `|`, `>`, `<`
   - **Result**: Explain the command

4. **Action Verbs** (smart detection)
   - Action words: `find`, `list`, `show`, `delete`, etc.
   - **Result**: Translate to command (Ask)

5. **Chat Fallback** (default)
   - Questions: `how do I...`, `what is...`
   - Conversational: `thanks`, `tell me about...`
   - **Result**: Conversational response (Chat)

### Detection Algorithm

#### Command Pattern Detection

```rust
fn looks_like_command(input: &str) -> bool {
    // Check for flags
    if input.contains(" -") || input.contains(" --") {
        return true;
    }

    // Check for pipes/redirects
    if input.contains('|') || input.contains(" > ") {
        return true;
    }

    // Check for unambiguous commands
    let first_word = input.split_whitespace().next();
    if UNAMBIGUOUS_COMMANDS.contains(first_word) {
        return true;
    }

    // Ambiguous words checked with context
    if AMBIGUOUS_COMMANDS.contains(first_word) {
        // Only treat as command if has path-like args
        return looks_like_command_args(rest);
    }

    false
}
```

**Unambiguous Commands:**
- `ls`, `grep`, `tar`, `git`, `docker`, etc.
- These are ALWAYS treated as commands to explain

**Ambiguous Commands:**
- `find`, `kill`, `echo`, `show`
- These require context:
  - `find /home -name "*.txt"` → Command (has path)
  - `find large log files` → Action request (natural language)

#### Action Request Detection

```rust
fn is_action_request(input: &str) -> bool {
    let first_word = input.to_lowercase().split_whitespace().next();

    let action_verbs = [
        "find", "list", "show", "get", "delete", "remove",
        "create", "make", "move", "copy", "search", ...
    ];

    action_verbs.contains(first_word)
}
```

### Examples by Category

**Commands → Explain**
```bash
[AI Mode] > ls -la
            ^^^ unambiguous command
[AI Mode] > tar -xzf file.tar.gz
            ^^^ has flags
[AI Mode] > ps aux | grep node
            ^^^ has pipe
[AI Mode] > find /home -name "*.log"
            ^^^ ambiguous but has path
```

**Actions → Ask (Translate)**
```bash
[AI Mode] > find files larger than 100MB
            ^^^ action verb, no command syntax
[AI Mode] > list running docker containers
            ^^^ action verb
[AI Mode] > show disk usage
            ^^^ action verb
```

**Conversational → Chat**
```bash
[AI Mode] > how do I compress a directory?
            ^^^ question word
[AI Mode] > what's the difference between rm and rm -rf?
            ^^^ question
[AI Mode] > that's helpful, thanks
            ^^^ conversational
[AI Mode] > tell me about pipes
            ^^^ no other pattern matches
```

**Explicit Override**
```bash
[AI Mode] > chat find files
            ^^^ explicit "chat" overrides "find" action detection
[AI Mode] > explain how pipes work
            ^^^ explicit "explain" overrides question detection
[AI Mode] > ask what is the current directory
            ^^^ explicit "ask" forces translation
```

## Implementation Details

### File Structure

```
host/src/
├── ai/
│   ├── mod.rs              # Handler functions, mode transition logic
│   ├── commands.rs         # Parser, smart detection, AiCommand enum
│   └── output.rs           # Display formatting
└── main.rs                 # REPL loop, mode state management
```

### Key Functions

**`parse_ai_mode_command(input: &str) -> AiCommand`**
- Entry point for AI mode command parsing
- Implements smart detection priority
- Always returns an AiCommand (never None)
- Default fallback: Chat

**`looks_like_command(input: &str) -> bool`**
- Detects command patterns
- Returns true for shell commands to explain
- Handles ambiguous cases intelligently

**`is_action_request(input: &str) -> bool`**
- Detects action verbs
- Returns true for natural language actions to translate
- Simple first-word matching

**`handle_ai_command(service, command, history) -> bool`**
- Executes AI commands
- Returns `true` for `EnterMode` (signals REPL to set flag)
- Returns `false` for all other commands

### Enum Definition

```rust
pub enum AiCommand {
    Ask(String),       // Translate NL → command
    Explain(String),   // Explain command
    Chat(String),      // Conversational
    Suggest,           // Autocomplete
    Status,            // Show AI status
    History,           // Show chat history
    Clear,             // Clear chat history
    EnterMode,         // Enter AI mode
    ExitMode,          // Exit AI mode
}
```

### Mode State in REPL

```rust
// In main.rs
let mut ai_mode = false;

loop {
    // Build prompt based on mode
    let prompt = if ai_mode {
        "[AI Mode] > "
    } else {
        format!("{}> ", cwd)
    };

    // Handle exit differently
    if cmd == "exit" {
        if ai_mode {
            ai_mode = false;
            println!("Exited AI mode.");
            continue;
        } else {
            break; // Quit shell
        }
    }

    // Route based on mode
    if ai_mode {
        let ai_cmd = parse_ai_mode_command(&cmd);
        if matches!(ai_cmd, AiCommand::ExitMode) {
            ai_mode = false;
            println!("Exited AI mode.");
        } else {
            handle_ai_command(&service, ai_cmd, &history).await;
        }
    } else {
        if let Some(ai_cmd) = parse_ai_command(&cmd) {
            let should_enter = handle_ai_command(&service, ai_cmd, &history).await;
            if should_enter {
                ai_mode = true;
                println!("Entered AI mode. Type 'exit' or 'quit' to return to shell.");
            }
        } else {
            // Normal shell command
        }
    }
}
```

## Testing

### Test Coverage

**Smart Detection Tests** (`host/src/ai/commands.rs`):
- `test_looks_like_command_known` - Unambiguous commands
- `test_looks_like_command_flags` - Flag detection
- `test_looks_like_command_pipes` - Pipe/redirect detection
- `test_looks_like_command_negative` - Not commands
- `test_is_action_request` - Action verbs
- `test_is_action_request_negative` - Not actions
- `test_smart_detection_command` - Command routing
- `test_smart_detection_action` - Action routing
- `test_smart_detection_chat_fallback` - Chat fallback
- `test_explicit_override` - Explicit subcommand precedence

**Mode Transition Tests**:
- `test_parse_enter_mode` - Entering AI mode
- `test_parse_mode_exit` - Exiting AI mode
- `test_parse_mode_explicit_subcommands` - Subcommands in AI mode

**Total**: 14 tests, all passing

### Running Tests

```bash
cargo test --bin swebash commands
```

## Design Decisions

### Why Smart Detection + Fallback?

**Option 1: Always require subcommands** ❌
- `[AI Mode] > ask find large files`
- `[AI Mode] > explain tar -xzf`
- **Problem**: Still tedious, defeats purpose of AI mode

**Option 2: Default to one command (e.g., chat)** ⚠️
- `[AI Mode] > tar -xzf file.tar.gz` → Sends to chat
- **Problem**: Obvious commands not explained automatically
- User has to say "explain tar -xzf file.tar.gz"

**Option 3: Smart detection + chat fallback** ✅ (Chosen)
- Commands automatically explained
- Actions automatically translated
- Everything else goes to chat
- Natural and intuitive

### Why These Detection Rules?

**Command Detection:**
- Flags (`-x`) are strong indicators of command syntax
- Pipes (`|`) are unambiguous command operators
- Known commands (ls, grep) are almost always shell commands
- Ambiguous words (find, show) need context

**Action Detection:**
- Action verbs (find, list, show) at start indicate intent
- Overridden by command patterns (higher priority)
- Natural language like "find large files" → translate
- Command syntax like "find . -name" → explain

**Chat Fallback:**
- Questions ("how", "what", "why") → conversational
- Ambiguous inputs → conversational (safest default)
- Chat engine has context, can handle follow-ups
- User can still use explicit subcommands

### Handling Edge Cases

**Case 1: "find large files"**
- Could be: Command to explain OR Action to translate
- **Resolution**: No flags/paths → Action request
- **Result**: Translates to `find . -size +100M`

**Case 2: "find . -name *.log"**
- Could be: Command to explain OR Action to translate
- **Resolution**: Has flags → Command pattern
- **Result**: Explains the find command

**Case 3: "how do I find files"**
- Could be: Question OR Action request
- **Resolution**: Question word → Chat
- **Result**: Conversational explanation

**Case 4: "chat find files"**
- Explicit subcommand overrides detection
- **Result**: Conversational response about finding files

## User Guide

### Quick Start

1. **Enter AI mode:**
   ```bash
   ~/swebash/> ai
   [AI Mode] >
   ```

2. **Ask questions:**
   ```bash
   [AI Mode] > how do I list files?
   ```

3. **Get commands:**
   ```bash
   [AI Mode] > find large log files
   ```

4. **Explain commands:**
   ```bash
   [AI Mode] > tar -xzf archive.tar.gz
   ```

5. **Exit AI mode:**
   ```bash
   [AI Mode] > exit
   ~/swebash/>
   ```

### Tips

- **Most natural**: Just type what you want, let smart detection handle it
- **Be specific**: Use explicit subcommands when detection guesses wrong
- **Conversational**: Chat remembers context within AI mode
- **Quick exit**: `Ctrl-D` also exits AI mode

### Common Patterns

**Learning about commands:**
```bash
[AI Mode] > what is tar?
[AI Mode] > how does grep work?
[AI Mode] > tell me about pipes
```

**Getting command suggestions:**
```bash
[AI Mode] > find files modified today
[AI Mode] > list docker containers
[AI Mode] > compress this directory
```

**Explaining existing commands:**
```bash
[AI Mode] > ps aux | grep node
[AI Mode] > find . -type f -mtime -1
[AI Mode] > docker run -it ubuntu bash
```

**Switching between modes:**
```bash
# Use AI mode for multiple related questions
[AI Mode] > how do I compress files?
[AI Mode] > what's the difference between gzip and bzip2?
[AI Mode] > which is faster?
[AI Mode] > exit

# Back to shell for quick commands
~/swebash/> ls -la
~/swebash/> cd documents
```

## Future Enhancements

### Potential Improvements

1. **History-aware hints** - Show AI mode commands in history hints
2. **Context persistence** - Remember conversation across sessions
3. **Multi-turn confirmations** - "yes, but with verbose output"
4. **Learning from usage** - Improve detection based on corrections
5. **Custom triggers** - User-defined detection rules in config

### Configuration Options

Future `.swebashrc` options:
```toml
[ai_mode]
# Default command when ambiguous
default_intent = "chat"  # or "ask", "explain"

# Disable smart detection
smart_detection = true

# Auto-enter AI mode on shell start
auto_enter = false
```

## Summary

AI Mode provides a natural, efficient way to interact with AI assistance in swebash:

✅ **Single command to enter**: Just type `ai`
✅ **Smart detection**: Automatically routes to right command
✅ **Conversational fallback**: Chat handles ambiguous cases
✅ **Explicit override**: Use subcommands when needed
✅ **Easy exit**: Type `exit` or `quit` to return to shell

**Architecture**: REPL state flag → Smart detection → Command routing → AI handlers

**Philosophy**: Natural language first, explicit commands when precision is needed.
