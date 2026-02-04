# Backlog: Case study — SEA eliminates cross-platform "common" libraries

**Problem**: `lib/common.sh` and `lib/common.ps1` duplicate the same logic (registry setup, env loading, Cargo.lock sync) across two languages with divergent idioms, edge cases, and bugs. Every new feature requires parallel implementation and parallel tests. The Cargo.lock sync work is a concrete example — identical intent, two implementations, two test suites.

- [ ] Document how a Single Executable Application (SEA) approach removes this duplication
- [ ] Show that a compiled Rust/Go binary handles platform detection, path normalization, and file rewriting once — no bash/PowerShell split
- [ ] Catalog current `common.sh`/`common.ps1` functions and map each to what a SEA replaces
- [ ] Evaluate trade-offs: build step requirement, loss of shell-native scripting flexibility, bootstrap problem (need the tool to build the tool)
- [ ] Write up as a case study in `docs/` for future architecture decisions
