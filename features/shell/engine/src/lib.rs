#![no_std]

extern crate alloc;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[cfg(target_arch = "wasm32")]
mod api;
#[cfg(target_arch = "wasm32")]
mod spi;
#[cfg(target_arch = "wasm32")]
mod builtins;
#[cfg(target_arch = "wasm32")]
mod dispatch;

// Parser is pure logic — always compiled so it can be tested on native.
mod parser;

#[cfg(target_arch = "wasm32")]
use crate::spi::host::host_write;

/// Helper: send a byte slice to the host stdout.
#[cfg(target_arch = "wasm32")]
pub fn write_bytes(bytes: &[u8]) {
    unsafe {
        host_write(bytes.as_ptr(), bytes.len());
    }
}

/// Helper: send a byte slice to the host stderr.
#[cfg(target_arch = "wasm32")]
pub fn write_err(bytes: &[u8]) {
    unsafe {
        spi::host::host_write_err(bytes.as_ptr(), bytes.len());
    }
}

// ---------------------------------------------------------------------------
// Exported API
// ---------------------------------------------------------------------------

/// Called once at startup. Prints a welcome banner via the host.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn shell_init() {
    write_bytes(b"wasm-shell v0.1.0\n");
    write_bytes(b"Type a command and press Enter. Type \"exit\" to quit.\n");
}

/// Evaluate a command string.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn shell_eval(len: usize) {
    let input = api::buffer::read_input(len);

    let cmd_str = match core::str::from_utf8(input) {
        Ok(s) => s,
        Err(_) => {
            write_err(b"error: invalid UTF-8 input\n");
            return;
        }
    };

    let trimmed = cmd_str.trim();
    if trimmed.is_empty() {
        return;
    }

    dispatch::dispatch(trimmed);
}

// ---------------------------------------------------------------------------
// Panic handler (required for #![no_std])
// ---------------------------------------------------------------------------
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    write_err(b"[engine] panic!\n");
    core::arch::wasm32::unreachable()
}
