# Command History Feature

> **TLDR:** Persistent command history with smart filtering, auto-save, and configurable size limits.

**Audience**: Developers

## Table of Contents

- [Overview](#overview)
- [Changes Made](#changes-made)
- [Usage](#usage)
- [Testing](#testing)
- [Technical Details](#technical-details)
- [Future Enhancements](#future-enhancements)
- [Troubleshooting](#troubleshooting)
- [Summary](#summary)


## Overview
Added command history support to swebash with a custom, in-house implementation:
- **Persistent history**: Commands automatically saved to `~/.swebash_history`
- **History across sessions**: Previous commands available after shell restart
- **Smart filtering**: Ignores empty lines, duplicates, and commands starting with space
- **Max size limit**: Configurable maximum history size (default: 1000 commands)
- **Automatic saving**: History saved on exit via Drop trait
- **Future-ready**: Foundation for interactive navigation (Phase 7-12 in backlog)

## Changes Made

### 1. New Module (`host/src/history.rs`)
Created custom `History` struct with file persistence:

**Features**:
- In-memory command storage with file backing
- Load history from file on startup
- Auto-save on Drop (when shell exits)
- Configurable max size with automatic rotation
- Smart filtering:
  - Ignores empty commands
  - Ignores commands starting with space (for secrets)
  - Ignores duplicate of last command
  - Never saves "exit" command

**API**:
```rust
pub struct History {
    commands: Vec<String>,
    max_size: usize,
    file_path: Option<PathBuf>,
}

impl History {
    pub fn new(max_size: usize) -> Self;
    pub fn with_file(max_size: usize, file_path: PathBuf) -> Self;
    pub fn add(&mut self, command: String);
    pub fn get(&self, index: usize) -> Option<&String>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn commands(&self) -> &[String];
    pub fn save(&self) -> std::io::Result<()>;
}
```

### 2. Main REPL (`host/src/main.rs`)
Integrated history into the REPL loop:

```rust
// Initialize history with file persistence
let history_path = dirs::home_dir()
    .map(|h| h.join(".swebash_history"))
    .unwrap_or_else(|| std::path::PathBuf::from(".swebash_history"));
let mut history = History::with_file(1000, history_path);

// In REPL loop, add commands after reading:
history.add(cmd.to_string());

// History automatically saved on exit (Drop trait)
```

**Key Implementation Details**:
- Read line with `stdin.read_line()` (simple, no external deps)
- Preserve leading whitespace until after history check
- Add to history *after* checking for "exit" command
- History saves automatically when shell exits

### 3. Integration Tests (`host/tests/integration.rs`)
Added 4 comprehensive integration tests:

- **`history_file_created`**: Verifies `.swebash_history` is created
- **`history_persists_commands`**: Tests commands are saved to file
- **`history_ignores_empty_lines`**: Empty lines don't clutter history
- **`history_ignores_space_prefix`**: Commands with space prefix not saved

### 4. Unit Tests (`host/src/history.rs`)
Added 6 unit tests for the History struct:

- `test_add_command`: Basic add functionality
- `test_ignore_empty`: Empty/whitespace-only commands ignored
- `test_ignore_space_prefix`: Leading space detection
- `test_ignore_duplicate_last`: Consecutive duplicates ignored
- `test_max_size`: History rotation when max size exceeded
- `test_persistence`: Load/save to file works correctly

## Usage

### Basic Command History

Commands are automatically saved as you type them:

```bash
~/swebash/> echo hello
hello
~/swebash/> pwd
/home/user/swebash
~/swebash/> ls
engine  host  ai  docs
~/swebash/> exit
```

After exiting, check your history:
```bash
$ cat ~/.swebash_history
echo hello
pwd
ls
```

### Secret Commands (Space Prefix)

Start a command with space to keep it out of history:

```bash
~/swebash/> export API_KEY=secret
~/swebash/>  export API_KEY=secret   # <-- leading space, not saved
~/swebash/> exit

$ cat ~/.swebash_history
export API_KEY=secret
# The second command is NOT in history
```

### History File Location

- Default: `~/.swebash_history`
- Fallback (if HOME not set): `./.swebash_history`
- Configurable via code (future: via config file)

## Testing

### Automated Tests

```bash
# Run all integration tests (includes 4 history tests)
cargo test --manifest-path host/Cargo.toml --test integration

# Run only history integration tests
cargo test --manifest-path host/Cargo.toml --test integration history

# Run history unit tests
cargo test --manifest-path host/Cargo.toml history::
```

All tests pass:
- 6 unit tests
- 4 integration tests
- 33 total integration tests

### Manual Testing

```bash
# 1. Build the shell
cargo build --manifest-path host/Cargo.toml

# 2. Clear existing history
rm ~/.swebash_history

# 3. Run and type some commands
./target/debug/swebash
~/swebash/> echo test1
~/swebash/> echo test2
~/swebash/> pwd
~/swebash/> exit

# 4. Verify history was saved
cat ~/.swebash_history
# Output:
# echo test1
# echo test2
# pwd

# 5. Run again and verify history persists
./target/debug/swebash
~/swebash/> echo test3
~/swebash/> exit

cat ~/.swebash_history
# Output:
# echo test1
# echo test2
# pwd
# echo test3
```

## Technical Details

### Why Custom Implementation?

Instead of using `rustyline`, we built a custom history system because:

1. **Simplicity**: Core history is simple - just save/load a file
2. **Control**: Full control over behavior and features
3. **No external deps**: One less dependency to manage
4. **Foundation**: Base for Phase 7-12 enhancements (completion, highlighting, hints)
5. **Learning**: Understanding how shells work under the hood

### Architecture

```
History Lifecycle:
1. Shell starts → Load ~/.swebash_history into memory
2. User types command → Add to in-memory history
3. Shell exits → Save all history to file (via Drop)

Thread Safety: Not needed (single-threaded shell)
Performance: O(1) add, O(n) load/save (acceptable for typical history size)
```

### History Filtering Logic

```rust
pub fn add(&mut self, command: String) {
    // Filter 1: Empty or whitespace-only
    if command.trim().is_empty() {
        return;
    }

    // Filter 2: Leading space (secret commands)
    if command.starts_with(' ') {
        return;
    }

    // Filter 3: Duplicate of last command
    if let Some(last) = self.commands.last() {
        if last == &command {
            return;
        }
    }

    // Add to history
    self.commands.push(command);

    // Enforce max size (rotate old entries out)
    if self.commands.len() > self.max_size {
        self.commands.remove(0);
    }
}
```

### File Format

Simple newline-delimited text file:
```
command1
command2
command3
```

**Benefits**:
- Human-readable and editable
- Simple to parse
- No versioning needed (yet)
- Compatible with other tools

**Future Enhancements** (see backlog Phase 7-12):
- Add timestamps
- Add metadata (cwd, exit code, etc.)
- Add search index
- Consider binary format for performance

## Future Enhancements

The current implementation provides the foundation for:

**Phase 7**: Tab Completion
- Use history for command completion
- Suggest recently-used commands

**Phase 8**: Syntax Highlighting
- Highlight valid vs invalid commands as you type

**Phase 9**: History Hints (fish-shell style)
- Show suggestions from history as you type
- Press → to accept hint

**Phase 10**: Vi Mode
- Vi-style history navigation

**Phase 11**: Multi-line Editing
- Track multi-line commands in history properly

**Phase 12**: Configuration System
- Customize history size, file location
- Configure filtering behavior

See `docs/backlog.md` for detailed implementation plans.

## Troubleshooting

### Issue: History file not created

**Cause**: HOME directory not set or not writable
**Solution**: Check `echo $HOME` and permissions

### Issue: Commands not saving

**Cause**: Shell crashed before Drop could save
**Solution**: Add explicit save on SIGTERM/SIGINT (future enhancement)

### Issue: History file too large

**Cause**: Max size not enforced
**Solution**: Current implementation enforces 1000 command limit. Adjust in code if needed.

### Issue: Want to clear history

```bash
# Option 1: Delete file
rm ~/.swebash_history

# Option 2: Truncate file
> ~/.swebash_history

# Future: Add "history clear" command
```

## Summary

We've successfully implemented persistent command history with:
- ✅ Custom, in-house implementation (no external deps for history)
- ✅ File persistence at `~/.swebash_history`
- ✅ Smart filtering (empty, duplicates, space-prefix)
- ✅ Automatic save on exit
- ✅ Comprehensive tests (10 tests total)
- ✅ Foundation for future interactive features

**Status**: ✅ Complete and working
**Next**: Implement Phase 7-12 enhancements as per backlog
