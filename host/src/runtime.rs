use anyhow::{Context, Result};
use wasmtime::*;

use crate::state::HostState;

/// Resolve the path to the compiled engine.wasm file.
pub fn engine_wasm_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("ENGINE_WASM") {
        return std::path::PathBuf::from(p);
    }
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("target");
    path.push("wasm32-unknown-unknown");
    path.push("release");
    path.push("engine.wasm");
    path
}

/// Create a Wasmtime engine, module, store, and linker.
/// Returns (store, instance) with all imports registered.
pub fn setup() -> Result<(Store<HostState>, Instance)> {
    let engine = Engine::default();

    let wasm_path = engine_wasm_path();
    let module = Module::from_file(&engine, &wasm_path)
        .with_context(|| format!("failed to load wasm module at {}", wasm_path.display()))?;

    let state = HostState {
        response_buf_ptr: 0,
        response_buf_cap: 0,
    };
    let mut store = Store::new(&engine, state);
    let mut linker = Linker::new(&engine);

    // Register all host imports
    crate::imports::register_all(&mut linker)?;

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
