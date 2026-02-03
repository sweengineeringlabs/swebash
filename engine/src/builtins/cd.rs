extern crate alloc;
use alloc::string::String;

pub fn run(args: &[String]) {
    let path = if args.is_empty() {
        // cd with no args: go to HOME
        let len = unsafe { crate::host::host_get_env(b"HOME".as_ptr(), 4) };
        if len < 0 {
            // Try USERPROFILE on Windows
            let len2 = unsafe { crate::host::host_get_env(b"USERPROFILE".as_ptr(), 11) };
            if len2 < 0 {
                crate::write_err(b"cd: HOME not set\n");
                return;
            }
            let data = crate::buffer::read_response(len2 as usize);
            match core::str::from_utf8(data) {
                Ok(s) => String::from(s),
                Err(_) => return,
            }
        } else {
            let data = crate::buffer::read_response(len as usize);
            match core::str::from_utf8(data) {
                Ok(s) => String::from(s),
                Err(_) => return,
            }
        }
    } else {
        args[0].clone()
    };

    let result = unsafe {
        crate::host::host_set_cwd(path.as_ptr(), path.len())
    };
    if result < 0 {
        crate::write_err(b"cd: ");
        crate::write_err(path.as_bytes());
        crate::write_err(b": no such directory\n");
    }
}
