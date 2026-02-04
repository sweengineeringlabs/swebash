/// swebash-readline: Line editing, history, and completion for swebash.
///
/// # Architecture (SEA Pattern)
///
/// - `api/` — public types re-exported at crate root
/// - `core/` — implementations (editor, completer, hinter, highlighter, history, validator, config)
/// - `spi/` — external provider integration (empty for now)
pub mod api;
pub mod core;
pub mod spi;

// Re-export the API surface at crate root for convenience.
pub use api::*;
