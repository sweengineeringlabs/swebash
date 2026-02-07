# Command Design

> **TLDR:** AI command triggers, parsing rules, and dispatch flows for ask/explain/chat/suggest modes.

**Audience**: Developers, architects

## Table of Contents

- [AI Command Triggers](#ai-command-triggers)
- [Parsing Rules](#parsing-rules)
- [Ask Flow (with confirmation)](#ask-flow-with-confirmation)
- [Explain Flow](#explain-flow)
- [Chat Flow](#chat-flow)
- [Suggest Flow](#suggest-flow)


## AI Command Triggers

All AI commands are intercepted in the host REPL **before** reaching the WASM engine.

### Full Commands

| Command | Action | Example |
|---------|--------|---------|
| `ai ask <text>` | Translate NL to shell command | `ai ask list all rust files modified today` |
| `ai explain <cmd>` | Explain a command | `ai explain find . -name "*.rs" -mtime -1` |
| `ai chat <text>` | Conversational assistant | `ai chat what is a symlink?` |
| `ai suggest` | Autocomplete suggestions | `ai suggest` |
| `ai status` | Show AI configuration | `ai status` |
| `ai history` | Show chat history | `ai history` |
| `ai clear` | Clear chat history | `ai clear` |

### Shorthand Commands

| Shorthand | Equivalent | Example |
|-----------|-----------|---------|
| `? <text>` | `ai ask <text>` | `? files bigger than 1MB` |
| `?? <cmd>` | `ai explain <cmd>` | `?? tar -xzf archive.tar.gz` |

## Parsing Rules

1. Input is trimmed before parsing.
2. `??` is checked before `?` to avoid ambiguity.
3. The `ai` prefix requires a space before the subcommand.
4. Subcommands that take arguments (`ask`, `explain`, `chat`) require non-empty text.
5. Commands without arguments (`suggest`, `status`, `history`, `clear`) match exactly.

## Ask Flow (with confirmation)

```
user> ? list all python files
[ai] thinking...
  find . -name "*.py"

  Execute? [Y/n/e(dit)]
```

- **Y / Enter**: Execute the command
- **n**: Cancel
- **e**: Show the command for manual editing

## Explain Flow

```
user> ?? find . -name "*.rs" -mtime -1

  find . -name "*.rs" -mtime -1

  This command searches for files recursively starting from the current
  directory (.) that match the pattern "*.rs" (Rust source files) and
  were modified within the last day (-mtime -1).

  - find .         : Start searching from current directory
  - -name "*.rs"   : Match files ending in .rs
  - -mtime -1      : Modified less than 1 day ago
```

## Chat Flow

```
user> ai chat what is a symlink?

  A symbolic link (symlink) is a file that points to another file or
  directory. It acts as a shortcut. You can create one with:
    ln -s target link_name
```

## Suggest Flow

```
user> ai suggest

Suggestions:
  1) ls -la
  2) git status
  3) cargo build
  4) cat README.md
```
