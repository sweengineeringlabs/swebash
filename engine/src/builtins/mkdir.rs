extern crate alloc;
use alloc::string::String;

pub fn run(args: &[String]) {
    let mut recursive = false;
    let mut paths: alloc::vec::Vec<&str> = alloc::vec::Vec::new();

    for arg in args {
        if arg == "-p" {
            recursive = true;
        } else if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                if ch == 'p' {
                    recursive = true;
                }
            }
        } else {
            paths.push(arg.as_str());
        }
    }

    if paths.is_empty() {
        crate::write_err(b"mkdir: missing operand\n");
        return;
    }

    for path in paths {
        let result = unsafe {
            crate::host::host_mkdir(path.as_ptr(), path.len(), if recursive { 1 } else { 0 })
        };
        if result < 0 {
            crate::write_err(b"mkdir: cannot create directory '");
            crate::write_err(path.as_bytes());
            crate::write_err(b"'\n");
        }
    }
}
