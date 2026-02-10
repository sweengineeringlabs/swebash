# AI Manual Test Report — 2026-02-10

> Test run against `manual_ai_tests.md` (smoke subset) using `sbh build --release` + piped commands.

---

## Environment

| Item | Value |
|------|-------|
| Date | 2026-02-10 |
| Branch | `dev` |
| Commit | `5a1e23e` (docs update for YAML agent config extraction) |
| LLM Provider | Anthropic |
| Build | Release (`./sbh build --release`) |
| Platform | WSL2 / Linux 6.6.87.2-microsoft-standard-WSL2 |
| Method | Non-interactive piped commands (`printf ... \| swebash 2>/dev/null`) |

---

## Scope

Smoke test of agent infrastructure after the YAML agent config extraction refactor. Focused on agent listing, switching, and AI status — not full LLM-dependent tests.

---

## Agent Listing (1 test)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 1 | `ai agents` lists all agents | **PASS** | All 11 agents listed: shell, review, devops, git, security, explain, clitester, apitester, seaaudit, rscagent, ragtest |

---

## Agent Switching (2 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 2 | `@review` switches agent | **PASS** | Output: `Switched to Code Reviewer (review)` |
| 3 | `@git` switches agent | **PASS** | Output: `Switched to Git Assistant (git)` |

---

## AI Status (1 test)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 4 | `ai status` shows service info | **PASS** | Enabled: yes, Provider: anthropic, Ready: yes |

---

## Summary

| Category | Pass | Skip | Fail | Total |
|----------|------|------|------|-------|
| Agent listing | 1 | 0 | 0 | 1 |
| Agent switching | 2 | 0 | 0 | 2 |
| AI status | 1 | 0 | 0 | 1 |
| **Total** | **4** | **0** | **0** | **4** |

**Verdict**: All tested scenarios pass. Agent infrastructure intact after YAML agent config extraction refactor.
