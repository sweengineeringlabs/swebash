extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

pub fn run(args: &[String]) {
    let mut num_lines: usize = 10;
    let mut file: Option<&str> = None;
    let mut i = 0;

    while i < args.len() {
        if args[i] == "-n" {
            i += 1;
            if i < args.len() {
                if let Some(n) = parse_usize(&args[i]) {
                    num_lines = n;
                }
            }
        } else if args[i].starts_with('-') {
            if let Some(n) = parse_usize(&args[i][1..]) {
                num_lines = n;
            }
        } else {
            file = Some(args[i].as_str());
        }
        i += 1;
    }

    let path = match file {
        Some(p) => p,
        None => {
            crate::write_err(b"tail: missing file operand\n");
            return;
        }
    };

    let len = unsafe {
        crate::spi::host::host_read_file(path.as_ptr(), path.len())
    };
    if len < 0 {
        crate::write_err(b"tail: ");
        crate::write_err(path.as_bytes());
        crate::write_err(b": no such file\n");
        return;
    }

    let data = crate::api::buffer::read_response(len as usize);
    let text = match core::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => {
            crate::write_bytes(data);
            return;
        }
    };

    let lines: Vec<&str> = text.split('\n').collect();
    let total = lines.len();
    // If file ends with \n, last element is empty — adjust
    let effective_total = if total > 0 && lines[total - 1].is_empty() {
        total - 1
    } else {
        total
    };

    let start = if effective_total > num_lines {
        effective_total - num_lines
    } else {
        0
    };

    for idx in start..effective_total {
        crate::write_bytes(lines[idx].as_bytes());
        crate::write_bytes(b"\n");
    }
}

fn parse_usize(s: &str) -> Option<usize> {
    let mut result: usize = 0;
    for ch in s.chars() {
        let d = ch.to_digit(10)? as usize;
        result = result.checked_mul(10)?.checked_add(d)?;
    }
    if s.is_empty() {
        None
    } else {
        Some(result)
    }
}
