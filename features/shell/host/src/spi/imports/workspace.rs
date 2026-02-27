use anyhow::Result;
use wasmtime::*;

use crate::spi::path::display_path;
use crate::spi::state::{AccessMode, HostState, PathRule};

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

/// Expand `~` or `~/...` to the user's home directory.
fn expand_tilde(raw: &str) -> std::path::PathBuf {
    if raw == "~" {
        dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(raw))
    } else if let Some(rest) = raw.strip_prefix("~/") {
        dirs::home_dir()
            .map(|h| h.join(rest))
            .unwrap_or_else(|| std::path::PathBuf::from(raw))
    } else {
        std::path::PathBuf::from(raw)
    }
}

fn format_mode(mode: AccessMode) -> &'static str {
    match mode {
        AccessMode::ReadOnly => "ro",
        AccessMode::ReadWrite => "rw",
    }
}

pub fn register(linker: &mut Linker<HostState>) -> Result<()> {
    // host_workspace(cmd_ptr, cmd_len) -> i32
    // Text protocol. Returns response length in the response buffer, or -1 on error.
    linker.func_wrap(
        "env",
        "host_workspace",
        |mut caller: Caller<'_, HostState>, cmd_ptr: i32, cmd_len: i32| -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let cmd = match read_str(&memory, &caller, cmd_ptr, cmd_len) {
                Some(c) => c,
                None => return -1,
            };

            let parts: Vec<&str> = cmd.split_whitespace().collect();
            let subcmd = parts.first().copied().unwrap_or("");

            match subcmd {
                "" | "status" => {
                    // Show current sandbox status
                    let sandbox = &caller.data().sandbox;
                    let mut out = String::new();
                    out.push_str(&format!(
                        "Workspace sandbox: {}\n",
                        if sandbox.enabled { "enabled" } else { "disabled" }
                    ));
                    out.push_str(&format!(
                        "Root: {}\n",
                        display_path(&sandbox.workspace_root)
                    ));
                    out.push_str("Allowed paths:\n");
                    for (i, rule) in sandbox.allowed_paths.iter().enumerate() {
                        let label = if i == 0 { " (workspace)" } else { "" };
                        out.push_str(&format!(
                            "  {} [{}]{}\n",
                            display_path(&rule.root),
                            format_mode(rule.mode),
                            label,
                        ));
                    }
                    write_response(&mut caller, out.as_bytes())
                }
                "rw" => {
                    if let Some(rule) = caller.data_mut().sandbox.allowed_paths.first_mut() {
                        rule.mode = AccessMode::ReadWrite;
                    }
                    let msg = "Workspace set to read-write.\n";
                    write_response(&mut caller, msg.as_bytes())
                }
                "ro" => {
                    if let Some(rule) = caller.data_mut().sandbox.allowed_paths.first_mut() {
                        rule.mode = AccessMode::ReadOnly;
                    }
                    let msg = "Workspace set to read-only.\n";
                    write_response(&mut caller, msg.as_bytes())
                }
                "allow" => {
                    // workspace allow PATH [ro|rw]
                    if parts.len() < 2 {
                        let msg = "usage: workspace allow PATH [ro|rw]\n";
                        return write_response(&mut caller, msg.as_bytes());
                    }
                    let raw_path = parts[1];
                    let mode_str = parts.get(2).copied().unwrap_or("rw");
                    let mode = match mode_str {
                        "ro" | "readonly" => AccessMode::ReadOnly,
                        _ => AccessMode::ReadWrite,
                    };
                    let expanded = expand_tilde(raw_path);
                    let canonical = expanded.canonicalize().unwrap_or(expanded);
                    let path_display = display_path(&canonical);
                    caller.data_mut().sandbox.allowed_paths.push(PathRule {
                        root: canonical,
                        mode,
                    });
                    let msg = format!("Allowed path added: {} [{}]\n", path_display, format_mode(mode));
                    write_response(&mut caller, msg.as_bytes())
                }
                "disable" => {
                    caller.data_mut().sandbox.enabled = false;
                    let msg = "Sandbox disabled.\n";
                    write_response(&mut caller, msg.as_bytes())
                }
                "enable" => {
                    caller.data_mut().sandbox.enabled = true;
                    let msg = "Sandbox enabled.\n";
                    write_response(&mut caller, msg.as_bytes())
                }
                _ => {
                    let msg = format!(
                        "workspace: unknown subcommand '{}'\n\
                         Usage:\n  \
                           workspace              show status\n  \
                           workspace rw            set workspace to read-write\n  \
                           workspace ro            set workspace to read-only\n  \
                           workspace allow PATH [ro|rw]  add allowed path\n  \
                           workspace disable       turn off sandbox\n  \
                           workspace enable        turn on sandbox\n",
                        subcmd,
                    );
                    write_response(&mut caller, msg.as_bytes())
                }
            }
        },
    )?;

    Ok(())
}
