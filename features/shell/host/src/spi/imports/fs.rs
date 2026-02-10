use anyhow::Result;
use std::fs;
use wasmtime::*;

use crate::spi::sandbox::{self, check_path_with_cwd, AccessKind};
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

/// Helper: read bytes from wasm memory at (ptr, len).
fn read_bytes(memory: &Memory, store: &impl AsContext, ptr: i32, len: i32) -> Option<Vec<u8>> {
    let data = memory.data(store.as_context());
    let start = ptr as usize;
    let end = start + len as usize;
    if end > data.len() {
        return None;
    }
    Some(data[start..end].to_vec())
}

/// Helper: write data into the response buffer in wasm memory.
/// Returns bytes written, or -1 on error.
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

/// Convert Unix epoch seconds to "YYYY-MM-DD HH:MM" (UTC) using civil date
/// algorithm (no floats, no std chrono dependency).
fn format_timestamp(secs: u64) -> String {
    let s = secs % 86400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;

    // Days since 1970-01-01
    let mut days = (secs / 86400) as i64;

    // Civil date from day count (algorithm from Howard Hinnant)
    days += 719468; // shift epoch from 1970-01-01 to 0000-03-01
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        y, m, d, hh, mm
    )
}

pub fn register(linker: &mut Linker<HostState>) -> Result<()> {
    // host_read_file(path_ptr, path_len) -> i32
    linker.func_wrap(
        "env",
        "host_read_file",
        |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let path = match read_str(&memory, &caller, path_ptr, path_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            if check_path_with_cwd(&caller.data().sandbox, &path, AccessKind::Read, &cwd).is_err()
            {
                return -1;
            }

            let resolved = sandbox::resolve_path_with_cwd(&path, &cwd);
            match fs::read(&resolved) {
                Ok(contents) => write_response(&mut caller, &contents),
                Err(_) => -1,
            }
        },
    )?;

    // host_list_dir(path_ptr, path_len) -> i32
    linker.func_wrap(
        "env",
        "host_list_dir",
        |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let path = match read_str(&memory, &caller, path_ptr, path_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            if check_path_with_cwd(&caller.data().sandbox, &path, AccessKind::Read, &cwd).is_err()
            {
                return -1;
            }

            let resolved = sandbox::resolve_path_with_cwd(&path, &cwd);
            let entries = match fs::read_dir(&resolved) {
                Ok(rd) => rd,
                Err(_) => return -1,
            };

            let mut result = String::new();
            for entry in entries {
                if let Ok(e) = entry {
                    if let Some(name) = e.file_name().to_str() {
                        result.push_str(name);
                        result.push('\n');
                    }
                }
            }

            write_response(&mut caller, result.as_bytes())
        },
    )?;

    // host_stat(path_ptr, path_len) -> i32
    linker.func_wrap(
        "env",
        "host_stat",
        |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let path = match read_str(&memory, &caller, path_ptr, path_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            if check_path_with_cwd(&caller.data().sandbox, &path, AccessKind::Read, &cwd).is_err()
            {
                return -1;
            }

            let resolved = sandbox::resolve_path_with_cwd(&path, &cwd);
            let meta = match fs::metadata(&resolved) {
                Ok(m) => m,
                Err(_) => return -1,
            };

            let file_type = if meta.is_dir() {
                "dir"
            } else if meta.is_file() {
                "file"
            } else {
                "other"
            };

            let size = meta.len();

            let modified = meta
                .modified()
                .ok()
                .and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs())
                })
                .unwrap_or(0);

            let ts = format_timestamp(modified);
            let result = format!("{} {} {}", file_type, size, ts);
            write_response(&mut caller, result.as_bytes())
        },
    )?;

    // host_write_file(path_ptr, path_len, data_ptr, data_len, append) -> i32
    linker.func_wrap(
        "env",
        "host_write_file",
        |mut caller: Caller<'_, HostState>,
         path_ptr: i32,
         path_len: i32,
         data_ptr: i32,
         data_len: i32,
         append: i32|
         -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let path = match read_str(&memory, &caller, path_ptr, path_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            if check_path_with_cwd(&caller.data().sandbox, &path, AccessKind::Write, &cwd).is_err()
            {
                return -1;
            }

            let resolved = sandbox::resolve_path_with_cwd(&path, &cwd);
            let content = if data_len > 0 {
                match read_bytes(&memory, &caller, data_ptr, data_len) {
                    Some(b) => b,
                    None => return -1,
                }
            } else {
                Vec::new()
            };

            let result = if append != 0 {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&resolved)
                    .and_then(|mut f| f.write_all(&content))
            } else {
                // For touch: if file exists and content is empty, just update mtime
                if content.is_empty() && resolved.exists() {
                    // File already exists, nothing to write
                    return 0;
                }
                // Create if not exists, or truncate if content provided
                if content.is_empty() {
                    // touch: create empty file
                    std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .open(&resolved)
                        .map(|_| ())
                } else {
                    fs::write(&resolved, &content)
                }
            };

            match result {
                Ok(_) => 0,
                Err(_) => -1,
            }
        },
    )?;

    // host_remove(path_ptr, path_len, recursive) -> i32
    linker.func_wrap(
        "env",
        "host_remove",
        |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32, recursive: i32| -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let path = match read_str(&memory, &caller, path_ptr, path_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            if check_path_with_cwd(&caller.data().sandbox, &path, AccessKind::Write, &cwd).is_err()
            {
                return -1;
            }

            let p = sandbox::resolve_path_with_cwd(&path, &cwd);
            let result = if p.is_dir() {
                if recursive != 0 {
                    fs::remove_dir_all(p)
                } else {
                    fs::remove_dir(p)
                }
            } else {
                fs::remove_file(p)
            };

            match result {
                Ok(_) => 0,
                Err(_) => -1,
            }
        },
    )?;

    // host_copy(src_ptr, src_len, dst_ptr, dst_len) -> i32
    linker.func_wrap(
        "env",
        "host_copy",
        |mut caller: Caller<'_, HostState>,
         src_ptr: i32,
         src_len: i32,
         dst_ptr: i32,
         dst_len: i32|
         -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let src = match read_str(&memory, &caller, src_ptr, src_len) {
                Some(p) => p,
                None => return -1,
            };
            let dst = match read_str(&memory, &caller, dst_ptr, dst_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            let sandbox = &caller.data().sandbox;
            if check_path_with_cwd(sandbox, &src, AccessKind::Read, &cwd).is_err() {
                return -1;
            }
            if check_path_with_cwd(sandbox, &dst, AccessKind::Write, &cwd).is_err() {
                return -1;
            }

            let resolved_src = sandbox::resolve_path_with_cwd(&src, &cwd);
            let resolved_dst = sandbox::resolve_path_with_cwd(&dst, &cwd);
            match fs::copy(&resolved_src, &resolved_dst) {
                Ok(_) => 0,
                Err(_) => -1,
            }
        },
    )?;

    // host_rename(src_ptr, src_len, dst_ptr, dst_len) -> i32
    linker.func_wrap(
        "env",
        "host_rename",
        |mut caller: Caller<'_, HostState>,
         src_ptr: i32,
         src_len: i32,
         dst_ptr: i32,
         dst_len: i32|
         -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let src = match read_str(&memory, &caller, src_ptr, src_len) {
                Some(p) => p,
                None => return -1,
            };
            let dst = match read_str(&memory, &caller, dst_ptr, dst_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            let sandbox = &caller.data().sandbox;
            if check_path_with_cwd(sandbox, &src, AccessKind::Write, &cwd).is_err() {
                return -1;
            }
            if check_path_with_cwd(sandbox, &dst, AccessKind::Write, &cwd).is_err() {
                return -1;
            }

            let resolved_src = sandbox::resolve_path_with_cwd(&src, &cwd);
            let resolved_dst = sandbox::resolve_path_with_cwd(&dst, &cwd);
            match fs::rename(&resolved_src, &resolved_dst) {
                Ok(_) => 0,
                Err(_) => -1,
            }
        },
    )?;

    // host_mkdir(path_ptr, path_len, recursive) -> i32
    linker.func_wrap(
        "env",
        "host_mkdir",
        |mut caller: Caller<'_, HostState>,
         path_ptr: i32,
         path_len: i32,
         recursive: i32|
         -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let path = match read_str(&memory, &caller, path_ptr, path_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            if check_path_with_cwd(&caller.data().sandbox, &path, AccessKind::Write, &cwd).is_err()
            {
                return -1;
            }

            let resolved = sandbox::resolve_path_with_cwd(&path, &cwd);
            let result = if recursive != 0 {
                fs::create_dir_all(&resolved)
            } else {
                fs::create_dir(&resolved)
            };

            match result {
                Ok(_) => 0,
                Err(_) => -1,
            }
        },
    )?;

    // host_get_cwd() -> i32
    linker.func_wrap(
        "env",
        "host_get_cwd",
        |mut caller: Caller<'_, HostState>| -> i32 {
            let s = caller.data().virtual_cwd.to_string_lossy().into_owned();
            write_response(&mut caller, s.as_bytes())
        },
    )?;

    // host_set_cwd(path_ptr, path_len) -> i32
    linker.func_wrap(
        "env",
        "host_set_cwd",
        |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
            let memory = caller
                .get_export("memory")
                .and_then(|e| e.into_memory())
                .expect("wasm module must export memory");
            let path = match read_str(&memory, &caller, path_ptr, path_len) {
                Some(p) => p,
                None => return -1,
            };

            let cwd = caller.data().virtual_cwd.clone();
            if check_path_with_cwd(&caller.data().sandbox, &path, AccessKind::Read, &cwd).is_err()
            {
                return -1;
            }

            // Resolve relative paths against current virtual CWD
            let resolved = sandbox::resolve_path_with_cwd(&path, &cwd);
            if !resolved.is_dir() {
                return -1;
            }
            caller.data_mut().virtual_cwd = resolved;
            0
        },
    )?;

    Ok(())
}
