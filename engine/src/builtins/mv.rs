extern crate alloc;
use alloc::string::String;

pub fn run(args: &[String]) {
    if args.len() < 2 {
        crate::write_err(b"mv: missing operand\n");
        return;
    }

    let src = &args[0];
    let dst = &args[1];

    let result = unsafe {
        crate::host::host_rename(
            src.as_ptr(), src.len(),
            dst.as_ptr(), dst.len(),
        )
    };
    if result < 0 {
        crate::write_err(b"mv: failed to rename '");
        crate::write_err(src.as_bytes());
        crate::write_err(b"' to '");
        crate::write_err(dst.as_bytes());
        crate::write_err(b"'\n");
    }
}
