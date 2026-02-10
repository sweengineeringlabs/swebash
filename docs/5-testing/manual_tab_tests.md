# Manual Tab Tests

> **TLDR:** Manual test checklist for the tab system: tab commands, CWD isolation, tab bar UI, keyboard shortcuts, and mode tabs (AI, History).

**Audience**: Developers, QA

**WHAT**: Manual test procedures for multi-tab shell features
**WHY**: Validates tab lifecycle, per-tab CWD/env isolation, keyboard navigation, and mode tab dispatch
**HOW**: Step-by-step test tables with expected outcomes

---

## Table of Contents

- [Tab Commands — Basics](#29-tab-commands--basics)
- [Tab Lifecycle](#30-tab-lifecycle)
- [CWD Isolation](#31-cwd-isolation)
- [Environment Isolation](#32-environment-isolation)
- [Context Tabs](#33-context-tabs)
- [Tab Bar UI](#34-tab-bar-ui)
- [Keyboard Shortcuts](#35-keyboard-shortcuts)
- [AI Mode Tabs](#36-ai-mode-tabs)
- [History View Tabs](#37-history-view-tabs)
- [Tab Rename and Labels](#38-tab-rename-and-labels)
- [Edge Cases](#39-edge-cases)

---

## 29. Tab Commands — Basics

| Test | Command | Expected |
|------|---------|----------|
| List default | `tab` | Shows one tab: `*1  [>:~/workspace]` (active marker, shell icon, CWD) |
| List alias | `tab list` | Same output as `tab` |
| Unknown subcommand | `tab foo` | Prints `tab: unknown subcommand 'foo'` |
| Tab zero | `tab 0` | Prints `tab: invalid tab number` |
| Tab out of range | `tab 99` | Prints `tab: no tab 99` |

## 30. Tab Lifecycle

| Test | Command | Expected |
|------|---------|----------|
| New tab | `tab new` | Creates a new shell tab, prints `Switched to tab 2.` |
| Tab list after new | `tab list` | Shows 2 tabs; tab 2 is active (`*`) |
| Switch to tab 1 | `tab 1` | Prints `Switched to tab 1.` |
| Switch back | `tab 2` | Prints `Switched to tab 2.` |
| Close active tab | `tab close` | Prints `Closed tab 2. Now on tab 1.` |
| Close last tab | Create only 1 tab, run `tab close` | Shell exits (prints `Last tab closed. Exiting.`) |
| Exit closes tab | Open 2 tabs, run `exit` on tab 2 | Tab 2 closes, switches to tab 1 |
| Exit on last tab | Run `exit` with only 1 tab open | Shell exits cleanly |

## 31. CWD Isolation

| Test | Steps | Expected |
|------|-------|----------|
| Independent CWD | 1. `tab new` (tab 2) 2. `cd /tmp` 3. `pwd` → `/tmp` 4. `tab 1` 5. `pwd` | Tab 1 still shows original workspace CWD (not `/tmp`) |
| CD in tab 1 | 1. `cd /tmp` (tab 1) 2. `tab 2` 3. `pwd` | Tab 2 still shows its own CWD (unchanged by tab 1's `cd`) |
| Prompt reflects CWD | 1. `tab new` 2. `cd /tmp` | Prompt changes to `/tmp/>` for tab 2 only |
| Spawned process uses virtual CWD | 1. `cd /tmp` 2. `/bin/pwd` | External command prints `/tmp` (uses virtual CWD, not process CWD) |

## 32. Environment Isolation

| Test | Steps | Expected |
|------|-------|----------|
| Independent env | 1. `tab new` (tab 2) 2. `export FOO=bar` 3. `tab 1` 4. `env \| grep FOO` | Tab 1 does not see `FOO=bar` |
| Env in original tab | 1. `tab 2` 2. `env \| grep FOO` | Tab 2 still has `FOO=bar` |
| Process env inheritance | 1. `export MY_VAR=hello` 2. `/bin/echo $MY_VAR` | External process receives the virtual env var |

## 33. Context Tabs

| Test | Command | Expected |
|------|---------|----------|
| New tab with path | `tab new /tmp` | Creates shell tab with CWD `/tmp`; prints `Switched to tab N.` |
| PWD in context tab | `pwd` (in newly created tab) | Prints `/tmp` |
| Invalid path | `tab new /no/such/dir` | Prints `tab new: not a directory: /no/such/dir` |
| Relative path | `tab new ..` | Creates tab with CWD one level up from current tab's CWD |

## 34. Tab Bar UI

| Test | Steps | Expected |
|------|-------|----------|
| No bar with 1 tab | Start shell with default single tab | No tab bar visible at terminal top |
| Bar appears at 2 tabs | `tab new` | Tab bar appears at terminal row 0: `[1:>:~]  [2:>:~]` |
| Active tab highlighted | Observe tab bar | Active tab is bold white; inactive is grey |
| Bar updates on switch | `tab 1` | Active marker moves to tab 1 in the bar |
| Bar updates on CD | `cd /tmp` | Tab bar label for active tab updates to show `/tmp` |
| Bar disappears | Close tabs until 1 remains | Tab bar disappears, scroll region resets |
| Truncation | Open many tabs until labels exceed terminal width | Bar shows `...` for overflow tabs |

## 35. Keyboard Shortcuts

| Test | Key | Expected |
|------|-----|----------|
| New tab shortcut | `Ctrl+T` | Creates a new shell tab (same as `tab new`) |
| Next tab | `Ctrl+PageDown` | Switches to next tab (wraps around) |
| Previous tab | `Ctrl+PageUp` | Switches to previous tab (wraps around) |
| Goto tab 1 | `Alt+1` | Switches to tab 1 |
| Goto tab 2 | `Alt+2` | Switches to tab 2 (if exists) |
| Goto tab 9 | `Alt+9` | Switches to tab 9 (if exists); prints error if not |
| Close tab shortcut | `Ctrl+T` then `tab close` | Creates and then closes a tab (no dedicated close shortcut — use command) |
| Wrap around next | With 2 tabs, on tab 2: `Ctrl+PageDown` | Wraps to tab 1 |
| Wrap around prev | With 2 tabs, on tab 1: `Ctrl+PageUp` | Wraps to tab 2 |

## 36. AI Mode Tabs

> Requires AI service configured (API key + `LLM_PROVIDER`).

| Test | Command | Expected |
|------|---------|----------|
| Create AI tab | `tab ai` | Opens AI tab with default agent `shell`; prints `Opened AI tab N (agent: shell).` |
| AI tab prompt | (observe prompt in AI tab) | Shows `[AI:shell] >` prompt |
| AI tab with agent | `tab ai review` | Opens AI tab with agent `review`; prompt shows `[AI:review] >` |
| AI status in tab | `ai status` (in AI tab) | Shows AI service status |
| Exit AI tab | `exit` (in AI tab) | Closes the AI tab, returns to previous tab |
| Quit AI tab | `quit` (in AI tab) | Same as `exit` — closes the AI tab |
| AI tab in tab list | `tab list` (from another tab) | AI tab shows with `AI` icon: `[AI:shell]` |
| AI tab CWD | `tab ai` then observe tab bar | AI tab inherits CWD from the tab that created it |

## 37. History View Tabs

| Test | Command | Expected |
|------|---------|----------|
| Create history tab | `tab history` | Opens history tab; prints `Opened history tab N.` |
| History prompt | (observe prompt) | Shows `[history] search>` prompt |
| Show all history | Press Enter (empty input) | Lists all history entries with line numbers |
| Search history | Type a search term, press Enter | Shows only matching entries |
| No matches | Search for nonexistent term | Prints `(no matching history entries)` |
| Exit history tab | `exit` (in history tab) | Closes the history tab |
| Quit history tab | `quit` (in history tab) | Same as `exit` |
| Q closes history | `q` (in history tab) | Closes the history tab |
| History tab in list | `tab list` (from another tab) | History tab shows with `H` icon |
| Shared history | Run commands in tab 1, open `tab history` | History tab sees commands from all tabs |

## 38. Tab Rename and Labels

| Test | Command | Expected |
|------|---------|----------|
| Rename tab | `tab rename my-project` | Prints `Tab renamed to 'my-project'.` |
| Renamed in list | `tab list` | Shows custom label: `[>:my-project]` instead of CWD |
| Renamed in bar | (with 2+ tabs, observe tab bar) | Tab bar shows custom label |
| Rename usage | `tab rename` | Prints `usage: tab rename <name>` |
| Rename with spaces | `tab rename my cool tab` | Prints `Tab renamed to 'my cool tab'.` |

## 39. Edge Cases

| Test | Steps | Expected |
|------|-------|----------|
| Ctrl+D on multiline | 1. Start a multiline command (e.g. `echo \`) 2. Press `Ctrl+D` | Clears multiline buffer, does not close tab |
| Ctrl+D on empty | Press `Ctrl+D` with empty input | Closes current tab (exits if last) |
| Many tabs | Open 10+ tabs via `tab new` | All tabs work; tab list shows all; tab bar truncates |
| Rapid switching | Quickly alternate `tab 1` / `tab 2` | No crash or state corruption |
| WASM isolation | 1. `tab new` 2. Run `echo hello` in tab 2 3. `tab 1` 4. Run `echo world` | Each tab's WASM engine processes commands independently |

---

## See Also

- [Manual Testing Hub](manual_testing.md) — prerequisites and setup
- [Manual Shell Tests](manual_shell_tests.md) — core shell feature tests
- [Manual AI Tests](manual_ai_tests.md) — AI feature tests
