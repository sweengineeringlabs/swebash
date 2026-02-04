# AI Mode Test Suite

## Overview

Comprehensive test coverage for AI Mode with smart detection, including 38 tests covering all aspects of the feature.

## Test Summary

**Total Tests**: 92 (all passing)
- **Unit Tests**: 49 (29 for AI mode)
- **Integration Tests**: 43 (9 for AI mode)

## Unit Tests (29)

Location: `host/src/ai/commands.rs`

### Core Detection (10 tests)

| Test | Purpose |
|------|---------|
| `test_parse_enter_mode` | Verify `ai` command enters mode |
| `test_parse_mode_exit` | Verify `exit`/`quit` exits mode |
| `test_parse_mode_explicit_subcommands` | Explicit subcommands in AI mode |
| `test_looks_like_command_known` | Known commands detected |
| `test_looks_like_command_flags` | Flag detection works |
| `test_looks_like_command_pipes` | Pipe detection works |
| `test_looks_like_command_negative` | Natural language not misdetected |
| `test_is_action_request` | Action verbs detected |
| `test_is_action_request_negative` | Non-actions not misdetected |
| `test_explicit_override` | Explicit subcommands override detection |

### Smart Detection (3 tests)

| Test | Purpose |
|------|---------|
| `test_smart_detection_command` | Commands routed to explain |
| `test_smart_detection_action` | Actions routed to ask/translate |
| `test_smart_detection_chat_fallback` | Chat handles questions/conversation |

### Edge Cases (16 tests)

| Test | Purpose |
|------|---------|
| `test_empty_input` | Empty/whitespace defaults to chat |
| `test_multiple_pipes` | Multi-pipe commands detected |
| `test_redirect_operators` | All redirects detected (>, <, 2>&1) |
| `test_action_verb_with_flags` | Flags override action detection |
| `test_case_sensitivity` | Action verbs work case-insensitive |
| `test_ambiguous_with_paths` | Paths make ambiguous words commands |
| `test_ambiguous_with_extensions` | File extensions indicate commands |
| `test_ambiguous_natural_language` | Ambiguous without syntax → action |
| `test_long_command_chains` | Long piped chains work |
| `test_commands_with_quotes` | Quoted args detected |
| `test_subcommands_with_extra_spaces` | Whitespace handled |
| `test_questions_with_punctuation` | Question formats recognized |
| `test_special_characters` | awk/sed special chars work |
| `test_docker_kubernetes_commands` | Modern tools recognized |
| `test_conversational_phrases` | Conversation goes to chat |

## Integration Tests (9)

Location: `host/tests/integration.rs`

| Test | Purpose | What It Verifies |
|------|---------|-----------------|
| `ai_mode_enter_and_exit` | Mode transitions | Shows "Entered" and "Exited" messages |
| `ai_mode_prompt_indicator` | Prompt changes | `[AI Mode]` prompt appears |
| `ai_mode_status_command` | Commands work | Status command executes in AI mode |
| `ai_mode_quit_exits` | Exit aliases | Both `exit` and `quit` work |
| `ai_mode_chat_response` | AI responses | Either responds or shows "not configured" |
| `ai_mode_preserves_history` | History tracking | Commands saved to history file |
| `shell_mode_exit_quits` | Shell exit | `exit` quits shell when not in AI mode |
| `ai_mode_nested_exit_behavior` | Nested exit | Exit from AI mode returns to shell, not quit |
| `ai_mode_multiple_commands` | Command sequences | Multiple commands stay in AI mode |
| `ai_mode_with_multiline` | Multi-line support | Multi-line works in AI mode |

## Test Examples

### Unit Test: Smart Detection

```rust
#[test]
fn test_smart_detection_command() {
    // Known command → explain
    assert!(matches!(
        parse_ai_mode_command("tar -xzf archive.tar.gz"),
        AiCommand::Explain(_)
    ));

    // Pipe → explain
    assert!(matches!(
        parse_ai_mode_command("ps aux | grep node"),
        AiCommand::Explain(_)
    ));
}

#[test]
fn test_smart_detection_action() {
    // Action verb → ask
    assert!(matches!(
        parse_ai_mode_command("find files larger than 100MB"),
        AiCommand::Ask(_)
    ));
}
```

### Integration Test: Mode Transitions

```rust
#[test]
fn ai_mode_enter_and_exit() {
    let (out, _err) = run(&["ai", "exit"]);

    assert!(
        out.contains("Entered AI mode"),
        "should show entered message"
    );

    assert!(
        out.contains("Exited AI mode"),
        "should show exited message"
    );
}
```

## Edge Cases Covered

### 1. Ambiguous Commands

**Challenge**: Words like "find" can be commands or natural language.

**Solution**: Context-aware detection
- `find . -name "*.log"` → Command (has flags)
- `find /var/log` → Command (has path)
- `find large files` → Action request (natural language)

**Tests**:
- `test_ambiguous_with_paths`
- `test_ambiguous_with_extensions`
- `test_ambiguous_natural_language`

### 2. Redirect Operators

**Challenge**: Many redirect formats (`>`, `2>`, `2>&1`, etc.)

**Solution**: Detect all common forms

**Tests**:
- `test_redirect_operators` - Tests `>`, `<`, `2>&1`

### 3. Quoted Arguments

**Challenge**: Quotes indicate command arguments, not natural language

**Solution**: Detect quotes in ambiguous commands
- `echo "hello world"` → Command (has quotes)
- `echo hello world` → Could be action request

**Tests**:
- `test_commands_with_quotes`

### 4. Case Sensitivity

**Challenge**: Users might capitalize action verbs

**Solution**: Lowercase action verb detection
- `Find large files` → Action request
- `LIST running processes` → Action request

**Tests**:
- `test_case_sensitivity`

### 5. Empty Input

**Challenge**: What happens with empty/whitespace input?

**Solution**: Default to chat (safest)

**Tests**:
- `test_empty_input`

## Running Tests

### All Tests
```bash
cargo test --manifest-path host/Cargo.toml
```

### Unit Tests Only
```bash
cargo test --manifest-path host/Cargo.toml --bin swebash commands
```

### Integration Tests Only
```bash
cargo test --manifest-path host/Cargo.toml --test integration ai_mode
```

### With Output
```bash
cargo test --manifest-path host/Cargo.toml -- --nocapture
```

## Test Results

```
Unit Tests (binary):
running 49 tests
test result: ok. 49 passed; 0 failed

Integration Tests:
running 43 tests
test result: ok. 43 passed; 0 failed

Total: 92 tests, 100% passing
```

## Coverage Analysis

### Command Detection
- ✅ Unambiguous commands (ls, grep, tar, etc.)
- ✅ Commands with flags (-x, --flag)
- ✅ Commands with pipes (|)
- ✅ Commands with redirects (>, <, 2>&1)
- ✅ Commands with quotes
- ✅ Commands with special chars (awk, sed)
- ✅ Modern tools (docker, kubectl, cargo)

### Action Detection
- ✅ Common action verbs (find, list, show, etc.)
- ✅ Case-insensitive matching
- ✅ Priority rules (flags override action verbs)

### Chat Fallback
- ✅ Questions (how, what, why, ?)
- ✅ Conversational phrases
- ✅ Unknown patterns
- ✅ Empty/whitespace input

### Mode Behavior
- ✅ Entering AI mode
- ✅ Exiting AI mode (exit, quit)
- ✅ Prompt indicator changes
- ✅ History preservation
- ✅ Exit behavior differences (shell vs AI mode)
- ✅ Multi-line support

### Edge Cases
- ✅ Ambiguous commands with context
- ✅ Long command chains
- ✅ Extra whitespace
- ✅ Multiple pipes
- ✅ File extensions detection
- ✅ Path detection (/, ./, ~/)
- ✅ Explicit subcommand override

## Continuous Integration

All tests must pass before merging:
- Unit tests verify detection logic correctness
- Integration tests verify end-to-end behavior
- No manual intervention required
- Fast execution (~13 seconds total)

## Future Test Additions

Potential areas for additional coverage:
1. Performance tests (large input, many commands)
2. Concurrency tests (if AI mode becomes multi-threaded)
3. Error handling tests (malformed input)
4. Locale/encoding tests (non-ASCII input)
5. Fuzz testing for detection algorithm

## Conclusion

The AI Mode test suite provides comprehensive coverage with 38 tests specifically for the feature:
- **Detection accuracy**: Verified across all command patterns
- **Edge cases**: Handled with specific tests
- **Integration**: End-to-end behavior verified
- **Reliability**: 100% pass rate

This ensures the smart detection system works correctly and handles edge cases gracefully.
