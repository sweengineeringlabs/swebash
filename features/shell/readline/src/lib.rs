/// swebash-readline: Shell-specific readline wrapper around swe-readline.
///
/// # Architecture (SEA Pattern)
///
/// - `api/` — public types re-exported at crate root
/// - `core/` — shell-specific implementations (ShellCompleter, ShellHighlighter)
/// - `spi/` — external provider integration (empty for now)
pub mod api;
pub mod core;
pub mod spi;

// Re-export the API surface at crate root for convenience.
pub use api::*;
