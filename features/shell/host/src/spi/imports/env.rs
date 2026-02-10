use anyhow::Result;
use wasmtime::*;

use crate::spi::state::HostState;

/// Helper: read a string from wasm memory at (ptr, len).
fn read_str(memory: &Memory, store: &impl AsContext, ptr: i32, len: i32) -> Option<String> {
    let data = memory.data(store.as_context());
    let start = ptr as usize;
    let end = start + len as usize;
    if end > data.len() {
        return None;
    }
    String::from_utf8(data[start..end].to_vec()).ok()
}

/// Helper: write data into the response buffer in wasm memory.
fn write_response(caller: &mut Caller<'_, HostState>, data: &[u8]) -> i32 {
    let buf_ptr = caller.data().response_buf_ptr as usize;
    let buf_cap = caller.data().response_buf_cap as usize;

    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => return -1,
    };

    let to_write = data.len().min(buf_cap);
    if memory.write(&mut *caller, buf_ptr, &data[..to_write]).is_err() {
        return -1;
    }
    to_write as i32
}

pub fn register(linker: &mut Linker<HostState>) -> Result<()> {
    // host_get_env(key_ptr, key_len) -> i32
    linker.func_wrap(
        "env",
        "host_get_env",
        |mut caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32| -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let key = match read_str(&memory, &caller, key_ptr, key_len) {
                Some(k) => k,
                None => return -1,
            };

            // Check virtual overlay first, then fall back to process env
            let state = caller.data();
            if state.removed_env.contains(&key) {
                return -1;
            }
            if let Some(val) = state.virtual_env.get(&key) {
                let val = val.clone();
                return write_response(&mut caller, val.as_bytes());
            }
            match std::env::var(&key) {
                Ok(val) => write_response(&mut caller, val.as_bytes()),
                Err(_) => -1,
            }
        },
    )?;

    // host_set_env(key_ptr, key_len, val_ptr, val_len)
    linker.func_wrap(
        "env",
        "host_set_env",
        |mut caller: Caller<'_, HostState>,
         key_ptr: i32,
         key_len: i32,
         val_ptr: i32,
         val_len: i32| {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let key = match read_str(&memory, &caller, key_ptr, key_len) {
                Some(k) => k,
                None => return,
            };
            let val = match read_str(&memory, &caller, val_ptr, val_len) {
                Some(v) => v,
                None => return,
            };

            if key == "SWEBASH_WORKSPACE" {
                eprintln!(
                    "warning: setting SWEBASH_WORKSPACE at runtime has no effect on the \
                     sandbox policy. Use the `workspace` command instead."
                );
            }

            // Write to virtual overlay instead of process env
            let state = caller.data_mut();
            state.removed_env.remove(&key);
            state.virtual_env.insert(key, val);
        },
    )?;

    // host_list_env() -> i32
    linker.func_wrap(
        "env",
        "host_list_env",
        |mut caller: Caller<'_, HostState>| -> i32 {
            // Merge: start with process env, overlay virtual env, remove removed keys
            let state = caller.data();
            let mut merged = std::collections::HashMap::new();
            for (key, val) in std::env::vars() {
                if !state.removed_env.contains(&key) {
                    merged.insert(key, val);
                }
            }
            for (key, val) in &state.virtual_env {
                merged.insert(key.clone(), val.clone());
            }

            let mut result = String::new();
            for (key, val) in &merged {
                result.push_str(key);
                result.push('=');
                result.push_str(val);
                result.push('\n');
            }
            write_response(&mut caller, result.as_bytes())
        },
    )?;

    Ok(())
}
