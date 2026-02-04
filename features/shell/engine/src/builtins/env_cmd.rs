extern crate alloc;
use alloc::string::String;

/// `env` — list all environment variables
pub fn run_env(_args: &[String]) {
    let len = unsafe { crate::spi::host::host_list_env() };
    if len < 0 {
        crate::write_err(b"env: failed to list environment\n");
        return;
    }
    let data = crate::api::buffer::read_response(len as usize);
    crate::write_bytes(data);
}

/// `export KEY=VAL` — set an environment variable
pub fn run_export(args: &[String]) {
    if args.is_empty() {
        // No args: behave like env
        run_env(args);
        return;
    }

    for arg in args {
        if let Some(eq_pos) = arg.find('=') {
            let key = &arg[..eq_pos];
            let val = &arg[eq_pos + 1..];
            unsafe {
                crate::spi::host::host_set_env(
                    key.as_ptr(), key.len(),
                    val.as_ptr(), val.len(),
                );
            }
        } else {
            crate::write_err(b"export: invalid format, use KEY=VALUE\n");
        }
    }
}
