# Rustyline Enhancement Backlog - Summary

Created: 2025-02-02

## Overview

Created comprehensive backlog for implementing advanced rustyline features in swebash, transforming it into a modern shell with features comparable to fish, zsh, and nushell.

## Documents Created

### 1. **docs/backlog.md** (Updated)
Added 6 new phases (7-12) with 65 specific, actionable tasks:

- **Phase 7**: Tab Completion (10 tasks)
- **Phase 8**: Syntax Highlighting (12 tasks)
- **Phase 9**: History Hints (10 tasks)
- **Phase 10**: Vi Mode (11 tasks)
- **Phase 11**: Multi-line Editing (11 tasks)
- **Phase 12**: Configuration System (12 tasks)

### 2. **docs/rustyline-enhancements.md** (New)
Comprehensive technical design document (800+ lines) covering:

- Architecture and module structure
- Detailed implementation plans for each feature
- Complete code examples and patterns
- Testing strategies
- Performance considerations
- User experience design
- Configuration format specifications

### 3. **CHANGELOG.md** (Updated)
Added "Planned" section referencing the new backlog phases.

## Feature Breakdown

### Phase 7: Tab Completion
**What**: Intelligent completion for commands, files, and arguments
**Impact**: Faster command entry, fewer typos
**Examples**:
- `ec<TAB>` → `echo`
- `cat ~/Doc<TAB>` → `cat ~/Documents/`
- `cd <TAB>` → shows only directories

**Key Tasks**:
- Implement `Completer` trait
- Command name completion (builtin + external)
- File/directory path completion
- Context-aware completion (cd only shows dirs)
- Environment variable completion ($VAR)

### Phase 8: Syntax Highlighting
**What**: Real-time color-coded syntax highlighting
**Impact**: Visual feedback, catch typos immediately
**Examples**:
- Builtin commands: green (`echo`)
- External commands: blue (`git`)
- Invalid commands: red (`typo`)
- Strings: yellow (`"hello"`)
- Paths: cyan (`~/file.txt`)
- Operators: magenta (`|`, `>`, `&&`)

**Key Tasks**:
- Implement `Highlighter` trait
- Color scheme for different token types
- Configurable themes
- Command validation (check if exists)

### Phase 9: History Hints
**What**: Fish-shell style suggestions as you type
**Impact**: Discover previous commands, less retyping
**Examples**:
```bash
~/swebash/> echo he█
                  llo world  # gray hint from history
# Press → or Ctrl-F to accept
```

**Key Tasks**:
- Implement `Hinter` trait
- History-based prefix matching
- Hint acceptance keybinding
- Most recent/frequent match selection

### Phase 10: Vi Mode
**What**: Full Vi editing mode for Vi/Vim users
**Impact**: Native editing experience for Vi users
**Examples**:
- Normal mode: `hjkl`, `dd`, `yy`, `p`
- Insert mode: standard typing
- Search: `/`, `?`, `n`, `N`
- Visual indicator: `[N]` or `[I]` in prompt

**Key Tasks**:
- Add EditMode configuration
- Implement Vi keybindings
- Mode indicator in prompt
- Configuration file support

### Phase 11: Multi-line Editing
**What**: Improved editing for complex, multi-line commands
**Impact**: Better shell scripting, complex pipelines
**Examples**:
```bash
~/swebash/> echo "hello \
... world"

~/swebash/> if [ -f file.txt ]; then \
... cat file.txt \
... fi
```

**Key Tasks**:
- Implement `Validator` trait
- Line continuation detection (backslash)
- Bracket/quote matching
- Continuation prompt styling
- Auto-indent

### Phase 12: Configuration System
**What**: User-configurable settings via `~/.swebashrc`
**Impact**: Personalized shell experience
**Example Config**:
```toml
[readline]
edit_mode = "vi"
max_history_size = 5000
enable_hints = true

[readline.colors]
builtin_command = "green"
external_command = "blue"
```

**Key Tasks**:
- Design config file format (TOML)
- Config file loading
- Per-feature configuration
- Keybinding customization
- Color theme customization

## Implementation Strategy

### Recommended Order

1. **Phase 7 (Completion)** - High value, moderate complexity
2. **Phase 8 (Highlighting)** - High visibility, moderate complexity
3. **Phase 9 (Hints)** - Nice-to-have, low complexity
4. **Phase 12 (Config)** - Foundation for customization
5. **Phase 11 (Multi-line)** - Advanced feature
6. **Phase 10 (Vi Mode)** - For Vi users specifically

### Incremental Approach

Each phase can be implemented independently and incrementally:
- Start with basic implementation
- Add tests
- Gather feedback
- Refine and add advanced features

### Integration Points

All features integrate through rustyline's `Helper` trait:
```rust
#[derive(Helper)]
pub struct SwebashHelper {
    completer: SwebashCompleter,
    highlighter: SwebashHighlighter,
    hinter: SwebashHinter,
    validator: SwebashValidator,
}
```

## Effort Estimates

**Phase 7 (Completion)**: 2-3 days
- Core implementation: 1 day
- Path completion: 1 day
- Testing & polish: 0.5-1 day

**Phase 8 (Highlighting)**: 2-3 days
- Basic highlighting: 1 day
- Color scheme: 0.5 day
- Command validation: 0.5 day
- Testing & polish: 0.5-1 day

**Phase 9 (Hints)**: 1-2 days
- Basic hinting: 0.5 day
- History integration: 0.5 day
- Testing & polish: 0.5 day

**Phase 10 (Vi Mode)**: 1-2 days
- Configuration: 0.5 day
- Mode indicator: 0.5 day
- Testing & documentation: 0.5-1 day

**Phase 11 (Multi-line)**: 2-3 days
- Validator implementation: 1 day
- Continuation prompt: 0.5 day
- Auto-indent: 0.5 day
- Testing & polish: 0.5-1 day

**Phase 12 (Config)**: 2-3 days
- Config format design: 0.5 day
- Loading/parsing: 1 day
- Integration: 0.5 day
- Testing & documentation: 0.5-1 day

**Total**: 10-16 days for all phases

## Testing Requirements

Each phase includes:
- **Unit tests**: Test trait implementations
- **Integration tests**: Test combined behavior
- **Manual tests**: Interactive testing scenarios

Test coverage targets:
- Code coverage: >80%
- Edge cases: Documented and tested
- Cross-platform: Test on Linux, macOS, Windows

## Documentation Requirements

For each phase:
- Update user guide with examples
- Add configuration documentation
- Create troubleshooting section
- Add screenshots/demos
- Update CHANGELOG

## Success Metrics

- **User Experience**: Modern shell feel, on-par with fish/zsh
- **Functionality**: All features working as designed
- **Performance**: No noticeable lag during typing
- **Stability**: No crashes or data loss
- **Compatibility**: Works across platforms and terminals

## Dependencies

- `rustyline = "15"` (already added)
- `which = "4"` (for command validation)
- `toml = "0.8"` (for config file)
- `serde = "1.0"` (already in deps)

## Risks and Mitigations

**Risk**: Feature complexity overwhelming users
**Mitigation**: Make features opt-in, provide sensible defaults

**Risk**: Performance impact on large histories
**Mitigation**: Implement caching, limit search scope

**Risk**: Cross-platform compatibility issues
**Mitigation**: Test on all platforms, graceful degradation

**Risk**: Breaking existing workflows
**Mitigation**: Maintain backward compatibility, configuration options

## Next Steps

1. **Review and approve** this backlog
2. **Prioritize** which phases to implement first
3. **Create GitHub issues** for tracking (optional)
4. **Start implementation** with Phase 7 (Completion)
5. **Iterate** based on feedback

## References

- [Rustyline Documentation](https://docs.rs/rustyline)
- [Rustyline GitHub](https://github.com/kkawakam/rustyline)
- [Fish Shell](https://fishshell.com/) - inspiration for hints
- [Zsh](https://zsh.org/) - inspiration for completion
- [Nushell](https://www.nushell.sh/) - inspiration for modern features

---

**Status**: ✅ Backlog Created - Ready for Implementation
**Total Tasks**: 65 across 6 phases
**Estimated Effort**: 10-16 days
**Priority**: Medium-High (User experience enhancement)
