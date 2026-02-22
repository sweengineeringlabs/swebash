# Manual Git Safety Gate Tests

> **TLDR:** Manual test checklist for the first-run setup wizard, branch pipeline, and per-branch safety gates.

**Audience**: Developers, QA

**WHAT**: Manual test procedures for git safety gates and the setup wizard
**WHY**: Validates branch protection enforcement that prevents accidental destructive operations on protected branches
**HOW**: Step-by-step test tables with expected outcomes

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Setup Wizard](#1-setup-wizard)
- [Config Persistence](#2-config-persistence)
- [Branch Pipeline](#3-branch-pipeline)
- [Gate Enforcement — Commit](#4-gate-enforcement--commit)
- [Gate Enforcement — Push](#5-gate-enforcement--push)
- [Gate Enforcement — Force-Push](#6-gate-enforcement--force-push)
- [Gate Enforcement — Merge/Rebase](#7-gate-enforcement--mergerebase)
- [Gate Override](#8-gate-override)
- [Passthrough Commands](#9-passthrough-commands)
- [Per-Repo Config Override](#10-per-repo-config-override)
- [Re-Run Wizard](#11-re-run-wizard)
- [Edge Cases](#12-edge-cases)
- [Branch Creation Gate](#13-branch-creation-gate)

---

## Prerequisites

1. swebash binary built: `./sbh build`
2. A git repository to test in (the wizard can create one)
3. Delete `~/.config/swebash/config.toml` to trigger the first-run wizard (or run the `setup` command)

---

## 1. Setup Wizard

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 1 | Wizard triggers on first run | Delete `~/.config/swebash/config.toml`, launch swebash | Wizard banner appears: "swebash - First-Run Setup Wizard" |
| 2 | Wizard skip prompt | At "Continue with setup?" type `n` | Wizard skipped, `setup_completed = true` saved, REPL starts |
| 3 | Wizard skip via keyword | At any prompt, type `skip` | Wizard aborts gracefully, REPL starts |
| 4 | Git repo detection | Run wizard inside a git repo | Step 1 shows "Git repository detected at: /path/to/repo" |
| 5 | Git repo missing | Run wizard outside a git repo | Step 1 offers "Initialize a new git repository here?" |
| 6 | Git init accepted | At git init prompt, type `y` | Repository initialized, step proceeds |
| 7 | Git init declined | At git init prompt, type `n` | "Skipping git init" printed, wizard continues |
| 8 | User ID auto-detected | Run wizard in repo with `git config user.name` set | Step 2 shows "Detected git user: <name>" |
| 9 | User ID manual entry | At user ID prompt, type custom name | Custom name used for pipeline (sanitized if needed) |
| 10 | User ID sanitization | Enter user ID with spaces: `John Doe` | Sanitized to `John_Doe`, message shown |
| 11 | Pipeline displayed | Reach step 3 | 6-branch pipeline shown: main, dev_{user_id}, test, integration, uat, staging-prod |
| 12 | Pipeline accepted | At "Accept this pipeline?" type `y` | Default pipeline used |
| 13 | Pipeline customized | At "Accept this pipeline?" type `n` | Interactive prompts for each branch name and protection flag |
| 14 | Gate matrix displayed | Reach step 4 | Gate matrix table shown with all 6 branches and 4 gate columns |
| 15 | Gate matrix accepted | At "Accept these gate rules?" type `y` | Default gates used |
| 16 | Gate matrix customized | At "Accept these gate rules?" type `n` | Interactive per-branch, per-operation prompts |
| 17 | Branch listing | Reach step 5 | Existing branches shown with checkmark, missing branches shown with exclamation |
| 18 | Missing branches created | At "Create missing branches?" type `y` | Missing branches created from HEAD |
| 19 | Missing branches skipped | At "Create missing branches?" type `n` | "Skipping branch creation" printed |

## 2. Config Persistence

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 20 | Global config saved | Complete wizard | `~/.config/swebash/config.toml` written with `setup_completed = true` |
| 21 | Per-repo config saved | Complete wizard inside a git repo | `.swebash/git.toml` written in repo root |
| 22 | Global config contains git section | Inspect `~/.config/swebash/config.toml` | `[git]` section present with user_id, pipeline, and gates |
| 23 | Per-repo config matches | Inspect `.swebash/git.toml` | Same git config as global config |
| 24 | Wizard does not re-trigger | Restart swebash after wizard completion | No wizard prompt; REPL starts directly |
| 25 | Config survives restart | Restart swebash, inspect behavior | Git gates still enforced from saved config |

## 3. Branch Pipeline

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 26 | Default pipeline branches | Complete wizard with user ID `alice` | Pipeline: main, dev_alice, test, integration, uat, staging-prod |
| 27 | Dev branch naming | Complete wizard with user ID `bob_123` | Dev branch named `dev_bob_123` |
| 28 | Protected flags | Inspect saved config | main=protected, dev=open, test=protected, integration=protected, uat=protected, staging-prod=protected |

## 4. Gate Enforcement -- Commit

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 29 | Commit blocked on main | Checkout main, `git add .`, `git commit -m "test"` | Warning printed: "git commit on protected branch 'main' is restricted" |
| 30 | Commit allowed on dev | Checkout dev_{user}, `git add .`, `git commit -m "test"` | Commit succeeds without any gate message |
| 31 | Commit blocked on test | Checkout test, stage file, `git commit -m "test"` | Warning printed with override prompt |
| 32 | Commit blocked on integration | Checkout integration, stage file, commit | Warning printed with override prompt |
| 33 | Commit blocked on uat | Checkout uat, stage file, commit | Warning printed with override prompt |
| 34 | Commit blocked on staging-prod | Checkout staging-prod, stage file, commit | Warning printed with override prompt |

## 5. Gate Enforcement -- Push

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 35 | Push blocked on main | On main, `git push origin main` | Warning: "git push on protected branch 'main' is restricted" |
| 36 | Push allowed on dev | On dev_{user}, `git push origin dev_{user}` | Push proceeds (may fail for other reasons like no remote, but no gate block) |

## 6. Gate Enforcement -- Force-Push

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 37 | Force-push denied on main (--force) | On main, `git push --force origin main` | Error: "git force-push on protected branch 'main' is denied" (no override option) |
| 38 | Force-push denied on main (-f) | On main, `git push -f origin main` | Same denial message |
| 39 | Force-push denied (--force-with-lease) | On main, `git push --force-with-lease origin main` | Same denial message |
| 40 | Force-push allowed on dev | On dev_{user}, `git push --force origin dev_{user}` | No gate block |

## 7. Gate Enforcement -- Merge/Rebase

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 41 | Merge blocked on main | On main, `git merge dev_{user}` | Warning: "git merge on protected branch 'main' is restricted" |
| 42 | Rebase blocked on main | On main, `git rebase dev_{user}` | Warning: "git rebase on protected branch 'main' is restricted" |
| 43 | Merge allowed on dev | On dev_{user}, `git merge main` | No gate block |

## 8. Gate Override

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 44 | Override accepted | On main, `git commit -m "x"`, at prompt type `yes` | Commit proceeds |
| 45 | Override rejected | On main, `git commit -m "x"`, at prompt type `no` | "Operation cancelled" printed, commit aborted |
| 46 | Override rejected (empty) | On main, `git commit -m "x"`, press Enter | Operation cancelled (non-"yes" = cancel) |
| 47 | Override rejected (random) | On main, `git commit -m "x"`, type `maybe` | Operation cancelled |
| 48 | Deny has no override | On main, `git push --force`, attempt to type `yes` | No prompt shown; operation denied immediately |

## 9. Passthrough Commands

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 49 | git status always allowed | On protected branch, `git status` | Output shown, no gate interference |
| 50 | git log always allowed | On protected branch, `git log --oneline` | Output shown, no gate interference |
| 51 | git diff always allowed | On protected branch, `git diff` | Output shown, no gate interference |
| 52 | git branch always allowed | On protected branch, `git branch -a` | Output shown, no gate interference |
| 53 | git stash always allowed | On protected branch, `git stash` | Output shown, no gate interference |
| 54 | git checkout always allowed | On protected branch, `git checkout dev_{user}` | Branch switches normally |
| 55 | git fetch always allowed | On protected branch, `git fetch` | Runs normally, no gate interference |

## 10. Per-Repo Config Override

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 56 | Per-repo overrides global | Set global config to allow commit on main; set `.swebash/git.toml` to deny commit on main; try `git commit` on main | Denied (per-repo wins) |
| 57 | Per-repo adds new branch | Add `[[gates]]` for `feature` branch in `.swebash/git.toml`; try `git commit` on `feature` | Gate applies per .swebash/git.toml rule |
| 58 | No per-repo falls through | Delete `.swebash/git.toml`; try operations | Global config gates apply |

## 11. Re-Run Wizard

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 59 | Setup command exists | Type `setup` in REPL | Wizard starts (not treated as external command) |
| 60 | Setup re-runs full wizard | Complete `setup` command | All 5 steps presented again |
| 61 | Config updated after re-run | Complete `setup` with different user ID | `~/.config/swebash/config.toml` updated with new user_id |

## 12. Edge Cases

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 62 | Not in git repo | Run `git commit` outside any git repo | No gate block (git itself will error) |
| 63 | Detached HEAD | `git checkout --detach HEAD`, then `git commit` | No gate block (no branch name to match) |
| 64 | Branch not in gate list | Checkout `feature/xyz` (not in pipeline), `git commit` | Commit allowed (no matching gate rule) |
| 65 | Non-git program | Run any non-git command (e.g. `ls`, `cat`) | No gate interference whatsoever |
| 66 | Empty git args | Run `git` with no subcommand | No gate block, git shows its help |
| 67 | Config missing git section | Delete `[git]` from config.toml, restart | No gates enforced, all operations allowed |

## 13. Branch Creation Gate

Users can only create branches that are listed in the config. This prevents ad-hoc branch proliferation.

| # | Test | Steps | Expected |
|---|------|-------|----------|
| 68 | Create allowed branch (checkout -b) | `git checkout -b dev_{user}` | Branch created successfully |
| 69 | Create allowed branch (switch -c) | `git switch -c main` | Branch created (or switches if exists) |
| 70 | Create allowed branch (branch cmd) | `git branch test` | Branch created successfully |
| 71 | Create disallowed branch (checkout -b) | `git checkout -b random-feature` | Error: "Cannot create branch 'random-feature'. Only configured branches are allowed." |
| 72 | Create disallowed branch (switch -c) | `git switch -c my-experiment` | Same denial message |
| 73 | Create disallowed branch (switch --create) | `git switch --create hotfix/123` | Same denial message |
| 74 | Create disallowed branch (branch cmd) | `git branch feature/new` | Same denial message |
| 75 | Denied message lists allowed branches | Try creating disallowed branch | Message includes: "Allowed branches: main, dev_{user}, test, integration, uat, staging-prod" |
| 76 | Branch deletion allowed | `git branch -d old-branch` | Deletion proceeds (not blocked by creation gate) |
| 77 | Branch deletion with -D allowed | `git branch -D force-delete` | Deletion proceeds |
| 78 | Checkout existing branch allowed | `git checkout main` (existing branch) | No gate block, checkout proceeds |
| 79 | Switch to existing branch allowed | `git switch dev_{user}` (existing branch) | No gate block, switch proceeds |

---

## Summary

| Category | Test Count |
|----------|-----------|
| Setup Wizard | 19 |
| Config Persistence | 6 |
| Branch Pipeline | 3 |
| Gate — Commit | 6 |
| Gate — Push | 2 |
| Gate — Force-Push | 4 |
| Gate — Merge/Rebase | 3 |
| Gate Override | 5 |
| Passthrough Commands | 7 |
| Per-Repo Override | 3 |
| Re-Run Wizard | 3 |
| Edge Cases | 6 |
| Branch Creation Gate | 12 |
| **Total** | **79** |
