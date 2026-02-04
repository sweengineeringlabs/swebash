extern crate alloc;
use alloc::string::String;

pub fn run(args: &[String]) {
    if args.is_empty() {
        crate::write_err(b"touch: missing file operand\n");
        return;
    }

    for path in args {
        // Write zero bytes to create/update the file
        let result = unsafe {
            crate::spi::host::host_write_file(
                path.as_ptr(),
                path.len(),
                core::ptr::null(),
                0,
                0, // not append — but zero bytes so it just creates if missing
            )
        };
        if result < 0 {
            crate::write_err(b"touch: cannot touch '");
            crate::write_err(path.as_bytes());
            crate::write_err(b"'\n");
        }
    }
}
