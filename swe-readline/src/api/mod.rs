/// L2 API: Public types and traits for the readline crate.
///
/// Re-exports the main user-facing types from the core layer.
pub use crate::core::completer::{common_prefix, Complete, Completion, NoComplete, PathCompleter};
pub use crate::core::config::{ColorConfig, EditMode, ReadlineConfig};
pub use crate::core::editor::{visible_width, EditorAction, LineEditor};
pub use crate::core::highlighter::{Highlight, NoHighlight};
pub use crate::core::hinter::Hinter;
pub use crate::core::history::History;
pub use crate::core::validator::{ValidationResult, Validator};
