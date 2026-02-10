use std::path::{Path, PathBuf};

use super::state::{AccessMode, SandboxPolicy};

/// The kind of access being requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
}

/// Strip Windows UNC prefix (\\?\) for consistent path comparison.
/// On non-Windows or non-UNC paths, returns the path unchanged.
fn normalize_for_comparison(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path.to_path_buf()
}

/// Resolve a potentially-relative path against the process-global current
/// working directory and canonicalize it. Retained for backward compatibility;
/// prefer `resolve_path_with_cwd` for virtualized per-tab resolution.
#[allow(dead_code)]
pub fn resolve_path(raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        // Try to canonicalize; if path doesn't exist yet, keep as-is.
        p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
    } else {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let joined = base.join(p);
        joined.canonicalize().unwrap_or(joined)
    }
}

/// Resolve a potentially-relative path against an explicit virtual CWD
/// (instead of the process-global `current_dir`). Used by virtualized
/// host imports so each tab can have its own working directory.
pub fn resolve_path_with_cwd(raw: &str, cwd: &Path) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
    } else {
        let joined = cwd.join(p);
        joined.canonicalize().unwrap_or(joined)
    }
}

/// Check whether `policy` allows the given `kind` of access to `resolved`.
/// Returns `Ok(())` if access is granted, or `Err(message)` with a
/// user-facing denial string.
pub fn check_access(
    policy: &SandboxPolicy,
    resolved: &Path,
    kind: AccessKind,
) -> Result<(), String> {
    if !policy.enabled {
        return Ok(());
    }

    // Normalize paths for comparison (handles Windows UNC prefix differences)
    let normalized_resolved = normalize_for_comparison(resolved);

    // Walk the allowed paths in order — first match wins.
    for rule in &policy.allowed_paths {
        let normalized_root = normalize_for_comparison(&rule.root);
        if normalized_resolved.starts_with(&normalized_root) {
            return match (kind, rule.mode) {
                (AccessKind::Read, _) => Ok(()),
                (AccessKind::Write, AccessMode::ReadWrite) => Ok(()),
                (AccessKind::Write, AccessMode::ReadOnly) => Err(format!(
                    "sandbox: write access denied for '{}': read-only workspace",
                    resolved.display()
                )),
            };
        }
    }

    // No rule matched — path is outside all allowed regions.
    let label = match kind {
        AccessKind::Read => "read",
        AccessKind::Write => "write",
    };
    Err(format!(
        "sandbox: {} access denied for '{}': outside workspace",
        label,
        resolved.display()
    ))
}

/// Convenience: resolve via process-global CWD, then check access.
/// Retained for backward compatibility; prefer `check_path_with_cwd`.
#[allow(dead_code)]
pub fn check_path(policy: &SandboxPolicy, raw: &str, kind: AccessKind) -> Result<(), ()> {
    let resolved = resolve_path(raw);
    match check_access(policy, &resolved, kind) {
        Ok(()) => Ok(()),
        Err(msg) => {
            eprintln!("{msg}");
            Err(())
        }
    }
}

/// Like `check_path` but resolves relative paths against an explicit virtual CWD.
pub fn check_path_with_cwd(
    policy: &SandboxPolicy,
    raw: &str,
    kind: AccessKind,
    cwd: &Path,
) -> Result<(), ()> {
    let resolved = resolve_path_with_cwd(raw, cwd);
    match check_access(policy, &resolved, kind) {
        Ok(()) => Ok(()),
        Err(msg) => {
            eprintln!("{msg}");
            Err(())
        }
    }
}
