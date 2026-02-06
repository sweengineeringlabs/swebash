# swebash Documentation

**Audience**: All team members

**WHAT**: Central navigation hub for all swebash documentation
**WHY**: Provides a single entry point so readers can find the right document for their role and task
**HOW**: Organized by SDLC phase with audience labels on every document

---

## Quick Links

| I want to... | Go to |
|--------------|-------|
| **Install swebash** | [Installation Guide](6-operation/installation.md) |
| **Configure AI providers** | [Configuration](6-operation/configuration.md) |
| **Create a custom agent** | [Creating Agents](6-operation/creating-agents.md) |
| **Set up a dev environment** | [Setup Guide](4-development/setup-guide.md) |
| **Understand the architecture** | [Architecture](3-design/architecture.md) |
| **Run tests** | [Manual Testing](5-testing/manual-testing.md) |
| **Look up a term** | [Glossary](glossary.md) |

---

## Documentation by Phase

### 3-Design

Architecture decisions and system design.

| Document | Description | Audience |
|----------|-------------|----------|
| [Architecture](3-design/architecture.md) | Three-crate architecture overview | Developers, architects |
| [Agent Architecture](3-design/agent-architecture.md) | Agent framework: prompts, tools, memory isolation | Developers, architects |
| [AI Integration](3-design/ai-integration.md) | LLM isolation boundary and provider abstraction | Developers, architects |
| [Command Design](3-design/command-design.md) | AI command triggers and shell-mode dispatch | Developers, architects |
| [Prompt Engineering](3-design/prompt-engineering.md) | System prompt design principles | Developers, architects |
| [ADR-001](3-design/ADR-001-agent-doc-context.md) | Agent documentation context strategy | Developers, architects |

### 4-Development

Implementation details, setup, and backlog.

| Document | Description | Audience |
|----------|-------------|----------|
| [Setup Guide](4-development/setup-guide.md) | Dev environment setup, build, and run | Contributors, developers |
| [AI Mode](4-development/ai-mode.md) | AI mode architecture and intent detection | Developers |
| [Arrow Keys](4-development/arrow-keys.md) | Arrow key navigation implementation | Developers |
| [History Feature](4-development/history-feature.md) | Persistent command history implementation | Developers |
| [Readline Enhancements](4-development/readline-enhancements.md) | Advanced readline features design | Developers |
| [Readline Phases](4-development/readline-phases.md) | Phases 7-12 implementation record | Developers |
| [Backlog](4-development/backlog.md) | Development backlog and task tracking | Developers, project leads |
| [Case Study](4-development/case_study_backlog.md) | SEA case study: eliminating cross-platform duplication | Developers, architects |

### 5-Testing

Test strategy, suites, and manual testing procedures.

| Document | Description | Audience |
|----------|-------------|----------|
| [Test Strategy](5-testing/strategy.md) | Arrow key navigation test coverage | Developers, QA |
| [E2E Testing](5-testing/e2e-testing.md) | End-to-end and integration test implementation | Developers, QA |
| [AI Mode Tests](5-testing/ai-mode-tests.md) | AI mode test suite (38 tests) | Developers, QA |
| [Manual Testing](5-testing/manual-testing.md) | Manual testing guide with prerequisites and procedures | Developers, QA |

### 6-Operation

Installation, configuration, and user-facing guides.

| Document | Description | Audience |
|----------|-------------|----------|
| [Installation](6-operation/installation.md) | System requirements and installation steps | Users, DevOps |
| [Configuration](6-operation/configuration.md) | Environment variables and provider setup | Users, DevOps |
| [Creating Agents](6-operation/creating-agents.md) | Custom agent creation via YAML | Users, DevOps |

### Other

| Document | Description | Audience |
|----------|-------------|----------|
| [Changelog](changelog.md) | Release history | All |
| [Glossary](glossary.md) | Domain terminology | All |
