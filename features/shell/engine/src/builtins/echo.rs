extern crate alloc;
use alloc::string::String;

pub fn run(args: &[String]) {
    let mut first = true;
    for arg in args {
        if !first {
            crate::write_bytes(b" ");
        }
        crate::write_bytes(arg.as_bytes());
        first = false;
    }
    crate::write_bytes(b"\n");
}
