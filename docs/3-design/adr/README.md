# Architecture Decision Records

> **TLDR:** Index and template for all swebash architecture decision records (ADRs).

**Audience**: Developers, architects

**WHAT**: Index of architecture decision records (ADRs) for swebash
**WHY**: Provides a single reference for all architectural decisions, their status, and rationale
**HOW**: Table listing each ADR with status, date, and summary

---

## Table of Contents

- [ADR Index](#adr-index)
- [Creating a New ADR](#creating-a-new-adr)
- [Context](#context)
- [Decision Drivers](#decision-drivers)
- [Options Considered](#options-considered)
- [Decision](#decision)
- [Consequences](#consequences)


## ADR Index

| ID | Title | Status | Date | Summary |
|----|-------|--------|------|---------|
| [ADR-001](../ADR-001-agent-doc-context.md) | Agent Documentation Context Strategy | Proposed | 2026-02-05 | Use `docs` field with token-budgeted pre-loading instead of fs tool reads or RAG |

## Creating a New ADR

1. Copy the template below into a new file: `docs/3-design/ADR-{NNN}-{short-title}.md`
2. Fill in all sections.
3. Add an entry to the index table above.
4. Submit via PR for team review.

### Template

```markdown
# ADR-{NNN}: {Title}

**Audience**: Developers, architects
**Status:** Proposed | Accepted | Deprecated | Superseded by ADR-{NNN}
**Date:** YYYY-MM-DD
**Authors:** {names}
**Reviewers:** {names}

## Context

{What is the issue? Why does it need a decision?}

## Decision Drivers

{Numbered list of key criteria}

## Options Considered

### A. {Option name}

{Description and assessment table}

### B. {Option name}

{Description and assessment table}

## Decision

{Which option was chosen and why}

## Consequences

### Positive
### Negative
### Risks
```
