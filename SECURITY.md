# Security Policy

**Audience**: Internal contributors, security team

**WHAT**: How to report security vulnerabilities in swebash
**WHY**: Structured reporting ensures vulnerabilities are triaged and resolved promptly
**HOW**: GitHub issues with the `security` label, response timeline, and scope definition

---

## Reporting a Vulnerability

This is an internal project. To report a security issue:

1. Open a GitHub issue with the **`security`** label.
2. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)
3. Do **not** include working exploit code in public issues — share details privately with the team if the issue is sensitive.

## Response Timeline

| Step | Target |
|------|--------|
| Acknowledge report | 2 business days |
| Triage and assess severity | 5 business days |
| Fix shipped (critical/high) | 10 business days |
| Fix shipped (medium/low) | Next planned release |

## Scope

The following areas are in scope for security review:

| Area | Concern |
|------|---------|
| **API keys** | Keys must never be logged, committed, or leaked in error output |
| **WASM sandbox** | Engine must not escape the WASM sandbox or access host resources directly |
| **Workspace sandbox** | Filesystem access must be restricted to configured paths and access modes |
| **Command injection** | Shell commands must be properly sanitized before execution |
| **Agent tool calls** | Tool execution (fs, exec, web) must respect configured permissions |
| **Dependencies** | Third-party crates should be audited for known vulnerabilities |

## Security Practices

- API keys are loaded from `.env` files (never committed — `.gitignore` enforced).
- The WASM engine runs in `no_std` with no direct host access.
- Host imports are explicitly defined and narrowly scoped.
- LLM responses are never executed as shell commands without user confirmation.
- **Workspace sandbox** enforces path-based access control at the host import layer:
  - All filesystem operations are checked against a `SandboxPolicy` before reaching the OS.
  - Default workspace (`~/workspace/`) starts in read-only mode.
  - Paths are canonicalized before matching to prevent traversal attacks (`../`).
  - External process spawning (`host_spawn`) verifies the CWD is within the sandbox.
  - The WASM engine cannot bypass sandbox checks — it has no direct OS access.
