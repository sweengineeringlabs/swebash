# Tab Manual Test Report — 2026-02-10

> Test run against `manual_tab_tests.md` (all 11 sections) using `sbh build --release` + piped commands.

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

Full tab system test across all 11 sections of `manual_tab_tests.md`. Keyboard shortcut tests (section 35) require an interactive terminal and could not be run via piped input. Sandbox was active, so CWD isolation tests used workspace-internal directories instead of `/tmp`.

---

## 29. Tab Commands — Basics (5 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 1 | List default | **PASS** | `*1  [>:~/workspace]` |
| 2 | List alias (`tab list`) | **PASS** | Same output as `tab` |
| 3 | Unknown subcommand | **PASS** | `tab: unknown subcommand 'foo'` |
| 4 | Tab zero | **PASS** | `tab: invalid tab number` |
| 5 | Tab out of range | **PASS** | `tab: no tab 99` |

---

## 30. Tab Lifecycle (8 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 6 | New tab | **PASS** | `Switched to tab 2.` |
| 7 | Tab list after new | **PASS** | 2 tabs shown; tab 2 active |
| 8 | Switch to tab 1 | **PASS** | `Switched to tab 1.` |
| 9 | Switch back | **PASS** | `Switched to tab 2.` |
| 10 | Close active tab | **PASS** | `Closed tab 2. Now on tab 1.` |
| 11 | Close last tab | **PASS** | `Last tab closed. Exiting.` |
| 12 | Exit closes tab | **PASS** | Tab 2 closed, switched to tab 1 |
| 13 | Exit on last tab | **PASS** | Shell exits cleanly |

---

## 31. CWD Isolation (4 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 14 | Independent CWD | **PASS** | Tab 1 retains original CWD after `cd` in tab 2 (used workspace-internal dirs due to sandbox) |
| 15 | CD in tab 1 | **PASS** | Tab 2 CWD unchanged by tab 1's `cd` |
| 16 | Prompt reflects CWD | **PASS** | Prompt shows per-tab CWD |
| 17 | Spawned process uses virtual CWD | **PASS** | `/bin/pwd` prints tab's virtual CWD |

---

## 32. Environment Isolation (3 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 18 | Independent env | **PASS** | Tab 1 does not see `FOO=bar` set in tab 2 |
| 19 | Env in original tab | **PASS** | Tab 2 retains `FOO=bar` |
| 20 | Process env inheritance | **PASS** | External process receives virtual env var |

---

## 33. Context Tabs (4 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 21 | New tab with path | **PASS** | Tab created with specified CWD |
| 22 | PWD in context tab | **PASS** | `pwd` prints the specified path |
| 23 | Invalid path | **PASS** | `tab new: not a directory: /no/such/dir` |
| 24 | Relative path | **PASS** | Tab created with resolved CWD |

---

## 34. Tab Bar UI (7 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 25 | No bar with 1 tab | **PASS** | No tab bar output with single tab |
| 26 | Bar appears at 2 tabs | **PASS** | Tab bar rendered with ANSI escape codes |
| 27 | Active tab highlighted | **PASS** | Bold white ANSI code on active tab |
| 28 | Bar updates on switch | **PASS** | Active marker moves |
| 29 | Bar updates on CD | **PASS** | Label updates to new CWD |
| 30 | Bar disappears | **PASS** | Scroll region reset when back to 1 tab |
| 31 | Truncation | **SKIP** | Cannot verify truncation via piped input (needs wide/narrow terminal) |

---

## 35. Keyboard Shortcuts (9 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 32–40 | All keyboard shortcut tests | **SKIP** | Require interactive terminal with real key events; not testable via piped input |

---

## 36. AI Mode Tabs (7 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 41 | Create AI tab | **PASS** | `Opened AI tab 2 (agent: shell).` |
| 42 | AI tab prompt | **PASS** | `[AI:shell]` prompt displayed |
| 43 | AI tab with agent | **PASS** | `Opened AI tab 3 (agent: review).` |
| 44 | AI status in tab | **PASS** | AI service status shown |
| 45 | Exit AI tab | **PASS** | Tab closed, returns to previous tab |
| 46 | Quit AI tab | **PASS** | Same as exit |
| 47 | AI tab in tab list | **PASS** | AI tab shown with `AI` icon |

---

## 37. History View Tabs (5 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 48 | Create history tab | **PASS** | `Opened history tab N.` |
| 49 | History prompt | **PASS** | `[history] search>` prompt shown |
| 50 | Show all history | **PASS** | History entries listed |
| 51 | Exit history tab | **PASS** | Tab closed |
| 52 | History tab in list | **PASS** | `H` icon shown in tab list |

---

## 38. Tab Rename and Labels (5 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 53 | Rename tab | **PASS** | `Tab renamed to 'my-project'.` |
| 54 | Renamed in list | **PASS** | Custom label `[>:my-project]` shown |
| 55 | Renamed in bar | **PASS** | Tab bar shows custom label |
| 56 | Rename usage | **PASS** | `usage: tab rename <name>` |
| 57 | Rename with spaces | **PASS** | `Tab renamed to 'my cool tab'.` |

---

## 39. Edge Cases (5 tests)

| # | Test | Result | Notes |
|---|------|--------|-------|
| 58 | Ctrl+D on multiline | **SKIP** | Requires interactive terminal |
| 59 | Ctrl+D on empty | **PASS** | Tab closed on EOF |
| 60 | Many tabs | **PASS** | 10+ tabs created without crash |
| 61 | Rapid switching | **PASS** | No crash or state corruption |
| 62 | WASM isolation | **PASS** | Each tab processes commands independently |

---

## Summary

| Section | Pass | Skip | Fail | Total |
|---------|------|------|------|-------|
| 29. Basics | 5 | 0 | 0 | 5 |
| 30. Lifecycle | 8 | 0 | 0 | 8 |
| 31. CWD Isolation | 4 | 0 | 0 | 4 |
| 32. Environment Isolation | 3 | 0 | 0 | 3 |
| 33. Context Tabs | 4 | 0 | 0 | 4 |
| 34. Tab Bar UI | 6 | 1 | 0 | 7 |
| 35. Keyboard Shortcuts | 0 | 9 | 0 | 9 |
| 36. AI Mode Tabs | 7 | 0 | 0 | 7 |
| 37. History View Tabs | 5 | 0 | 0 | 5 |
| 38. Tab Rename | 5 | 0 | 0 | 5 |
| 39. Edge Cases | 4 | 1 | 0 | 5 |
| **Total** | **51** | **11** | **0** | **62** |

**Verdict**: 51/62 pass, 11 skipped (all require interactive terminal — keyboard shortcuts and truncation). Zero failures.
