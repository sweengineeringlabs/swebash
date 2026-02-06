# Configuration

**Audience**: Users, DevOps

## Environment Variables

All configuration is done via environment variables. No config files are required.

### AI Feature Control

| Variable | Default | Description |
|----------|---------|-------------|
| `SWEBASH_AI_ENABLED` | `true` | Set to `false` or `0` to disable AI features entirely |
| `SWEBASH_AI_HISTORY_SIZE` | `20` | Maximum number of chat history messages to retain |

### LLM Provider Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_PROVIDER` | `openai` | LLM provider: `openai`, `anthropic`, `gemini` |
| `LLM_DEFAULT_MODEL` | *(per provider)* | Model to use for completions |

Default models per provider:
- `openai` → `gpt-4o`
- `anthropic` → `claude-sonnet-4-20250514`
- `gemini` → `gemini-2.0-flash`

### API Keys

| Variable | Provider |
|----------|----------|
| `OPENAI_API_KEY` | OpenAI |
| `ANTHROPIC_API_KEY` | Anthropic |
| `GEMINI_API_KEY` | Google Gemini |

## Quick Start

### OpenAI (default)
```bash
export OPENAI_API_KEY=sk-...
# That's it — OpenAI is the default provider
```

### Anthropic
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export LLM_PROVIDER=anthropic
```

### Gemini
```bash
export GEMINI_API_KEY=AI...
export LLM_PROVIDER=gemini
```

### Disable AI
```bash
export SWEBASH_AI_ENABLED=false
```

## Graceful Degradation

If no API key is set or AI is disabled:
- The shell works normally (all non-AI commands pass through to WASM)
- AI commands print a friendly "not configured" message with setup instructions
- `ai status` shows the current configuration state
- No errors, no crashes, no startup delays
