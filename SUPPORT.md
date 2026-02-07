# Support

**Audience**: All team members

**WHAT**: Where to get help with swebash
**WHY**: Central reference so contributors and users know where to ask questions
**HOW**: GitHub issues, documentation links, and team contacts

---

## Getting Help

### GitHub Issues

Open an issue for:
- Bug reports — use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md)
- Feature requests — use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md)
- Questions about architecture or design decisions

### Documentation

| Question | Resource |
|----------|----------|
| How do I install swebash? | [Installation Guide](docs/7-operation/installation.md) |
| How do I set up a dev environment? | [Setup Guide](docs/4-development/setup_guide.md) |
| How does the architecture work? | [Architecture](docs/3-design/architecture.md) |
| How do I configure AI providers? | [Configuration](docs/7-operation/configuration.md) |
| How do I create a custom agent? | [Creating Agents](docs/7-operation/creating_agents.md) |
| What does a term mean? | [Glossary](docs/glossary.md) |

### FAQ

**Q: The shell starts but AI commands don't work.**
A: Check that `.env` contains a valid API key and that `LLM_PROVIDER` is set. See [Configuration](docs/7-operation/configuration.md).

**Q: Build fails with "registry index was not found".**
A: Run `./sbh setup && source ~/.bashrc`. See [Setup Guide](docs/4-development/setup_guide.md#local-registry-setup).

**Q: Tests fail with "engine.wasm not found".**
A: Run `./sbh build` before `./sbh test`. The WASM module must be compiled first.
