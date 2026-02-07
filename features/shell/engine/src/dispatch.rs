// ---------------------------------------------------------------------------
// Dispatch: builtin lookup → host_spawn fallback
// ---------------------------------------------------------------------------

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::builtins;
use crate::parser;

/// Parse and dispatch a command line.
pub fn dispatch(input: &str) {
    let parsed = match parser::parse(input) {
        Some(p) => p,
        None => return,
    };

    let name = parsed.name.as_str();
    let args = &parsed.args;

    match name {
        "echo" => builtins::echo::run(args),
        "pwd" => builtins::pwd::run(args),
        "cd" => builtins::cd::run(args),
        "ls" => builtins::ls::run(args),
        "cat" => builtins::cat::run(args),
        "mkdir" => builtins::mkdir::run(args),
        "rm" => builtins::rm::run(args),
        "cp" => builtins::cp::run(args),
        "mv" => builtins::mv::run(args),
        "env" => builtins::env_cmd::run_env(args),
        "export" => builtins::env_cmd::run_export(args),
        "head" => builtins::head::run(args),
        "tail" => builtins::tail::run(args),
        "touch" => builtins::touch::run(args),
        "workspace" => builtins::workspace::run(args),
        "exit" => { /* handled by host REPL */ }
        _ => spawn_external(input),
    }
}

/// Fall through to host_spawn for unrecognized commands.
fn spawn_external(input: &str) {
    // Build null-separated payload: cmd\0arg1\0arg2\0...
    let parsed = match parser::parse(input) {
        Some(p) => p,
        None => return,
    };

    let mut payload: Vec<u8> = Vec::new();
    payload.extend_from_slice(parsed.name.as_bytes());
    for arg in &parsed.args {
        payload.push(0);
        payload.extend_from_slice(arg.as_bytes());
    }

    let exit_code = unsafe {
        crate::spi::host::host_spawn(payload.as_ptr(), payload.len())
    };

    if exit_code != 0 {
        let mut msg = String::from("process exited with code ");
        write_i32(&mut msg, exit_code);
        msg.push('\n');
        crate::write_err(msg.as_bytes());
    }
}

fn write_i32(buf: &mut String, mut val: i32) {
    if val < 0 {
        buf.push('-');
        // Handle i32::MIN safely
        if val == i32::MIN {
            buf.push_str("2147483648");
            return;
        }
        val = -val;
    }
    write_u32(buf, val as u32);
}

fn write_u32(buf: &mut String, val: u32) {
    if val == 0 {
        buf.push('0');
        return;
    }
    let mut digits = [0u8; 10];
    let mut i = 0;
    let mut v = val;
    while v > 0 {
        digits[i] = b'0' + (v % 10) as u8;
        v /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        buf.push(digits[i] as char);
    }
}
