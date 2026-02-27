#![forbid(unsafe_code)]

/// swe-readline: Shared line editing, history, and completion library.
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
