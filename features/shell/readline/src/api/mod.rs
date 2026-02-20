/// L2 API: Public types and traits for the swebash readline crate.
///
/// Re-exports everything from swe-readline, plus shell-specific types.

// Re-export all generic readline types
pub use swe_readline::{
    common_prefix, ColorConfig, Complete, Completion, EditMode, EditorAction, Highlight, Hinter,
    History, LineEditor, NoComplete, NoHighlight, PathCompleter, ReadlineConfig, ValidationResult,
    Validator, visible_width,
};

// Re-export shell-specific implementations (aliased for backward compat)
pub use crate::core::completer::ShellCompleter as Completer;
pub use crate::core::highlighter::ShellHighlighter as Highlighter;

// Also export the unaliased names for new code
pub use crate::core::completer::ShellCompleter;
pub use crate::core::highlighter::ShellHighlighter;
