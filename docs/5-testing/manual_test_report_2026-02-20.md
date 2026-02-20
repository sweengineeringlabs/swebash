# Manual Test Report — 2026-02-20

**Build**: release (`/tmp/swebash-target/release/swebash` — pre-built, no source changes since build)
**Platform**: WSL2 (Linux 6.6.87.2-microsoft-standard-WSL2)
**Provider**: anthropic / claude-sonnet-4-20250514
**API key status**: Invalid (`authentication_error: invalid x-api-key`) — all LLM requests reach the Anthropic API but fail auth; all non-LLM code paths pass.
**AI enabled**: `SWEBASH_AI_ENABLED=true` required (new — see §7 note)
**User agents config**: `~/.config/swebash/agents.yaml` loaded (ragtest agent) — 11 agents total.

---

## Results Summary

### §1 Shell Basics

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Startup banner | `./sbh run` | ✅ PASS | Prints `wasm-shell v0.1.0` and prompt |
| Echo | `echo hello world` | ✅ PASS | Printed `hello world` |
| PWD | `pwd` | ✅ PASS | Printed `/home/adentic/workspace` |
| LS | `ls` | ✅ PASS | Listed `.fastembed_cache` (only item in workspace) |
| Exit | `exit` | ✅ PASS | Clean exit |

### §2 Directory Navigation

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| CD absolute | `cd /tmp` | ✅ PASS | Sandbox blocks: `sandbox: read access denied for '/tmp': outside workspace` |
| CD relative | `cd ..` | ✅ PASS | Sandbox blocks leaving workspace root; stays in `/home/adentic/workspace` |
| CD nonexistent | `cd /no/such/dir` | ✅ PASS | Prints sandbox error then `cd: /no/such/dir: no such directory` |
| PWD after blocked CD | `cd /tmp && pwd` | ✅ PASS | Stays at `/home/adentic/workspace` |

### §3 File Operations

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Touch (outside workspace) | `touch /tmp/test_manual.txt` | ✅ PASS | Sandbox blocks: `sandbox: write access denied for '/tmp/test_manual.txt': outside workspace` |
| Touch (workspace RW) | `workspace rw && touch foo.txt` | ✅ PASS | File created in workspace |
| Cat empty | `cat foo.txt` | ✅ PASS | Empty output (file exists) |
| Cat missing | `cat /tmp/no_such_file` | ✅ PASS | Sandbox blocks with write-access error |
| LS after create | `ls` | ✅ PASS | `foo.txt` and `.fastembed_cache` listed |
| RM | `rm foo.txt` | ✅ PASS | File removed |
| Mkdir outside workspace | `mkdir /tmp/test_dir_manual` | ✅ PASS | Sandbox blocks |
| Mkdir -p outside workspace | `mkdir -p /tmp/a/b/c` | ✅ PASS | Sandbox blocks |

### §4 Environment Variables

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Export | `export FOO=bar` | ✅ PASS | Sets variable |
| Env | `env` | ✅ PASS | `FOO=bar` visible in env output |

### §5 External Commands

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| External echo | `/bin/echo test` | ✅ PASS | Prints `test` |
| Unknown command | `notarealcommand` | ✅ PASS | `notarealcommand: No such file or directory (os error 2); process exited with code 127` |

### §6b Workspace Sandbox

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Default workspace | `pwd` on startup | ✅ PASS | `/home/adentic/workspace` (auto-created) |
| Sandbox status | `workspace` | ✅ PASS | Enabled; root `/home/adentic/workspace`; `/home/adentic/workspace [ro]` |
| Read-only default | `touch bar` | ✅ PASS | `sandbox: write access denied ... read-only workspace` |
| Switch to RW | `workspace rw` then `touch bar` | ✅ PASS | `Workspace set to read-write.`; file created |
| Switch back to RO | `workspace ro` then `touch baz` | ✅ PASS | `Workspace set to read-only.`; denied |
| Allow path | `workspace allow /tmp rw` then `cd /tmp` | ✅ PASS | `Allowed path added: /tmp [rw]`; cd succeeds; pwd shows `/tmp` |
| Deny outside | `cd /etc` | ✅ PASS | `sandbox: read access denied for '/etc': outside workspace` |
| Disable sandbox | `workspace disable` then `cd /etc` | ✅ PASS | `Sandbox disabled.`; cd succeeds; pwd shows `/etc` |
| Re-enable sandbox | `workspace enable` then `cd /etc` | ✅ PASS | `Sandbox enabled.`; denied again |
| Env var override | `SWEBASH_WORKSPACE=/tmp ./sbh run` then `pwd` | ✅ PASS | Shows `/tmp`; workspace status shows `[rw]` |

### §7 AI Status

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Status | `ai status` | ✅ PASS | `Enabled: yes, Provider: anthropic, Model: claude-sonnet-4-20250514, Ready: yes` |

**Note**: `SWEBASH_AI_ENABLED=true` must be set explicitly. Without it, `ai status` produces no output. This is a new requirement introduced by commit `fdd6bda feat(ai): wire OAuth credentials as primary auth for anthropic provider`. The `.env` file does not include this variable — testers must export it manually or add it to `.env`.

### §8 AI Ask (NL to Command)

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Ask via subcommand | `ai ask list all files` | ✅ PASS (error-path) | `[ai] thinking...` printed; API 401; `[ai] AI provider error: Authentication failed: ...`; shell recovers |
| Ask via `?` shorthand | `? find rust files` | ✅ PASS (error-path) | Same dispatch and error-path behavior |

### §9 AI Explain

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Explain via subcommand | `ai explain ls -la` | ✅ PASS (error-path) | `[ai] thinking...`; API 401; shell recovers |
| Explain via `??` shorthand | `?? ps aux` | ✅ PASS (error-path) | Same behavior |

### §11 AI Chat from Shell

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Inline chat | `ai chat what is Rust` | ✅ PASS (error-path) | `[ai] thinking...`; `LLM error: Provider error (anthropic): HTTP 401 Unauthorized`; shell prompt returns |

### §12 AI Suggest

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| Suggest | `ai suggest` | ✅ PASS (error-path) | API 401; error returned cleanly |

### §13 Agent Listing

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| List from shell | `ai agents` | ✅ PASS | 11 agents listed (10 built-in + ragtest from user config); `*shell` active |
| List from AI mode | `ai` then `agents` | ✅ PASS | Same table inside AI mode |

### §14 Agent Switching (AI Mode)

| Test | Steps | Result | Notes |
|------|-------|--------|-------|
| Switch to review | `@review` | ✅ PASS | "Switched to Code Reviewer (review)" printed **twice**; prompt → `[AI:review]` |
| Switch to git | `@git` | ✅ PASS | "Switched to Git Assistant (git)" printed **twice**; prompt → `[AI:git]` |
| Switch to devops | `@devops` | ✅ PASS | "Switched to DevOps Assistant (devops)" (once); prompt → `[AI:devops]` |
| Switch back to shell | `@shell` | ✅ PASS | "Switched to Shell Assistant (shell)" (once); prompt → `[AI:shell]` |
| Active marker follows | `@review` then `agents` | ✅ PASS | `*review` marked active; all other agents unmarked |

### §14b Agent Switching (from Shell Mode)

| Test | Steps | Result | Notes |
|------|-------|--------|-------|
| `@devops` enters AI mode | `@devops` from shell | ✅ PASS | "Switched to DevOps Assistant", "Entered AI mode", prompt `[AI:devops]` |
| `@git` enters AI mode | `@git` from shell | ✅ PASS | "Switched to Git Assistant", "Entered AI mode", prompt `[AI:git]` |
| `@review` enters AI mode | `@review` from shell | ✅ PASS | "Switched to Code Reviewer", "Entered AI mode", prompt `[AI:review]` |
| `ai @devops` also works | `ai @devops` from shell | ✅ PASS | Same behavior as bare `@devops` |
| Exit returns to shell | `@devops` → `exit` → `echo hello` | ✅ PASS | AI mode exited; `echo hello` printed `hello` |

### §15 One-Shot Agent Chat (Shell Mode)

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| One-shot @devops | `ai @devops how do I check running containers` | ✅ PASS (error-path) | `[ai] [devops] DevOps Assistant` header printed; thinking; HTTP 401; returns to shell (no AI mode entered) |

### §16 Auto-Detection (AI Mode)

| Test | Steps | Result | Notes |
|------|-------|--------|-------|
| Docker keyword | `ai` then `docker ps` | ✅ PASS | "Switched to DevOps Assistant (devops)"; API 401 on LLM call |
| Git keyword | `ai` then `git rebase` | ✅ PASS | "Switched to Git Assistant (git)"; API 401 on LLM call |
| No match stays | `ai` then `how do I list files` | ✅ PASS | No switch message; stays on shell; API 401 on LLM call |

### §17b History and Clear

| Test | Steps | Result | Notes |
|------|-------|--------|-------|
| History empty | `ai` → `history` | ✅ PASS | `(no chat history)` |
| Clear | `ai` → `clear` → `history` | ✅ PASS | "Chat history cleared." then `(no chat history)` |

### §21 Request/Response Logging

| Test | Steps | Result | Notes |
|------|-------|--------|-------|
| Logging disabled by default | Run `ai ask` without `SWEBASH_AI_LOG_DIR` | ✅ PASS | No log directory created |
| Log dir auto-created | Set `SWEBASH_AI_LOG_DIR` to new path, run `ai ask` | ✅ PASS | Directory created automatically |
| `ai-complete` file created | Same setup | ✅ PASS | `*-ai-complete.json` written with `kind: "ai-complete"` |
| `complete` file created | Same setup | ✅ PASS | `*-complete.json` written with `kind: "complete"` |
| Error logged with status | API key invalid | ✅ PASS | `result.status: "error"` with full auth error message |
| Both files share timestamp | Inspect both files | ✅ PASS | `timestamp_epoch_ms` identical in both files (`1771598772148`) |
| `ai-complete` has 4 messages | Inspect request | ✅ PASS | system, user context, assistant ack, user NL |
| `complete` has model field | Inspect request | ✅ PASS | `model: "claude-sonnet-4-20250514"` present |
| Multiple requests → multiple files | Run `ai ask` ×3 | ✅ PASS | 6 files (3 `ai-complete` + 3 `complete`), all unique UUIDs |
| Unset disables logging | No `SWEBASH_AI_LOG_DIR` in env | ✅ PASS | No new files written |

### §22 sbh Launcher

| Test | Command | Result | Notes |
|------|---------|--------|-------|
| `--help` | `./sbh --help` | ✅ PASS | Prints all commands; exit 0 |
| `help` | `./sbh help` | ✅ PASS | Same output as `--help` |
| No args | `./sbh` | ✅ PASS | Prints usage; exit 0 |
| Unknown command | `./sbh foo` | ✅ PASS | Prints usage; exit 1 |
| Registry ok | `./sbh test engine` | ✅ PASS | First line: `==> Registry: file:///...index (ok)` |
| Registry missing | `CARGO_REGISTRIES_LOCAL_INDEX=file:///nonexistent ./sbh test engine` | ✅ PASS | `ERROR: Local registry index not found at /nonexistent`; exits 1 |
| gen-aws-docs dispatch | `./sbh gen-aws-docs` | ✅ PASS | Routes to `bin/gen-aws-docs.sh`; "AWS CLI not found" (expected on this system) |

---

## LLM-Dependent Tests (Skipped — invalid API key)

| Section | Test | Status |
|---------|------|--------|
| §8 AI Ask | Execute suggested command | ⚪ SKIP |
| §9 AI Explain | Natural language output | ⚪ SKIP |
| §10 AI Chat Mode | Basic chat, multi-turn memory | ⚪ SKIP |
| §11 AI Chat from Shell | Response content | ⚪ SKIP |
| §12 AI Suggest | Autocomplete suggestions | ⚪ SKIP |
| §15 One-Shot | Response content for @devops/@review | ⚪ SKIP |
| §17 Memory Isolation | Cross-agent memory isolation | ⚪ SKIP |
| §19 Shared Directives | Verify directives block in prompt | ⚪ SKIP |
| §19b docs_context | rscagent/docreview docs loaded | ⚪ SKIP |
| §20 DevOps Agent | Docker-specific responses | ⚪ SKIP |

---

## Notes

- **All non-LLM features pass**: shell basics, workspace sandbox, agent listing/switching, auto-detection, history/clear, logging infrastructure, sbh launcher.
- **API key invalid**: `.env` `ANTHROPIC_API_KEY` returns `authentication_error: invalid x-api-key` — all LLM requests reach Anthropic but are rejected at auth. Error propagation via `AiEvent::Error` continues to work correctly.
- **`SWEBASH_AI_ENABLED=true` now required** (new finding): AI features do not activate from `LLM_PROVIDER` + API key alone. This env var must be set explicitly. Related to commit `fdd6bda feat(ai): wire OAuth credentials as primary auth for anthropic provider`. Recommend adding `SWEBASH_AI_ENABLED=true` to `.env.example` and the manual testing prerequisites.
- **Double switch message for `@review` and `@git`**: These two agents print "Switched to X" twice in AI mode (once from the AI layer, once from the shell/UI layer). `@devops` and `@shell` print it once. Pre-existing behavior; consistent with 2026-02-10 report.
- **User config loaded correctly**: `~/.config/swebash/agents.yaml` ragtest agent present — 11 agents listed.
- **Workspace sandbox**: All sandbox modes (RO, RW, allow-path, disable/enable, env-var override) work correctly.
- **Logging**: 3 requests produced 6 log files with unique UUIDs and correct `kind` values. Timestamps matched between paired files.
