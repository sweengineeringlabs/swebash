use anyhow::Result;
use std::process::Command;
use wasmtime::*;

use crate::state::HostState;

pub fn register(linker: &mut Linker<HostState>) -> Result<()> {
    // host_spawn(data_ptr, data_len) -> i32
    // data is null-separated: cmd\0arg1\0arg2\0...
    linker.func_wrap(
        "env",
        "host_spawn",
        |mut caller: Caller<'_, HostState>, data_ptr: i32, data_len: i32| -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");

            let data = memory.data(&caller);
            let start = data_ptr as usize;
            let end = start + data_len as usize;

            if end > data.len() {
                return -1;
            }

            let bytes = &data[start..end];

            // Split on null bytes
            let parts: Vec<&str> = bytes
                .split(|&b| b == 0)
                .filter_map(|s| std::str::from_utf8(s).ok())
                .collect();

            if parts.is_empty() {
                return -1;
            }

            let program = parts[0];
            let args = &parts[1..];

            // On Windows, use cmd /C for better compatibility
            let result = if cfg!(windows) {
                let mut full_cmd = String::from(program);
                for arg in args {
                    full_cmd.push(' ');
                    full_cmd.push_str(arg);
                }
                Command::new("cmd")
                    .args(["/C", &full_cmd])
                    .status()
            } else {
                Command::new(program).args(args).status()
            };

            match result {
                Ok(status) => status.code().unwrap_or(-1),
                Err(e) => {
                    let msg = format!("{}: {}\n", program, e);
                    let stderr = std::io::stderr();
                    let mut handle = stderr.lock();
                    use std::io::Write;
                    let _ = handle.write_all(msg.as_bytes());
                    127
                }
            }
        },
    )?;

    Ok(())
}
