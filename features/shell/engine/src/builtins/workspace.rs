extern crate alloc;
use alloc::string::String;

pub fn run(args: &[String]) {
    // Build a single command string from args to pass to host_workspace
    let cmd: String = if args.is_empty() {
        String::from("status")
    } else {
        let mut buf = String::new();
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                buf.push(' ');
            }
            buf.push_str(arg);
        }
        buf
    };

    let len = unsafe {
        crate::spi::host::host_workspace(cmd.as_ptr(), cmd.len())
    };

    if len < 0 {
        crate::write_err(b"workspace: command failed\n");
        return;
    }

    let data = crate::api::buffer::read_response(len as usize);
    crate::write_bytes(data);
}
