// ---------------------------------------------------------------------------
// Input + response buffer management
// ---------------------------------------------------------------------------

use core::ptr::addr_of;
use core::ptr::addr_of_mut;

const INPUT_BUF_CAP: usize = 4096;
const RESPONSE_BUF_CAP: usize = 65536;

static mut INPUT_BUF: [u8; INPUT_BUF_CAP] = [0u8; INPUT_BUF_CAP];
static mut RESPONSE_BUF: [u8; RESPONSE_BUF_CAP] = [0u8; RESPONSE_BUF_CAP];

// ---------------------------------------------------------------------------
// Input buffer — the host writes command bytes here before calling shell_eval
// ---------------------------------------------------------------------------

/// Return a pointer to the input buffer so the host can write into it.
#[no_mangle]
pub extern "C" fn get_input_buf() -> *mut u8 {
    addr_of_mut!(INPUT_BUF) as *mut u8
}

/// Return the capacity of the input buffer.
#[no_mangle]
pub extern "C" fn get_input_buf_len() -> usize {
    INPUT_BUF_CAP
}

/// Read `len` bytes from the input buffer as a byte slice.
pub fn read_input(len: usize) -> &'static [u8] {
    let n = if len > INPUT_BUF_CAP { INPUT_BUF_CAP } else { len };
    unsafe { core::slice::from_raw_parts(addr_of!(INPUT_BUF) as *const u8, n) }
}

// ---------------------------------------------------------------------------
// Response buffer — the host writes results here, engine reads them back
// ---------------------------------------------------------------------------

/// Return a pointer to the response buffer so the host can write into it.
#[no_mangle]
pub extern "C" fn get_response_buf() -> *mut u8 {
    addr_of_mut!(RESPONSE_BUF) as *mut u8
}

/// Return the capacity of the response buffer.
#[no_mangle]
pub extern "C" fn get_response_buf_len() -> usize {
    RESPONSE_BUF_CAP
}

/// Read `len` bytes from the response buffer as a byte slice.
pub fn read_response(len: usize) -> &'static [u8] {
    let n = if len > RESPONSE_BUF_CAP {
        RESPONSE_BUF_CAP
    } else {
        len
    };
    unsafe { core::slice::from_raw_parts(addr_of!(RESPONSE_BUF) as *const u8, n) }
}
