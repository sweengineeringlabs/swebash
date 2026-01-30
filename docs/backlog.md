# Development Backlog

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

## Future Work
- Streaming responses for chat mode
- Integration test suite with mock providers
- YAML configuration file support
- Custom prompt templates
- Plugin system for additional AI features
- Publish llm-provider to crates.io for version-based deps
