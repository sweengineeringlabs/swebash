/// L2 API: Public types and traits for the readline crate.
///
/// Re-exports the main user-facing types from the core layer.
pub use crate::core::completer::{Completer, Completion};
pub use crate::core::config::{ColorConfig, ReadlineConfig};
pub use crate::core::editor::{EditorAction, LineEditor};
pub use crate::core::highlighter::Highlighter;
pub use crate::core::hinter::Hinter;
pub use crate::core::history::History;
pub use crate::core::validator::{ValidationResult, Validator};
