extern crate alloc;
use alloc::string::String;

pub fn run(_args: &[String]) {
    let len = unsafe { crate::spi::host::host_get_cwd() };
    if len < 0 {
        crate::write_err(b"pwd: failed to get current directory\n");
        return;
    }
    let data = crate::api::buffer::read_response(len as usize);
    crate::write_bytes(data);
    crate::write_bytes(b"\n");
}
