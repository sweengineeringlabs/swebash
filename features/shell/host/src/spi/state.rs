use std::path::PathBuf;

/// Whether a path rule grants read-only or read-write access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

/// A single allowed-path entry in the sandbox policy.
#[derive(Debug, Clone)]
pub struct PathRule {
    /// Canonicalized root of this allowed path.
    pub root: PathBuf,
    /// Access level granted under this root.
    pub mode: AccessMode,
}

/// Controls which filesystem paths the shell may access and how.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// Canonicalized workspace root directory.
    pub workspace_root: PathBuf,
    /// Ordered list of allowed path rules. Index 0 is always the workspace
    /// root itself. First matching rule wins.
    pub allowed_paths: Vec<PathRule>,
    /// When `false`, all sandbox checks are bypassed.
    pub enabled: bool,
}

#[allow(dead_code)]
impl SandboxPolicy {
    /// Build a default policy: workspace at the given root, read-only, enabled.
    pub fn new(workspace_root: PathBuf, mode: AccessMode) -> Self {
        let allowed_paths = vec![PathRule {
            root: workspace_root.clone(),
            mode,
        }];
        Self {
            workspace_root,
            allowed_paths,
            enabled: true,
        }
    }

    /// Build a disabled (pass-through) policy.
    pub fn disabled() -> Self {
        Self {
            workspace_root: PathBuf::new(),
            allowed_paths: Vec::new(),
            enabled: false,
        }
    }
}

/// Shared host state passed through the Wasmtime store.
pub struct HostState {
    /// Pointer offset of the response buffer inside wasm linear memory.
    pub response_buf_ptr: u32,
    /// Capacity of the response buffer.
    pub response_buf_cap: u32,
    /// Sandbox access-control policy.
    pub sandbox: SandboxPolicy,
}
