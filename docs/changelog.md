# Changelog

## [Unreleased]

### Added
- **AI Mode with Smart Detection** (2025-02-02)
  - Interactive AI mode: type `ai` to enter, `exit` to leave
  - Smart intent detection automatically routes commands:
    - Command patterns (flags, pipes, redirects) → explain
    - Action requests (find, list, show) → translate to command
    - Questions and conversation → chat
  - No need to repeatedly type "ai" prefix
  - Explicit subcommands override detection when needed
  - Cyan `[AI Mode]` prompt indicator
  - Handles edge cases: quoted arguments, multiple pipes, case sensitivity
  - Comprehensive test suite:
    - 29 unit tests covering detection logic
    - 9 integration tests for mode transitions and behavior
    - All 92 tests passing (49 unit + 43 integration total)
  - See `docs/ai-mode.md` for complete architecture

- **Persistent command history** (2025-02-02)
  - Custom, in-house implementation (no external deps)
  - Commands automatically saved to `~/.swebash_history`
  - Smart filtering: ignores empty lines, duplicates, commands starting with space
  - Max size limit (1000 commands) with automatic rotation
  - Auto-save on exit via Drop trait
  - History persists across shell sessions
  - Comprehensive test suite (6 unit + 4 integration tests)
  - Foundation for Phase 7-12 interactive features
  - See `docs/history-feature.md` for details

- **Readline Enhancements - Phases 7-12** (2025-02-02 - Simplified)
  - **Tab Completion**: Show completions for commands and file paths (double space to trigger)
  - **History Hints**: Display suggestions from history below prompt
  - **Multi-line Support**: Auto-detect incomplete commands with continuation prompt (`...>`)
  - **Configuration System**: TOML config file at `~/.swebashrc`
  - **Modular Architecture**: `readline` module with completer, hinter, validator
  - 690 lines of code, +2 dependencies (serde, toml)
  - 13 new unit tests (all passing)
  - See `docs/PHASES_7-12_IMPLEMENTATION.md` for details

### Changed
- Added `history` module to host crate
- Added `readline` module with completion, hinting, validation, config
- Updated REPL to support multi-line editing and tab completion
- Updated integration tests to verify history persistence
- Added `.swebashrc.example` configuration file

### Dependencies Added
- `serde = "1.0"` with derive feature (for config)
- `toml = "0.8"` (for config file parsing)

## [0.1.0] - Initial Release

### Features
- WASM-based shell engine with host runtime
- Built-in commands: echo, pwd, cd, ls, cat, mkdir, rm, cp, mv, touch, env, export, head, tail
- External command execution via host imports
- AI integration with natural language commands (ai ask, ?, ??)
- LLM-powered assistance using Anthropic API
- Cross-workspace dependency on rustratify framework
- Comprehensive integration test suite
