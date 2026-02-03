extern crate alloc;
use alloc::string::String;

pub fn run(args: &[String]) {
    let mut recursive = false;
    let mut force = false;
    let mut paths: alloc::vec::Vec<&str> = alloc::vec::Vec::new();

    for arg in args {
        if arg.starts_with('-') && arg.len() > 1 {
            for ch in arg[1..].chars() {
                match ch {
                    'r' | 'R' => recursive = true,
                    'f' => force = true,
                    _ => {}
                }
            }
        } else {
            paths.push(arg.as_str());
        }
    }

    if paths.is_empty() {
        if !force {
            crate::write_err(b"rm: missing operand\n");
        }
        return;
    }

    for path in paths {
        let result = unsafe {
            crate::host::host_remove(path.as_ptr(), path.len(), if recursive { 1 } else { 0 })
        };
        if result < 0 && !force {
            crate::write_err(b"rm: cannot remove '");
            crate::write_err(path.as_bytes());
            crate::write_err(b"'\n");
        }
    }
}
