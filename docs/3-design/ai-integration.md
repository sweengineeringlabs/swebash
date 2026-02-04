# AI Integration Design

## Isolation Boundary

The `AiClient` trait in `spi/mod.rs` is the isolation boundary between swebash-ai and the LLM HTTP APIs.

```
swebash-ai code          │  External APIs
                          │
  AiService trait         │
  DefaultAiService        │
  core/* modules          │
  AiClient trait  ────────│──→  LlmProviderClient (spi/llm_provider.rs)
                          │       └── reqwest HTTP client
                          │       └── OpenAI, Anthropic, Gemini API calls
```

**Only `spi/llm_provider.rs` makes HTTP calls to LLM APIs.** All other modules program against `AiClient`.

## Current Implementation

The `LlmProviderClient` uses `reqwest` to call provider APIs directly:

```rust
pub struct LlmProviderClient {
    http: reqwest::Client,
    provider: String,
    model: String,
    api_key: String,
    base_url: String,
}
```

Each provider has dedicated request/response types and a `complete_*` function:
- `complete_openai` — OpenAI Chat Completions API
- `complete_anthropic` — Anthropic Messages API
- `complete_gemini` — Google Gemini GenerateContent API

## Future: llm-provider Integration

When `llm-provider` from swe-studio is published to a registry, `spi/llm_provider.rs` can be replaced with a thin wrapper:

```rust
// Future version:
pub struct LlmProviderClient {
    service: llm_provider::DefaultLlmService,
    model: String,
    provider: String,
}
```

The rest of swebash-ai remains unchanged — only the SPI implementation file changes.

## Type Conversions

`llm_provider.rs` converts between swebash-ai types and provider API formats:

| swebash-ai | Provider API |
|------------|-------------|
| `AiRole::System` | `"system"` (or separate field for Anthropic/Gemini) |
| `AiRole::User` | `"user"` |
| `AiRole::Assistant` | `"assistant"` / `"model"` (Gemini) |
| `AiMessage { role, content }` | Provider-specific message format |
| `CompletionOptions { temperature, max_tokens }` | Provider-specific request fields |
| `AiResponse { content, model }` | Extracted from provider response |

## Provider-Specific Handling

### OpenAI
- System messages included in the `messages` array
- Standard `Authorization: Bearer <key>` header

### Anthropic
- System message extracted to a separate `system` field
- Uses `x-api-key` header and `anthropic-version` header

### Gemini
- System message extracted to `system_instruction` field
- Assistant role mapped to `"model"`
- API key passed as URL query parameter

## Error Mapping

HTTP status codes and response errors are mapped to `AiError` variants:
- HTTP 429 → `AiError::RateLimited`
- Non-success status → `AiError::Provider(message)`
- Parse failures → `AiError::ParseError(message)`
- Connection errors → `AiError::Provider(message)`
