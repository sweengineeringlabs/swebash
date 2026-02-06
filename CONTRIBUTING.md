# Contributing to swebash

**Audience**: Internal contributors

**WHAT**: Guidelines for contributing code, documentation, and bug fixes to swebash
**WHY**: Consistent process keeps the codebase healthy and reviews efficient
**HOW**: Branch conventions, commit format, PR requirements, and review expectations

---

## Branch Naming

Create branches from `main` using the following prefixes:

| Prefix | Purpose | Example |
|--------|---------|---------|
| `feature/` | New functionality | `feature/agent-memory` |
| `fix/` | Bug fixes | `fix/readline-escape-codes` |
| `docs/` | Documentation only | `docs/adr-002-tool-registry` |
| `refactor/` | Code restructuring (no behavior change) | `refactor/ai-error-types` |
| `test/` | Test additions or fixes | `test/wasm-integration` |

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <short summary>

<optional body>
```

**Types**: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `ci`

**Scopes**: `engine`, `host`, `readline`, `ai`, `docs`, `build`

Examples:

```
feat(ai): add tool-call streaming support
fix(readline): handle multi-byte UTF-8 in cursor positioning
docs(adr): ADR-002 tool registry pattern
test(engine): add builtin cd path-resolution tests
```

## Pull Request Process

1. **Create a branch** following the naming convention above.
2. **Keep PRs focused** — one logical change per PR.
3. **Write or update tests** for any behavior change.
4. **Update documentation** if the change affects user-facing behavior or architecture.
5. **Fill out the PR template** (auto-loaded from `.github/PULL_REQUEST_TEMPLATE.md`).
6. **Request review** from at least one team member.
7. **Squash-merge** into `main` once approved.

## Code Review Expectations

- **Reviewers** should respond within one business day.
- Focus on correctness, clarity, and adherence to project conventions.
- Use conventional comment prefixes: `nit:`, `question:`, `suggestion:`, `blocking:`.

## Test Requirements

All PRs must pass the existing test suite:

```bash
./sbh test
```

If adding new functionality, include tests in the appropriate crate:

| Change Area | Test Command |
|-------------|-------------|
| WASM engine | `./sbh test engine` |
| Host / REPL | `./sbh test host` |
| Readline | `./sbh test readline` |
| AI features | `./sbh test ai` |

## Code Style

- Follow Rust 2021 edition idioms.
- Run `cargo clippy` before submitting.
- Keep AI logic isolated in the `features/ai/` crate (SEA layers).
- No `unwrap()` outside of tests.

## Questions?

Open a GitHub issue or see [SUPPORT.md](SUPPORT.md).
