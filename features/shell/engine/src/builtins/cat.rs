extern crate alloc;
use alloc::string::String;

pub fn run(args: &[String]) {
    if args.is_empty() {
        crate::write_err(b"cat: missing file operand\n");
        return;
    }

    for path in args {
        let len = unsafe {
            crate::spi::host::host_read_file(path.as_ptr(), path.len())
        };
        if len < 0 {
            crate::write_err(b"cat: ");
            crate::write_err(path.as_bytes());
            crate::write_err(b": no such file\n");
            continue;
        }
        let data = crate::api::buffer::read_response(len as usize);
        crate::write_bytes(data);
    }
}
