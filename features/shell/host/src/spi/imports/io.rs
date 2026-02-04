use anyhow::Result;
use std::io::Write;
use wasmtime::*;

use crate::spi::state::HostState;

pub fn register(linker: &mut Linker<HostState>) -> Result<()> {
    // host_write(ptr: i32, len: i32)
    linker.func_wrap(
        "env",
        "host_write",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");

            let data = memory.data(&caller);
            let start = ptr as usize;
            let end = start + len as usize;

            if end <= data.len() {
                let bytes = &data[start..end];
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                let _ = handle.write_all(bytes);
                let _ = handle.flush();
            }
        },
    )?;

    // host_write_err(ptr: i32, len: i32)
    linker.func_wrap(
        "env",
        "host_write_err",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");

            let data = memory.data(&caller);
            let start = ptr as usize;
            let end = start + len as usize;

            if end <= data.len() {
                let bytes = &data[start..end];
                let stderr = std::io::stderr();
                let mut handle = stderr.lock();
                let _ = handle.write_all(bytes);
                let _ = handle.flush();
            }
        },
    )?;

    Ok(())
}
