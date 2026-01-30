mod imports;
mod runtime;
mod state;

use anyhow::{Context, Result};
use std::io::{self, Write};

fn main() -> Result<()> {
    let (mut store, instance) = runtime::setup()?;

    // Grab exported functions
    let shell_init = instance
        .get_typed_func::<(), ()>(&mut store, "shell_init")
        .context("missing export: shell_init")?;

    let shell_eval = instance
        .get_typed_func::<u32, ()>(&mut store, "shell_eval")
        .context("missing export: shell_eval")?;

    let get_input_buf = instance
        .get_typed_func::<(), u32>(&mut store, "get_input_buf")
        .context("missing export: get_input_buf")?;

    let get_input_buf_len = instance
        .get_typed_func::<(), u32>(&mut store, "get_input_buf_len")
        .context("missing export: get_input_buf_len")?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .context("missing export: memory")?;

    // Call shell_init
    shell_init.call(&mut store, ())?;

    // REPL loop
    let buf_ptr = get_input_buf.call(&mut store, ())? as usize;
    let buf_cap = get_input_buf_len.call(&mut store, ())? as usize;

    let stdin = io::stdin();
    let mut line = String::new();

    let home_dir = dirs::home_dir();

    loop {
        // Show cwd in prompt, substituting ~ for home directory
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let display_cwd = match &home_dir {
            Some(h) => {
                let home_str = h.to_string_lossy();
                if cwd == home_str.as_ref() {
                    String::from("~")
                } else if cwd.starts_with(home_str.as_ref()) {
                    let rest = &cwd[home_str.len()..];
                    if rest.starts_with('/') || rest.starts_with('\\') {
                        format!("~{}", rest)
                    } else {
                        cwd
                    }
                } else {
                    cwd
                }
            }
            None => cwd,
        };
        print!("\x1b[1;32m{}\x1b[0m/> ", display_cwd);
        io::stdout().flush()?;

        line.clear();
        let n = stdin.read_line(&mut line)?;
        if n == 0 {
            break;
        }

        let cmd = line.trim();
        if cmd.is_empty() {
            continue;
        }
        if cmd == "exit" {
            break;
        }

        let cmd_bytes = cmd.as_bytes();
        if cmd_bytes.len() > buf_cap {
            eprintln!(
                "[host] command too long ({} bytes, max {})",
                cmd_bytes.len(),
                buf_cap
            );
            continue;
        }

        memory.write(&mut store, buf_ptr, cmd_bytes)?;
        shell_eval.call(&mut store, cmd_bytes.len() as u32)?;
    }

    Ok(())
}
