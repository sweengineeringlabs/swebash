use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use wasmtime::*;

use super::git_gates::GitGateEnforcer;
use super::state::{HostState, SandboxPolicy};

/// Engine WASM bytes embedded at compile time by build.rs.
const EMBEDDED_ENGINE_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/engine.wasm"));

/// Create a Wasmtime engine, module, store, and linker.
/// Returns (store, instance) with all imports registered.
///
/// If the `ENGINE_WASM` environment variable is set at runtime, the module is
/// loaded from that file path (useful during development).  Otherwise the
/// compile-time embedded bytes are used.
pub fn setup(
    sandbox: SandboxPolicy,
    initial_cwd: PathBuf,
    git_enforcer: Option<Arc<GitGateEnforcer>>,
) -> Result<(Store<HostState>, Instance)> {
    let engine = Engine::default();

    let module = if let Ok(path) = std::env::var("ENGINE_WASM") {
        Module::from_file(&engine, &path)
            .with_context(|| format!("failed to load wasm module at {path}"))?
    } else {
        Module::new(&engine, EMBEDDED_ENGINE_WASM).context("failed to load embedded wasm module")?
    };

    let state = HostState {
        response_buf_ptr: 0,
        response_buf_cap: 0,
        sandbox,
        virtual_cwd: initial_cwd,
        virtual_env: HashMap::new(),
        removed_env: HashSet::new(),
        git_enforcer,
    };
    let mut store = Store::new(&engine, state);
    let mut linker = Linker::new(&engine);

    // Register all host imports
    super::imports::register_all(&mut linker)?;

    let instance = linker
        .instantiate(&mut store, &module)
        .context("failed to instantiate wasm module")?;

    // Grab the response buffer pointer and capacity from the engine
    let get_response_buf = instance
        .get_typed_func::<(), u32>(&mut store, "get_response_buf")
        .context("missing export: get_response_buf")?;
    let get_response_buf_len = instance
        .get_typed_func::<(), u32>(&mut store, "get_response_buf_len")
        .context("missing export: get_response_buf_len")?;

    let buf_ptr = get_response_buf.call(&mut store, ())?;
    let buf_cap = get_response_buf_len.call(&mut store, ())?;

    store.data_mut().response_buf_ptr = buf_ptr;
    store.data_mut().response_buf_cap = buf_cap;

    Ok((store, instance))
}
