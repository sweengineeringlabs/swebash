# Manual sbh Launcher Tests

> **TLDR:** Manual test checklist for the sbh/sbh.ps1 launcher: help, build, run, test dispatch, cargo registry, and gen-aws-docs.

**Audience**: Developers, QA

**WHAT**: Manual test procedures for the sbh launcher and its subcommands
**WHY**: Validates the primary entry point delegates correctly to build, test, and tooling scripts
**HOW**: Step-by-step test tables with expected outcomes

---

## Table of Contents

- [sbh help](#21-sbh-help)
- [sbh test](#22-sbh-test)
- [Cargo registry](#23-cargo-registry)
- [sbh build & run](#24-sbh-build--run)
- [gen-aws-docs](#24b-gen-aws-docs)
- [sbh.ps1 help](#25-sbhps1-help-powershell)
- [sbh.ps1 test](#26-sbhps1-test-powershell)
- [sbh.ps1 setup](#27-sbhps1-setup-powershell)
- [sbh.ps1 build & run](#28-sbhps1-build--run-powershell)

---

## 21. sbh help

| Test | Command | Expected |
|------|---------|----------|
| Help flag | `./sbh --help` | Prints usage with all commands: setup, build, run, test, gen-aws-docs |
| Help command | `./sbh help` | Same output as `--help` |
| No args | `./sbh` | Prints usage and exits with code 0 (same as help) |
| Unknown command | `./sbh foo` | Prints usage and exits with code 1 |

## 22. sbh test

| Test | Command | Expected |
|------|---------|----------|
| All suites | `./sbh test` | Runs engine, readline, host, ai tests in order; all pass |
| Engine only | `./sbh test engine` | Runs engine tests only |
| Host only | `./sbh test host` | Runs host tests only |
| Readline only | `./sbh test readline` | Runs readline tests only |
| AI only | `./sbh test ai` | Runs AI tests only |
| Scripts only | `./sbh test scripts` | Runs bash script tests via `bin/tests/runner.sh` (feature + e2e `*.test.sh` files) |
| Help text matches suites | `./sbh --help` | Test suite list includes `engine|host|readline|ai|scripts|all` |

## 23. Cargo registry

The project depends on a local Cargo registry for rustratify crates. The test scripts verify the registry is configured and reachable before running tests.

| Test | Command | Expected |
|------|---------|----------|
| Registry set | `./sbh test engine` | First line prints `==> Registry: file:///...index (ok)` |
| Registry missing | `CARGO_REGISTRIES_LOCAL_INDEX=file:///nonexistent ./sbh test engine` | Prints `ERROR: Local registry index not found`, exits 1 |
| Registry unset | Unset `CARGO_REGISTRIES_LOCAL_INDEX` and remove from `.bashrc`, run `./sbh test engine` | `preflight` sets fallback path; verify it resolves |

## 24. sbh build & run

Both `sbh build` and `sbh run` resolve binaries via `TARGET_DIR` (default `/tmp/swebash-target`), which is also exported as `CARGO_TARGET_DIR` so that Cargo writes output to the same location the scripts read from. A custom target directory can be set with `TARGET_DIR=/path ./sbh build`.

| Test | Command | Expected |
|------|---------|----------|
| Release build | `./sbh build` | Builds engine WASM (release) and host (release) to `/tmp/swebash-target/release/` without errors |
| Debug build | `./sbh build --debug` | Builds engine WASM and host (debug) to `/tmp/swebash-target/debug/` without errors |
| Run release | `./sbh run --release` | Finds binary at `/tmp/swebash-target/release/swebash`; launches shell, shows banner and prompt. No rebuild if `./sbh build` (release) was already run. |
| Run debug | `./sbh run` | Finds binary at `/tmp/swebash-target/debug/swebash`; launches shell. Builds debug if not present. |
| Target dir override | `TARGET_DIR=/tmp/mydir ./sbh build && TARGET_DIR=/tmp/mydir ./sbh run --release` | Builds and runs from `/tmp/mydir/`; `CARGO_TARGET_DIR` is set automatically to match |
| Run without prior build | `rm -rf /tmp/swebash-target && ./sbh run --release` | Detects missing binary, builds release automatically, then launches shell |

## 24b. gen-aws-docs

Generates AWS reference docs from live CLI help output. Writes 3 markdown files to `~/.config/swebash/docs/aws/` (or `$SWEBASH_AWS_DOCS_DIR`). Offers to install the AWS CLI if not found.

> Requires `aws` CLI (v2 recommended). Optionally: `cdk`, `sam`, `terraform` for richer IaC docs.

| Test | Command | Expected |
|------|---------|----------|
| Help lists command | `./sbh --help` | Output includes `gen-aws-docs` |
| Dispatch works | `./sbh gen-aws-docs` | Routes to `bin/gen-aws-docs.sh`, does not show sbh usage |
| No aws, non-interactive | `echo "" \| ./sbh gen-aws-docs` | Exits 1, prints "AWS CLI not found" and install URL |
| No aws, interactive | `./sbh gen-aws-docs` (in terminal) | Prompts "Install AWS CLI v2?" with options [1/2/n] |
| Install option 1 (local) | Choose `1` at install prompt | Installs to `~/.local/aws-cli`, prints version, continues to doc generation |
| Install option n (skip) | Choose `n` at install prompt | Prints "Skipping install", exits 1 with manual install URL |
| Full generation | `./sbh gen-aws-docs` (with aws installed) | Prints progress for 13 services, writes 3 files, prints byte counts and summary |
| Output dir default | `./sbh gen-aws-docs` | Files written to `~/.config/swebash/docs/aws/` |
| Output dir override | `SWEBASH_AWS_DOCS_DIR=/tmp/aws-test ./sbh gen-aws-docs` | Files written to `/tmp/aws-test/` |
| services_reference.md | `cat ~/.config/swebash/docs/aws/services_reference.md` | Contains EC2, S3, LAMBDA, IAM, CLOUDFORMATION sections with synopsis and subcommand lists |
| iac_patterns.md | `cat ~/.config/swebash/docs/aws/iac_patterns.md` | Contains CloudFormation (CFN), CDK, SAM, Terraform sections with deploy recipes |
| troubleshooting.md | `cat ~/.config/swebash/docs/aws/troubleshooting.md` | Contains Authentication section (STS commands, error table) and Debugging section (--debug, env vars, CloudWatch Logs) |
| Version provenance | `head -2 ~/.config/swebash/docs/aws/services_reference.md` | HTML comment contains `aws-cli/2.x.x` version string |
| Budget guard | `wc -c ~/.config/swebash/docs/aws/*.md` | Total under 48k chars; each file under 16k chars |
| Re-run is safe | Run `./sbh gen-aws-docs` twice | Second run overwrites files cleanly, no errors |
| Live CDK help | Install `cdk`, run `./sbh gen-aws-docs` | iac_patterns.md CDK section contains live `cdk --help` output and version comment |
| Live Terraform help | Install `terraform`, run `./sbh gen-aws-docs` | iac_patterns.md Terraform section contains live `terraform --help` output and version comment |

## 25. sbh.ps1 help (PowerShell)

| Test | Command | Expected |
|------|---------|----------|
| Help flag | `.\sbh.ps1 --help` | Prints usage with all commands: setup, build, run, test |
| Help short flag | `.\sbh.ps1 -h` | Same output as `--help` |
| Help command | `.\sbh.ps1 help` | Same output as `--help` |
| No args | `.\sbh.ps1` | Prints usage, exits with code 0 |
| Unknown command | `.\sbh.ps1 foo` | Prints usage, exits with code 1 |

## 26. sbh.ps1 test (PowerShell)

| Test | Command | Expected |
|------|---------|----------|
| All suites | `.\sbh.ps1 test` | Runs engine, readline, host, ai tests in order; all pass |
| Engine only | `.\sbh.ps1 test engine` | Runs engine tests only |
| Host only | `.\sbh.ps1 test host` | Runs host tests only |
| Readline only | `.\sbh.ps1 test readline` | Runs readline tests only |
| AI only | `.\sbh.ps1 test ai` | Runs AI tests only |
| Scripts only | `.\sbh.ps1 test scripts` | Runs Pester script tests (feature + e2e) |

## 27. sbh.ps1 setup (PowerShell)

| Test | Command | Expected |
|------|---------|----------|
| Setup dispatch | `.\sbh.ps1 setup` | Dispatches to `bin\setup.ps1`; checks prerequisites, registry, .env |
| No parse errors | `.\sbh.ps1 setup` | No `ParserError` or `MissingEndCurlyBrace` errors |

## 28. sbh.ps1 build & run (PowerShell)

| Test | Command | Expected |
|------|---------|----------|
| Release build | `.\sbh.ps1 build` | Builds engine WASM (release) and host (release) without errors |
| Debug build | `.\sbh.ps1 build -Debug` | Builds engine WASM (release) and host (debug) without errors |
| Run | `.\sbh.ps1 run` | Launches shell, shows banner and prompt |

---

## See Also

- [Manual Testing Hub](manual_testing.md) — prerequisites and setup
- [Manual Shell Tests](manual_shell_tests.md) — shell feature tests
- [Installation](../7-operations/installation.md) — system requirements
- [@awscli Agent](../7-operations/awscli_agent.md) — AWS agent setup and usage
