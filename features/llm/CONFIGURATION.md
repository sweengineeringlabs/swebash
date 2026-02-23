# AI Configuration Guide

## Overview

Swebash AI can be configured via environment variables. For development convenience, you can use a `.env` file in the project root.

## Quick Start

1. Copy the example configuration:
```bash
cp .env.example .env
```

2. Edit `.env` and add your API key:
```bash
# For Anthropic (Claude)
ANTHROPIC_API_KEY=sk-ant-api03-your-actual-key-here

# Or for OpenAI
OPENAI_API_KEY=sk-your-actual-openai-key-here
```

3. Run swebash - it will automatically load `.env`:
```bash
cargo run --release
```

## Configuration Options

### AI Provider

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_AI_ENABLED` | `true` | Enable/disable all AI features |
| `LLM_PROVIDER` | `openai` | Provider: `openai`, `anthropic`, or `gemini` |
| `LLM_DEFAULT_MODEL` | Provider-specific | Model to use (see below) |
| `SWEBASH_AI_HISTORY_SIZE` | `20` | Max chat history messages |

#### Supported Models

**OpenAI:**
- `gpt-4o` (default)
- `gpt-4o-mini`
- `gpt-4-turbo`

**Anthropic:**
- `claude-sonnet-4-20250514` (default)
- `claude-opus-4-20250514`
- `claude-3-5-sonnet-20241022`

**Gemini:**
- `gemini-2.0-flash` (default)
- `gemini-1.5-pro`

### API Keys

Set **only the key for your chosen provider**:

| Variable | Provider |
|----------|----------|
| `OPENAI_API_KEY` | OpenAI (required for `openai` provider) |
| `ANTHROPIC_API_KEY` | Anthropic (required for `anthropic` provider) |
| `GEMINI_API_KEY` | Google (required for `gemini` provider) |

### Tool Calling

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_AI_TOOLS_FS` | `true` | Enable file system tools |
| `SWEBASH_AI_TOOLS_EXEC` | `true` | Enable command execution |
| `SWEBASH_AI_TOOLS_WEB` | `true` | Enable web search |
| `SWEBASH_AI_TOOLS_CONFIRM` | `true` | Require confirmation for dangerous ops |
| `SWEBASH_AI_TOOLS_MAX_ITER` | `10` | Max tool calling loop iterations |
| `SWEBASH_AI_FS_MAX_SIZE` | `1048576` | Max file read size (bytes, 1MB) |
| `SWEBASH_AI_EXEC_TIMEOUT` | `30` | Command timeout (seconds) |

### Tool Details

#### File System Tool (`SWEBASH_AI_TOOLS_FS`)

When enabled, the AI can:
- **Read files**: Access file contents (text files only, up to size limit)
- **List directories**: View directory contents
- **Check existence**: Verify if files/directories exist
- **Get metadata**: File size, modification time, permissions

**Safety:**
- Read-only operations (no write/delete)
- Path validation (blocks `../` traversal)
- Sensitive file blacklist (`/etc/passwd`, SSH keys, etc.)
- Size limits (default 1MB)
- UTF-8 validation (text files only)

#### Command Execution Tool (`SWEBASH_AI_TOOLS_EXEC`)

When enabled, the AI can:
- Execute shell commands
- Capture stdout and stderr
- Return exit codes

**Safety:**
- Configurable timeout (default 30s, max 300s)
- Dangerous command blocking (`rm -rf`, `dd`, `sudo`, etc.)
- Output size limits (100KB)
- Command length limits
- No privilege escalation

**Blocked commands:**
- `rm -rf`, `rm -r`
- `dd if=`, `mkfs`, `format`
- `sudo`, `su`
- `chmod 777`, `chown`
- Fork bombs (`:(){:|:&};:`)

#### Web Search Tool (`SWEBASH_AI_TOOLS_WEB`)

When enabled, the AI can:
- Search the web using DuckDuckGo
- Get instant answers and related topics
- Return titles, URLs, and snippets

**Safety:**
- Rate limiting
- Query length limits (500 chars)
- Result count limits (max 10)
- No direct URL fetching (only search results)

## Environment Variable Priority

Configuration is loaded in this order (later overrides earlier):

1. System environment variables
2. `.env` file (if present)
3. Shell-set variables

Example:
```bash
# .env file has:
LLM_PROVIDER=openai

# Shell override:
LLM_PROVIDER=anthropic cargo run

# Result: Uses anthropic (shell override wins)
```

## Disabling Tools

To disable specific tools:

```bash
# Disable file system access
SWEBASH_AI_TOOLS_FS=false

# Disable command execution
SWEBASH_AI_TOOLS_EXEC=false

# Disable web search
SWEBASH_AI_TOOLS_WEB=false
```

To disable **all tools** (chat-only mode):
```bash
SWEBASH_AI_TOOLS_FS=false
SWEBASH_AI_TOOLS_EXEC=false
SWEBASH_AI_TOOLS_WEB=false
```

Or simply don't set them (defaults to enabled).

## Production Deployment

For production, **do not use `.env` files**. Instead:

1. Set environment variables via your deployment platform
2. Use secrets management (AWS Secrets Manager, HashiCorp Vault, etc.)
3. Never commit `.env` or API keys to version control

Example (Docker):
```dockerfile
ENV ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
ENV SWEBASH_AI_TOOLS_EXEC=false  # Disable dangerous tools in prod
```

Example (systemd):
```ini
[Service]
Environment="ANTHROPIC_API_KEY=sk-ant-..."
Environment="SWEBASH_AI_TOOLS_CONFIRM=true"
```

## Troubleshooting

### AI features not working

1. Check if AI is enabled:
```bash
echo $SWEBASH_AI_ENABLED  # Should be 'true' or unset
```

2. Check if API key is set:
```bash
# For Anthropic
echo $ANTHROPIC_API_KEY | head -c 20  # Should show sk-ant-api03-...

# For OpenAI
echo $OPENAI_API_KEY | head -c 10  # Should show sk-...
```

3. Check provider/model match:
```bash
echo $LLM_PROVIDER  # Should match your API key provider
```

### Tools not working

Check if tools are enabled:
```bash
echo $SWEBASH_AI_TOOLS_FS    # Should be 'true' or unset
echo $SWEBASH_AI_TOOLS_EXEC  # Should be 'true' or unset
echo $SWEBASH_AI_TOOLS_WEB   # Should be 'true' or unset
```

### Command execution failing

1. Check timeout is sufficient:
```bash
export SWEBASH_AI_EXEC_TIMEOUT=60  # Increase to 60 seconds
```

2. Check if command is blocked:
- Commands with `rm`, `sudo`, `dd` are blocked by default

### File reading failing

1. Check file size limit:
```bash
export SWEBASH_AI_FS_MAX_SIZE=10485760  # Increase to 10MB
```

2. Check if file is text:
- Only UTF-8 text files are supported
- Binary files are rejected

## Example Configurations

### Minimal (Chat only, no tools)
```bash
ANTHROPIC_API_KEY=sk-ant-...
SWEBASH_AI_TOOLS_FS=false
SWEBASH_AI_TOOLS_EXEC=false
SWEBASH_AI_TOOLS_WEB=false
```

### Safe (No command execution)
```bash
ANTHROPIC_API_KEY=sk-ant-...
SWEBASH_AI_TOOLS_EXEC=false
```

### Power User (All tools, higher limits)
```bash
ANTHROPIC_API_KEY=sk-ant-...
SWEBASH_AI_TOOLS_MAX_ITER=20
SWEBASH_AI_FS_MAX_SIZE=10485760  # 10MB
SWEBASH_AI_EXEC_TIMEOUT=60
```

### OpenAI Setup
```bash
OPENAI_API_KEY=sk-...
LLM_PROVIDER=openai
LLM_DEFAULT_MODEL=gpt-4o
```

### Gemini Setup
```bash
GEMINI_API_KEY=your-gemini-key
LLM_PROVIDER=gemini
LLM_DEFAULT_MODEL=gemini-2.0-flash
```
