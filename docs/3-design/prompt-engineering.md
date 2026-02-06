# Prompt Engineering

**Audience**: Developers, architects

## Design Principles

1. **Role clarity**: Each system prompt defines a clear, specific role.
2. **Output format**: Explicit rules about what the LLM should output.
3. **Conciseness**: Prompts are kept minimal to reduce token usage.
4. **Context injection**: Dynamic context (cwd, history) is injected as user messages.

## System Prompts

### Translate (NL → Shell Command)

**Goal**: Return a single, executable shell command with no explanation.

**Temperature**: 0.1 (low — deterministic, precise commands)

**Key rules**:
- Output ONLY the shell command
- No markdown, no backticks, no explanations
- Use standard Unix commands
- Pick most common interpretation when ambiguous

**Example I/O**:
- Input: "list all rust files modified in the last day"
- Output: `find . -name "*.rs" -mtime -1`

### Explain (Command Explanation)

**Goal**: Break down a command into understandable parts.

**Temperature**: 0.3 (slightly creative for clear explanations)

**Key rules**:
- Break down each flag and argument
- Use plain language for intermediate users
- Keep to 3-8 lines
- No markdown code blocks

**Example I/O**:
- Input: "Explain this command: tar -xzf archive.tar.gz"
- Output: Multi-line explanation of tar, -x, -z, -f flags

### Chat (Conversational)

**Goal**: Helpful shell assistant with conversation memory.

**Temperature**: 0.5 (balanced — helpful but not too creative)

**Key rules**:
- Concise and direct
- Reference conversation history
- Shell/computing focused
- Present commands clearly

### Autocomplete (Suggestions)

**Goal**: Return 3-5 likely next commands.

**Temperature**: 0.3

**Key rules**:
- One command per line, no numbering or bullets
- Complete, runnable commands
- Based on partial input, directory contents, and history

## Context Injection Pattern

For translate and autocomplete, dynamic context is injected as a user message:

```
System: [system prompt]
User: Current directory: /home/user/project
      Recent commands: git status, cargo build
Assistant: Understood. I'll use this context for my translation.
User: [actual request]
```

This pattern keeps the system prompt static and cacheable while providing dynamic context.
