pub mod env;
pub mod fs;
pub mod io;
pub mod process;
pub mod workspace;

use anyhow::Result;
use wasmtime::Linker;

use crate::spi::state::HostState;

/// Register all host import functions with the linker.
pub fn register_all(linker: &mut Linker<HostState>) -> Result<()> {
    io::register(linker)?;
    fs::register(linker)?;
    env::register(linker)?;
    process::register(linker)?;
    workspace::register(linker)?;
    Ok(())
}
