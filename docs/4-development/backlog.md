# Development Backlog

> **TLDR:** Phase-by-phase task tracking for swebash feature development.

**Audience**: Developers, project leads

## Table of Contents

- [Phase 1: Foundation (Complete)](#phase-1-foundation-complete)
- [Phase 2: NL → Shell Commands (Complete)](#phase-2-nl-shell-commands-complete)
- [Phase 3: Command Explanation (Complete)](#phase-3-command-explanation-complete)
- [Phase 4: Conversational Assistant (Complete)](#phase-4-conversational-assistant-complete)
- [Phase 5: Autocomplete (Complete)](#phase-5-autocomplete-complete)
- [Phase 6: Polish (Complete)](#phase-6-polish-complete)
- [Phase 7: Rustyline Enhancements - Tab Completion (✅ Complete - Simplified)](#phase-7-rustyline-enhancements-tab-completion-complete-simplified)
- [Phase 8: Rustyline Enhancements - Syntax Highlighting](#phase-8-rustyline-enhancements-syntax-highlighting)
- [Phase 9: Rustyline Enhancements - History Hints](#phase-9-rustyline-enhancements-history-hints)
- [Phase 10: Rustyline Enhancements - Vi Mode](#phase-10-rustyline-enhancements-vi-mode)
- [Phase 11: Rustyline Enhancements - Multi-line Editing](#phase-11-rustyline-enhancements-multi-line-editing)
- [Phase 12: Rustyline Configuration System](#phase-12-rustyline-configuration-system)
- [Phase 13: Agent Infrastructure — Delegate to Rustratify (SRP)](#phase-13-agent-infrastructure-delegate-to-rustratify-srp)
- [Backlog: Migrate bash tests from Git Bash to WSL/Linux](#backlog-migrate-bash-tests-from-git-bash-to-wsllinux)
- [Backlog: Agent documentation context injection](#backlog-agent-documentation-context-injection)
- [Backlog: Agent Behavior Testing](#backlog-agent-behavior-testing)
- [Backlog: Autotest Framework Enhancements](#backlog-autotest-framework-enhancements)
- [Future Work](#future-work)


## Phase 1: Foundation (Complete)
- [x] 1.1 Create `ai/` crate with Cargo.toml, add to workspace
- [x] 1.2 Implement L1 types: AiMessage, AiRole, CompletionOptions, AiError
- [x] 1.3 Implement L2 SPI: AiClient trait
- [x] 1.4 Implement L2 SPI: LlmProviderClient wrapping llm-provider
- [x] 1.5 Implement L5 facade: lib.rs re-exports, create_ai_service()
- [x] 1.6 Add tokio to host, convert main to #[tokio::main]
- [x] 1.7 Add swebash-ai dependency to host, verify cargo check

## Phase 2: NL → Shell Commands (Complete)
- [x] 2.1 Implement translate system prompt in prompt.rs
- [x] 2.2 Implement core/translate.rs
- [x] 2.3 Implement AiService trait (translate method)
- [x] 2.4 Implement DefaultAiService::translate()
- [x] 2.5 Implement host/src/ai/commands.rs: parse `ai ask` and `?`
- [x] 2.6 Implement host/src/ai/mod.rs: handle Ask command
- [x] 2.7 Implement execute confirmation [Y/n/e]
- [x] 2.8 Implement host/src/ai/output.rs: colored AI output

## Phase 3: Command Explanation (Complete)
- [x] 3.1 Add explain system prompt
- [x] 3.2 Implement core/explain.rs
- [x] 3.3 Add explain() to AiService + DefaultAiService
- [x] 3.4 Wire `ai explain` and `??` commands in host

## Phase 4: Conversational Assistant (Complete)
- [x] 4.1 Implement core/history.rs ring buffer
- [x] 4.2 Add chat system prompt
- [x] 4.3 Implement core/chat.rs
- [x] 4.4 Add chat() to AiService + DefaultAiService
- [x] 4.5 Wire `ai chat`, `ai history`, `ai clear` in host

## Phase 5: Autocomplete (Complete)
- [x] 5.1 Add autocomplete system prompt
- [x] 5.2 Implement context gathering in host (cwd listing, recent commands)
- [x] 5.3 Implement core/complete.rs
- [x] 5.4 Add autocomplete() to AiService + DefaultAiService
- [x] 5.5 Wire `ai suggest` in host

## Phase 6: Polish (Complete)
- [x] 6.1 Implement `ai status` command
- [x] 6.2 Graceful degradation when AI unconfigured
- [x] 6.3 Timeout handling with "thinking..." indicator
- [ ] 6.4 Streaming output for chat mode (future)
- [ ] 6.5 Integration tests with providers (future)

## Phase 7: Rustyline Enhancements - Tab Completion (✅ Complete - Simplified)
**Goal**: Implement intelligent tab completion for commands, file paths, and arguments

- [x] 7.1 Implement `Completer` module for swebash
- [x] 7.2 Add builtin command completion (echo, pwd, cd, ls, cat, etc.)
- [x] 7.3 Add file/directory path completion with tilde expansion
- [x] 7.4 Add context-aware completion (directories show `/` suffix)
- [ ] 7.5 Add command history-based completion (future)
- [ ] 7.6 Add environment variable completion ($VAR) (future)
- [x] 7.7 Integrate completer with REPL (double space trigger)
- [ ] 7.8 Add completion for AI commands (future)
- [x] 7.9 Add tests for completion logic (4 tests passing)
- [x] 7.10 Document tab completion in user guide

**Status**: Core features implemented (180 lines). Trigger: double space or tab.
**File**: `host/src/readline/completer.rs`

## Phase 8: Rustyline Enhancements - Syntax Highlighting
**Goal**: Add color-coded syntax highlighting for commands as they're typed

- [ ] 8.1 Implement `Highlighter` trait for swebash
- [ ] 8.2 Add color scheme configuration (can use existing prompt colors)
- [ ] 8.3 Highlight builtin commands (green)
- [ ] 8.4 Highlight external commands (blue)
- [ ] 8.5 Highlight invalid/unknown commands (red)
- [ ] 8.6 Highlight strings/quotes (yellow)
- [ ] 8.7 Highlight file paths (cyan)
- [ ] 8.8 Highlight operators (|, >, <, &&, etc.)
- [ ] 8.9 Integrate highlighter with rustyline Editor
- [ ] 8.10 Add configurable color themes
- [ ] 8.11 Add tests for highlighting logic
- [ ] 8.12 Document syntax highlighting in user guide

## Phase 9: Rustyline Enhancements - History Hints
**Goal**: Show inline hints/suggestions based on command history (fish-shell style)

- [ ] 9.1 Implement `Hinter` trait for swebash
- [ ] 9.2 Add history-based hint matching (prefix search)
- [ ] 9.3 Add hint display configuration (color, style)
- [ ] 9.4 Add hint acceptance keybinding (Right arrow or Ctrl-F)
- [ ] 9.5 Add hint filtering (show most recent/frequent match)
- [ ] 9.6 Add hint context awareness (working directory, etc.)
- [ ] 9.7 Integrate hinter with rustyline Editor
- [ ] 9.8 Add option to disable hints via config
- [ ] 9.9 Add tests for hint logic
- [ ] 9.10 Document hints feature in user guide

## Phase 10: Rustyline Enhancements - Vi Mode
**Goal**: Add Vi editing mode as alternative to Emacs mode

- [ ] 10.1 Add EditMode configuration option (Emacs/Vi)
- [ ] 10.2 Implement Vi command mode keybindings
- [ ] 10.3 Implement Vi insert mode keybindings
- [ ] 10.4 Add visual mode indicator in prompt
- [ ] 10.5 Add Vi-specific commands (dd, yy, p, etc.)
- [ ] 10.6 Add Vi search commands (/, ?, n, N)
- [ ] 10.7 Add Vi motion commands (w, b, e, $, ^, etc.)
- [ ] 10.8 Add configuration file support (.swebashrc)
- [ ] 10.9 Add runtime mode switching (if feasible)
- [ ] 10.10 Add tests for Vi mode
- [ ] 10.11 Document Vi mode in user guide

## Phase 11: Rustyline Enhancements - Multi-line Editing
**Goal**: Improve multi-line command editing for complex scripts

- [ ] 11.1 Implement `Validator` trait for swebash
- [ ] 11.2 Add line continuation detection (trailing backslash)
- [ ] 11.3 Add bracket/quote matching for multi-line
- [ ] 11.4 Add continuation prompt styling (different from main prompt)
- [ ] 11.5 Add multi-line navigation (Up/Down within multi-line)
- [ ] 11.6 Add multi-line history preservation
- [ ] 11.7 Add auto-indent for multi-line commands
- [ ] 11.8 Add bracket/quote auto-closing
- [ ] 11.9 Integrate validator with rustyline Editor
- [ ] 11.10 Add tests for multi-line editing
- [ ] 11.11 Document multi-line editing in user guide

## Phase 12: Rustyline Configuration System
**Goal**: Make rustyline features configurable via config file

- [ ] 12.1 Design .swebashrc configuration format (TOML/YAML)
- [ ] 12.2 Add config file loading from ~/.swebashrc
- [ ] 12.3 Add rustyline section in config (edit_mode, colors, etc.)
- [ ] 12.4 Add keybinding customization support
- [ ] 12.5 Add color theme customization
- [ ] 12.6 Add history configuration (max_size, ignore_patterns)
- [ ] 12.7 Add completion configuration (case_sensitive, etc.)
- [ ] 12.8 Add config validation and error reporting
- [ ] 12.9 Add config reload command (or on-the-fly)
- [ ] 12.10 Add tests for config loading
- [ ] 12.11 Create example .swebashrc file
- [ ] 12.12 Document configuration in setup guide

## Phase 13: Agent Infrastructure — Delegate to Rustratify (SRP)

**Problem**: `AgentRegistry` in `ai/src/core/agents/mod.rs` bundles two responsibilities:
1. **Agent metadata** — register, get, list, detect, suggest (pure data operations)
2. **Engine lifecycle** — lazy `ChatEngine` creation/caching with `LlmService` dependency

This forces `MockLlm` boilerplate in every test that only needs metadata operations. The coupling is a swebash design choice — rustratify's `agent-controller` crate doesn't provide an `AgentRegistry`, so swebash built its own.

**Goal**: swebash should only define its agents (via YAML) and delegate registry infrastructure to rustratify. After rustratify ships AG-1 through AG-4 (see rustratify backlog), swebash refactors to consume them.

**Depends on**: Rustratify AG-1, AG-2, AG-3, AG-4

- [x] 13.1 Replace swebash `Agent` trait with rustratify's `AgentDescriptor` trait
  - `ConfigAgent` implements `AgentDescriptor` instead of the local `Agent` trait
  - `ToolFilter` enum moved to rustratify
  - Local `Agent` trait removed from `ai/src/core/agents/mod.rs`

- [x] **13.1b Extract generic YAML agent config into rustratify `agent-controller`**
  - Added `yaml` feature module to `agent-controller` crate
  - Generic types: `AgentsYaml<Ext>`, `AgentEntry<Ext>`, `AgentDefaults`, `ToolsConfig` (HashMap-based), `YamlAgentDescriptor`
  - `ConfigAgent` refactored to wrap `YamlAgentDescriptor` via composition
  - Swebash-specific types: `SwebashAgentsYaml`, `SwebashFullDefaults`, `SwebashAgentExt`
  - `ToolsConfig` changed from named boolean fields to generic `HashMap<String, bool>`
  - All 123 unit tests + 187 integration tests pass

- [ ] 13.2 Replace swebash `AgentRegistry` with rustratify's `AgentRegistry<D>`
  - Delete `AgentRegistry` struct from `ai/src/core/agents/mod.rs`
  - Use `agent_controller::AgentRegistry<ConfigAgent>` instead
  - Constructor no longer requires `Arc<dyn LlmService>` — metadata-only
  - `detect_agent()`, `suggest_agent()`, `list()`, `get()` delegated to rustratify

- [ ] 13.3 Use rustratify's `EngineCache` for engine lifecycle
  - Replace inline `engines: RwLock<HashMap<...>>` + `create_engine()` with `EngineCache`
  - `EngineCache` takes `LlmService` — engine coupling isolated from metadata
  - `engine_for()`, `clear_agent()`, `clear_all()` delegated to rustratify

- [ ] 13.4 Update `builtins.rs` to compose registry + cache
  - `create_default_registry()` returns `(AgentRegistry<ConfigAgent>, EngineCache)` or a composed wrapper
  - YAML loading (`register_from_yaml`) remains in swebash — agent definitions are swebash's concern
  - User config overlay logic unchanged

- [x] 13.5 Replace `MockLlm` with rustratify's testing infrastructure
  - `MockLlm` replaced with `agent_controller::testing::MockLlmService` across all test files
  - Tests that only need metadata construct `AgentRegistry` with `MockLlmService`

- [x] 13.6 Update integration tests for new architecture
  - All tests updated for new types (`SwebashAgentsYaml`, `SwebashAgentExt`, `SwebashFullDefaults`)
  - `ToolsConfig` assertions updated for HashMap-based structure
  - 123 unit + 189 integration tests pass (187 code-passing; 2 require API credits)

## Backlog: Migrate bash tests from Git Bash to WSL/Linux

**Problem**: Bash test suite (`bin/tests/runner.sh`) currently runs under Git Bash (MSYS2) on Windows. Tests should run on WSL or native Linux only — Git Bash is not a target platform.

- [ ] Update `runner.sh` to detect and refuse to run under MSYS2/Git Bash
- [ ] Add WSL invocation path so PowerShell scripts can dispatch bash tests to WSL
- [ ] Audit existing `.test.sh` files for MSYS2-specific behavior (path formats, `grep -P` support)
- [ ] Ensure CI runs bash tests on Linux or WSL, not Git Bash
- [ ] Document supported test platforms (PowerShell on Windows, bash on WSL/Linux)

## Backlog: Agent documentation context injection (✅ Complete)

**Problem**: Agent knowledge is limited to what fits in the static `systemPrompt` YAML field.
Agents with `fs: true` can read docs at runtime via tool calls, but this requires the LLM to
decide to read a file before answering — adding latency and consuming tool iterations.

**Solution**: Implemented via `docs` YAML field with two strategies: `preload` (default) and `rag`. See [ADR-001](../3-design/ADR-001-agent-doc-context.md) and [RAG Architecture](../3-design/rag_architecture.md).

- [x] Design `docs` field in agent YAML schema (list of glob patterns or paths)
- [x] Implement doc loading at engine creation time (read matching files, concatenate into context)
- [x] Add token budget / truncation strategy (max doc tokens per agent, priority ordering)
- [x] Evaluate RAG-style approach: embed doc chunks, retrieve relevant chunks per query
- [x] Update `AgentEntry` and `ConfigAgent` structs to support new `docs` field
- [x] Add tests for doc injection (doc found, doc missing, doc over budget)
- [x] Migrate `@rscagent` to use `docs` field instead of inline path list
- [x] Migrate `@seaaudit` to reference SEA architecture docs if available
- [x] Document the feature in agent architecture docs

## Backlog: Agent Behavior Testing

**Problem**: Current autotest suite (`ai_features.yaml`) only tests command plumbing (entering AI mode, switching agents, prompt display). It doesn't verify that agents actually behave correctly or use their capabilities appropriately.

**Current Gaps**: ✅ All addressed
1. ~~Tests accept `"not configured"` as valid — pass without an actual LLM~~ ✅ Fixed via mock provider
2. ~~No verification of agent-specific behavior (does `@review` review differently than `@shell`?)~~ ✅ AB-2 reflect mode
3. ~~No tool usage validation (do agents call fs/git/exec tools correctly?)~~ ✅ Tool call logging added
4. ~~No system prompt verification (are agent personas applied?)~~ ✅ AB-5 reflect mode
5. ~~No RAG/docs injection testing (does context get injected?)~~ ✅ AB-6 reflect mode

**Enhancements**:

- [x] **AB-1**: Add mock LLM provider for deterministic agent testing (no API key needed)
  - Added `LLM_PROVIDER=mock` support in `create_ai_service_with_sandbox()`
  - Created `MockAiClient` wrapper in `features/ai/src/spi/mock_client.rs`
  - Environment variables: `SWEBASH_MOCK_RESPONSE`, `SWEBASH_MOCK_RESPONSE_FILE`, `SWEBASH_MOCK_ERROR`, `SWEBASH_MOCK_REFLECT`
  - Added `testing` feature to llm-provider dependency
- [x] **AB-2**: Test agent-specific responses (review agent gives code review, git agent suggests git commands)
  - Added reflect mode (`SWEBASH_MOCK_REFLECT=1`) that echoes system prompt structure
  - `reflect_response()` detects agent identity via keywords (shell, review, git, devops)
  - Tests: `ab2_shell_agent_identity`, `ab2_review_agent_identity`, `ab2_git_agent_identity`, `ab2_devops_agent_identity`
- [x] **AB-3**: Test tool invocation (`tool_called` / `tool_params` validation in spec schema)
  - Added `log_tool_call()` in `features/ai/src/core/chat.rs` (emits `SWEBASH_TOOL:{json}` to stderr)
  - Added `ToolCallRecord` struct and `tool_calls()` method in `features/autotest/src/driver.rs`
  - Added `check_tool_called_structured()` and `check_tool_params()` in `features/autotest/src/validation.rs`
  - Environment variable: `SWEBASH_AI_TOOL_LOG=1` enables tool call logging
- [x] **AB-4**: Test tool filtering (verify `fs: false` blocks filesystem tools)
  - Tests: `ab4_review_agent_has_fs_only`, `ab4_web_agent_has_web_only`, `ab4_shell_agent_has_all_tools`
- [x] **AB-5**: Test system prompt injection (verify agent persona affects responses)
  - Reflect mode returns `[SYSTEM_PROMPT:...]` with first 100 chars of system prompt
  - Tests: `ab5_system_prompt_passed`, `ab5_system_prompt_contains_agent_context`
- [x] **AB-6**: Test RAG/docs context injection for agents with `docs` field
  - Reflect mode detects `[DOCS_INJECTED:true]` when docs context is present
  - Tests: `ab6_agent_with_docs_configured`, `ab6_docreview_agent_available`
- [x] **AB-7**: Test agent memory/conversation continuity across turns
  - Reflect mode returns `[HISTORY:user=N,assistant=M]` showing message counts
  - Tests: `ab7_history_count_increases`, `ab7_clear_resets_history`, `ab7_agent_switch_isolates_history`
- [x] **AB-8**: Test custom user agents loaded from `~/.config/swebash/agents/`
  - Tests: `ab8_env_agents_config_respected`, `ab8_builtin_agents_always_available`
- [x] **AB-9**: Test agent detection/suggestion based on working directory context
  - Tests: `ab9_git_keyword_detection`, `ab9_docker_keyword_detection`, `ab9_disabled_when_env_false`
- [x] **AB-10**: Remove `"not configured"` fallbacks from smoke tests (require mock provider)
  - Added 25+ new mock-based tests in `tests/suites/ai_features.yaml` with `[mock]` tag
  - Tests verify: fixed response, echo mode, status display, all agents, history, clear, one-shot

**Files**: `tests/suites/ai_features.yaml`, `features/autotest/src/validation.rs`, `features/ai/src/spi/mock_client.rs`

## Backlog: Autotest Framework Enhancements

**Problem**: `swebash-autotest` has several limitations that reduce its effectiveness for comprehensive interactive shell testing.

**Current Limitations**:
1. Timeout parameter is accepted but ignored (`driver.rs:220-228`) — tests can hang forever
2. Per-step output not tracked — validation runs against combined output, not per-command
3. No true interactive mode — commands sent all at once, no prompt waiting or mid-session reactions

**Enhancements**:

- [ ] **AT-1**: Enforce timeouts using async/threads in `wait_with_timeout()`
- [ ] **AT-2**: Implement PTY-based driver for true interactive testing (wait for prompts, react to output)
- [ ] **AT-3**: Track output per-step instead of combined session output
- [ ] **AT-4**: Add retry logic for flaky tests (retry N times before failing)
- [ ] **AT-5**: Add watch mode (re-run tests on file changes during development)
- [ ] **AT-6**: Add snapshot/golden file testing for output validation
- [ ] **AT-7**: Integrate with code coverage tools
- [ ] **AT-8**: Add GitHub Actions integration (PR comments, status checks)
- [ ] **AT-9**: Add historical trend analysis in reports
- [ ] **AT-10**: Investigate parallelism isolation issues with shared state

**Files**: `features/autotest/src/driver.rs`, `features/autotest/src/executor.rs`

## Future Work
- Evaluate Loom (tokio-rs/loom) for exhaustive concurrency testing of async task coordination
- Evaluate Shuttle (awslabs/shuttle) for randomized concurrency testing of async/tokio code
- Streaming responses for chat mode
- Integration test suite with mock providers
- Custom prompt templates
- Plugin system for additional AI features
- Publish llm-provider to crates.io for version-based deps
- inputrc compatibility layer (read GNU readline configs)
- Rustyline macro/keyboard macro recording
- Incremental search with preview
- Command abbreviations/aliases via rustyline
- Undo/redo history with visualization
