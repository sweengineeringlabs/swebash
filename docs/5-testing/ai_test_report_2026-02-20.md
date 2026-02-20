# AI Manual Test Report — 2026-02-20

**Build**: release (`./sbh build`)
**Binary**: `/tmp/swebash-target/release/swebash`
**Platform**: WSL2 (Linux 6.6.87.2-microsoft-standard-WSL2)
**Provider**: anthropic / claude-sonnet-4-20250514
**API key status**: Credits exhausted — LLM calls return `invalid_request_error`; all non-LLM tests pass.

---

## Results Summary

| Section | Test | Result | Notes |
|---------|------|--------|-------|
| Shell basics | `echo hello world` | ✅ PASS | Printed `hello world` |
| Shell basics | `pwd` | ✅ PASS | Printed `/home/adentic/workspace` |
| Shell basics | `ls` | ✅ PASS | Listed directory entries |
| Shell basics | `exit` | ✅ PASS | Clean exit |
| §7 AI Status | `ai status` | ✅ PASS | enabled=yes, provider=anthropic, model=claude-sonnet-4-20250514, ready=yes |
| §8 AI Ask | `ai ask list all files` | ✅ PASS (error-path) | LLM call made; provider error propagated via `AiEvent::Error` |
| §13 Agent Listing | `ai agents` (shell mode) | ✅ PASS | 11 agents listed, `*shell` marked active |
| §13 Agent Listing | `ai agents` (AI mode) | ✅ PASS | Same table inside AI mode |
| §14 Agent Switching | `@review` | ✅ PASS | Prompt → `[AI:review]`, "Switched to Code Reviewer" |
| §14 Agent Switching | `@git` | ✅ PASS | Prompt → `[AI:git]`, "Switched to Git Assistant" |
| §14 Agent Switching | `@devops` | ✅ PASS | Prompt → `[AI:devops]`, "Switched to DevOps Assistant" |
| §14 Agent Switching | `@shell` | ✅ PASS | Prompt → `[AI:shell]`, "Switched to Shell Assistant" |
| §14b @agent from shell | `@devops` (shell mode) | ✅ PASS | "Switched to DevOps Assistant", "Entered AI mode", prompt `[AI:devops]` |
| §16 Auto-Detection | `docker ps` (AI mode) | ✅ PASS | Auto-switched to devops |
| §16 Auto-Detection | `git rebase` (AI mode) | ✅ PASS | Auto-switched to git |
| §17b History / Clear | `history` (empty) | ✅ PASS | "(no chat history)" |
| §17b History / Clear | `clear` then `history` | ✅ PASS | "Chat history cleared.", then "(no chat history)" |
| §21 Logging | `SWEBASH_AI_LOG_DIR` set | ✅ PASS | Directory auto-created |
| §21 Logging | `ai-complete` file created | ✅ PASS | `*-ai-complete.json` written with `kind: "ai-complete"` |
| §21 Logging | `complete` file created | ✅ PASS | `*-complete.json` written with `kind: "complete"` (LLM layer) |
| §21 Logging | Error logged with status | ✅ PASS | `result.status: "error"`, full provider error message present |
| §21 Logging | Two layers, same request | ✅ PASS | Both `ai-complete` and `complete` files share same `timestamp_epoch_ms` |
| §21 Logging | `ai-complete` messages logged | ✅ PASS | All 4 messages (system, user context, assistant ack, user NL) in `request.messages` |
| §21 Logging | `complete` includes model field | ✅ PASS | `request.model: "claude-sonnet-4-20250514"` present in LLM-layer log |

---

## Logging Detail (§21)

Two files produced by a single `ai ask list files` call with `SWEBASH_AI_LOG_DIR=/tmp/swebash-manual-log-test`:

### `*-ai-complete.json` (LoggingAiClient layer)
```json
{
  "id": "194c7600-9935-45ee-8d4a-71441878fcb1-ai-complete",
  "kind": "ai-complete",
  "duration_ms": 585,
  "request": {
    "messages": [ <4 messages including NL "list files"> ],
    "options": { "max_tokens": 256, "temperature": 0.1 }
  },
  "result": { "status": "error", "error": "AI provider error: ...credit balance too low..." }
}
```

### `*-complete.json` (LoggingLlmService layer)
```json
{
  "id": "8d007714-2e93-4489-9e61-fd9adee1577d-complete",
  "kind": "complete",
  "request": {
    "model": "claude-sonnet-4-20250514",
    "messages": [ <same 4 messages> ],
    "temperature": 0.1
  },
  "result": { "status": "error", "error": "Invalid request: ...credit balance too low..." }
}
```

Both layers correctly capture the full request context and error. The `ai-complete` file uses `AiError::Provider(...)` wrapping; the `complete` file uses the raw provider error string.

---

## LLM-Dependent Tests (Skipped — no API credits)

| Section | Test | Status |
|---------|------|--------|
| §8 AI Ask | Execute suggested command | ⚪ SKIP |
| §9 AI Explain | `ai explain ls -la` | ⚪ SKIP |
| §10 AI Chat Mode | Multi-turn memory | ⚪ SKIP |
| §11 AI Chat from Shell | `ai chat what is Rust?` | ⚪ SKIP |
| §12 AI Suggest | `ai suggest` | ⚪ SKIP |
| §15 One-Shot Agent Chat | `ai @devops how do I...` | ⚪ SKIP |
| §20 DevOps Agent | `ai @devops list containers` | ⚪ SKIP |

---

## Notes

- All non-LLM features (agent listing, switching, auto-detection, history, clear, logging) pass correctly.
- The `AiEvent::Error` variant introduced in this session correctly propagates provider errors to the UI — the shell prints `[ai] AI provider error: ...` and returns to the prompt cleanly without hanging.
- The two-layer logging (§21) works exactly as designed: `ai-complete` and `complete` files are produced for each request, differentiated by `kind` field and UUID prefix.
