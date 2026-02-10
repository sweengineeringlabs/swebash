use anyhow::Result;
use std::process::Command;
use wasmtime::*;

use crate::spi::sandbox::{check_path_with_cwd, AccessKind};
use crate::spi::state::HostState;

pub fn register(linker: &mut Linker<HostState>) -> Result<()> {
    // host_spawn(data_ptr, data_len) -> i32
    // data is null-separated: cmd\0arg1\0arg2\0...
    linker.func_wrap(
        "env",
        "host_spawn",
        |mut caller: Caller<'_, HostState>, data_ptr: i32, data_len: i32| -> i32 {
            // Read virtual CWD and env from HostState
            let virtual_cwd = caller.data().virtual_cwd.clone();
            let virtual_env = caller.data().virtual_env.clone();
            let removed_env = caller.data().removed_env.clone();

            // Verify current working directory is within the sandbox before
            // spawning an external process.
            let sandbox = &caller.data().sandbox;
            if sandbox.enabled {
                let cwd_str = virtual_cwd.to_string_lossy();
                if check_path_with_cwd(sandbox, &cwd_str, AccessKind::Read, &virtual_cwd).is_err()
                {
                    return -1;
                }
            }

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

            // Build the child command with virtual CWD and env overlays
            let build_cmd = |cmd: &mut Command| {
                cmd.current_dir(&virtual_cwd);
                for (key, val) in &virtual_env {
                    cmd.env(key, val);
                }
                for key in &removed_env {
                    cmd.env_remove(key);
                }
            };

            // On Windows, use cmd /C for better compatibility
            let result = if cfg!(windows) {
                let mut full_cmd = String::from(program);
                for arg in args {
                    full_cmd.push(' ');
                    full_cmd.push_str(arg);
                }
                let mut cmd = Command::new("cmd");
                cmd.args(["/C", &full_cmd]);
                build_cmd(&mut cmd);
                cmd.status()
            } else {
                let mut cmd = Command::new(program);
                cmd.args(args);
                build_cmd(&mut cmd);
                cmd.status()
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
