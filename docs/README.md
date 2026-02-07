# swebash Documentation

> **TLDR:** Central navigation hub for all swebash documentation, organized by SDLC phase.

**Audience**: All team members

**WHAT**: Central navigation hub for all swebash documentation
**WHY**: Provides a single entry point so readers can find the right document for their role and task
**HOW**: Organized by SDLC phase with audience labels on every document

---

## Quick Links

| I want to... | Go to |
|--------------|-------|
| **Install swebash** | [Installation Guide](7-operation/installation.md) |
| **Configure AI providers** | [Configuration](7-operation/configuration.md) |
| **Create a custom agent** | [Creating Agents](7-operation/creating_agents.md) |
| **Set up a dev environment** | [Setup Guide](4-development/setup_guide.md) |
| **Understand the architecture** | [Architecture](3-design/architecture.md) |
| **Run tests** | [Manual Testing](5-testing/manual_testing.md) |
| **Look up a term** | [Glossary](glossary.md) |

---

## Documentation by Phase

### 0-Ideation

Research notes and feasibility studies.

| Document | Description | Audience |
|----------|-------------|----------|
| [README](0-ideation/README.md) | Phase overview | Architects, project leads |

### 1-Requirements

Business and functional requirements.

| Document | Description | Audience |
|----------|-------------|----------|
| [README](1-requirements/README.md) | Phase overview | Project leads, developers |

### 2-Planning

Roadmap and sprint planning.

| Document | Description | Audience |
|----------|-------------|----------|
| [README](2-planning/README.md) | Phase overview | Project leads, developers |

### 3-Design

Architecture decisions and system design.

| Document | Description | Audience |
|----------|-------------|----------|
| [Architecture](3-design/architecture.md) | Three-crate architecture overview | Developers, architects |
| [Agent Architecture](3-design/agent_architecture.md) | Agent framework: prompts, tools, memory isolation | Developers, architects |
| [AI Integration](3-design/ai_integration.md) | LLM isolation boundary and provider abstraction | Developers, architects |
| [Command Design](3-design/command_design.md) | AI command triggers and shell-mode dispatch | Developers, architects |
| [Prompt Engineering](3-design/prompt_engineering.md) | System prompt design principles | Developers, architects |
| [ADR-001](3-design/ADR-001-agent-doc-context.md) | Agent documentation context strategy | Developers, architects |
| [ADR Index](3-design/adr/README.md) | Architecture decision record index and template | Developers, architects |

### 4-Development

Implementation details, setup, and backlog.

| Document | Description | Audience |
|----------|-------------|----------|
| [Developer Guide](4-development/developer_guide.md) | Day-to-day workflow, conventions, common tasks | Contributors, developers |
| [Setup Guide](4-development/setup_guide.md) | Dev environment setup, build, and run | Contributors, developers |
| [AI Mode](4-development/ai_mode.md) | AI mode architecture and intent detection | Developers |
| [Arrow Keys](4-development/arrow_keys.md) | Arrow key navigation implementation | Developers |
| [History Feature](4-development/history_feature.md) | Persistent command history implementation | Developers |
| [Readline Enhancements](4-development/readline_enhancements.md) | Advanced readline features design | Developers |
| [Readline Phases](4-development/readline_phases.md) | Phases 7-12 implementation record | Developers |
| [Backlog](4-development/backlog.md) | Development backlog and task tracking | Developers, project leads |
| [Case Study](4-development/case_study_backlog.md) | SEA case study: eliminating cross-platform duplication | Developers, architects |

### 5-Testing

Test strategy, suites, and manual testing procedures.

| Document | Description | Audience |
|----------|-------------|----------|
| [Test Strategy](5-testing/testing_strategy.md) | Arrow key navigation test coverage | Developers, QA |
| [E2E Testing](5-testing/e2e_testing.md) | End-to-end and integration test implementation | Developers, QA |
| [AI Mode Tests](5-testing/ai_mode_tests.md) | AI mode test suite (38 tests) | Developers, QA |
| [Manual Testing](5-testing/manual_testing.md) | Manual testing guide with prerequisites and procedures | Developers, QA |

### 6-Deployment

CI/CD pipelines and release procedures.

| Document | Description | Audience |
|----------|-------------|----------|
| [README](6-deployment/README.md) | Phase overview | DevOps, contributors |

### 7-Operation

Installation, configuration, and user-facing guides.

| Document | Description | Audience |
|----------|-------------|----------|
| [Installation](7-operation/installation.md) | System requirements and installation steps | Users, DevOps |
| [Configuration](7-operation/configuration.md) | Environment variables and provider setup | Users, DevOps |
| [Creating Agents](7-operation/creating_agents.md) | Custom agent creation via YAML | Users, DevOps |

### Reference

| Document | Description | Audience |
|----------|-------------|----------|
| [Changelog](../CHANGELOG.md) | Release history | All |
| [Glossary](glossary.md) | Domain terminology | All |
