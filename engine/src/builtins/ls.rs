extern crate alloc;
use alloc::string::String;

// ANSI color codes
const RESET: &[u8] = b"\x1b[0m";
const BOLD: &[u8] = b"\x1b[1m";
const DIM: &[u8] = b"\x1b[2m";
const BOLD_BLUE: &[u8] = b"\x1b[1;34m";
const CYAN: &[u8] = b"\x1b[36m";

pub fn run(args: &[String]) {
    let mut long_format = false;
    let mut path: Option<&str> = None;

    for arg in args {
        if arg == "-l" {
            long_format = true;
        } else if arg.starts_with('-') {
            // Check for combined flags like -la
            for ch in arg[1..].chars() {
                if ch == 'l' {
                    long_format = true;
                }
            }
        } else {
            path = Some(arg.as_str());
        }
    }

    if long_format {
        list_long(path);
    } else {
        list_short(path);
    }
}

fn list_short(path: Option<&str>) {
    let len = match path {
        Some(p) => unsafe { crate::host::host_list_dir(p.as_ptr(), p.len()) },
        None => {
            let dot = b".";
            unsafe { crate::host::host_list_dir(dot.as_ptr(), 1) }
        }
    };

    if len < 0 {
        crate::write_err(b"ls: cannot access directory\n");
        return;
    }

    let data = crate::buffer::read_response(len as usize);
    crate::write_bytes(data);
}

/// Format a byte size into a human-readable 7-char right-aligned string.
/// Uses integer-only arithmetic to keep wasm small.
fn format_size(bytes: u64) -> [u8; 7] {
    let mut buf = [b' '; 7];
    if bytes < 1000 {
        // Raw bytes, right-aligned
        let mut n = bytes;
        let mut pos = 6usize;
        if n == 0 {
            buf[pos] = b'0';
        } else {
            while n > 0 {
                buf[pos] = b'0' + (n % 10) as u8;
                n /= 10;
                if pos == 0 { break; }
                pos -= 1;
            }
        }
    } else {
        // Determine unit: K, M, or G
        let (whole, frac, suffix) = if bytes < 1000 * 1024 {
            // kilobytes
            let kb_x10 = bytes * 10 / 1024;
            ((kb_x10 / 10) as u32, (kb_x10 % 10) as u32, b'K')
        } else if bytes < 1000 * 1024 * 1024 {
            // megabytes
            let mb_x10 = bytes * 10 / (1024 * 1024);
            ((mb_x10 / 10) as u32, (mb_x10 % 10) as u32, b'M')
        } else {
            // gigabytes
            let gb_x10 = bytes * 10 / (1024 * 1024 * 1024);
            ((gb_x10 / 10) as u32, (gb_x10 % 10) as u32, b'G')
        };
        // Format as "X.YS" right-aligned in 7 chars
        // e.g. "  42.8K"
        buf[6] = suffix;
        buf[5] = b'0' + (frac as u8);
        buf[4] = b'.';
        let mut n = whole;
        let mut pos = 3usize;
        if n == 0 {
            buf[pos] = b'0';
        } else {
            while n > 0 {
                buf[pos] = b'0' + (n % 10) as u8;
                n /= 10;
                if pos == 0 { break; }
                pos -= 1;
            }
        }
    }
    buf
}

fn list_long(path: Option<&str>) {
    // First get the directory listing
    let len = match path {
        Some(p) => unsafe { crate::host::host_list_dir(p.as_ptr(), p.len()) },
        None => {
            let dot = b".";
            unsafe { crate::host::host_list_dir(dot.as_ptr(), 1) }
        }
    };

    if len < 0 {
        crate::write_err(b"ls: cannot access directory\n");
        return;
    }

    let data = crate::buffer::read_response(len as usize);
    // Copy into owned String because host_stat below overwrites the response buffer
    let listing = match core::str::from_utf8(data) {
        Ok(s) => String::from(s),
        Err(_) => return,
    };

    // Print header
    crate::write_bytes(BOLD);
    crate::write_bytes(b"TYPE     SIZE  DATE              NAME");
    crate::write_bytes(RESET);
    crate::write_bytes(b"\n");

    // Build the base path for stat calls
    let base = match path {
        Some(p) => String::from(p),
        None => String::from("."),
    };

    for entry in listing.lines() {
        if entry.is_empty() {
            continue;
        }
        // Build full path for stat
        let mut full_path = base.clone();
        if !full_path.ends_with('/') && !full_path.ends_with('\\') {
            full_path.push('/');
        }
        full_path.push_str(entry);

        let stat_len = unsafe {
            crate::host::host_stat(full_path.as_ptr(), full_path.len())
        };

        let mut is_dir = false;

        if stat_len > 0 {
            let stat_data = crate::buffer::read_response(stat_len as usize);
            if let Ok(stat_str) = core::str::from_utf8(stat_data) {
                let stat_trimmed = stat_str.trim();
                // stat now returns "type size YYYY-MM-DD HH:MM"
                let mut parts = stat_trimmed.splitn(3, ' ');
                let ftype = parts.next().unwrap_or("file");
                let size_str = parts.next().unwrap_or("0");
                let date = parts.next().unwrap_or("                ");

                is_dir = ftype == "dir";

                // Right-aligned type (4 chars), colored for dirs
                if is_dir {
                    crate::write_bytes(BOLD_BLUE);
                    crate::write_bytes(b" dir");
                    crate::write_bytes(RESET);
                } else {
                    crate::write_bytes(b"file");
                }
                crate::write_bytes(b"  ");

                // Human-readable size (7 chars right-aligned); dirs show "-"
                crate::write_bytes(CYAN);
                if is_dir {
                    crate::write_bytes(b"      -");
                } else {
                    let size_val = parse_u64(size_str);
                    let size_buf = format_size(size_val);
                    crate::write_bytes(&size_buf);
                }
                crate::write_bytes(RESET);
                crate::write_bytes(b"  ");

                // Date (16 chars), dimmed
                crate::write_bytes(DIM);
                crate::write_bytes(date.as_bytes());
                crate::write_bytes(RESET);
                crate::write_bytes(b"  ");
            }
        }

        // Entry name, bold blue for dirs
        if is_dir {
            crate::write_bytes(BOLD_BLUE);
        }
        crate::write_bytes(entry.as_bytes());
        if is_dir {
            crate::write_bytes(RESET);
        }
        crate::write_bytes(b"\n");
    }
}

fn parse_u64(s: &str) -> u64 {
    let mut n: u64 = 0;
    for b in s.bytes() {
        if b >= b'0' && b <= b'9' {
            n = n * 10 + (b - b'0') as u64;
        } else {
            break;
        }
    }
    n
}
