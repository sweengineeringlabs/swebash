//! Path utilities for cross-platform display.
//!
//! The shell uses Unix-style parsing where `\` is an escape character.
//! On Windows, paths use `\` as separator, which conflicts with this.
//! To allow users to copy-paste displayed paths back into commands,
//! we normalize paths to use forward slashes in user-facing output.

use std::path::Path;

/// Convert a path to a display string using forward slashes.
///
/// This allows users to copy-paste paths from shell output directly
/// into commands without needing to quote them.
///
/// On Windows, also strips the extended-length path prefix (`\\?\`)
/// that `canonicalize()` adds, as it's not user-friendly.
///
/// # Examples
///
/// ```ignore
/// // On Windows: C:\Users\alice -> C:/Users/alice
/// // On Windows: \\?\C:\Users\alice -> C:/Users/alice
/// // On Unix: /home/alice -> /home/alice (unchanged)
/// ```
pub fn display_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    // Strip Windows extended-length path prefix if present
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    // Replace backslashes with forward slashes for consistent display
    s.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn unix_path_unchanged() {
        let p = PathBuf::from("/home/alice/docs");
        assert_eq!(display_path(&p), "/home/alice/docs");
    }

    #[test]
    fn windows_path_normalized() {
        // Simulate a Windows-style path (works on any platform as a string test)
        let p = PathBuf::from("C:\\Users\\alice\\docs");
        let displayed = display_path(&p);
        // On Windows this will normalize; on Unix the path is stored as-is
        #[cfg(windows)]
        assert_eq!(displayed, "C:/Users/alice/docs");
        #[cfg(not(windows))]
        assert_eq!(displayed, "C:\\Users\\alice\\docs"); // No backslashes to replace on Unix
    }

    #[test]
    fn mixed_separators() {
        let p = PathBuf::from("C:\\Users/alice\\docs");
        let displayed = display_path(&p);
        #[cfg(windows)]
        assert_eq!(displayed, "C:/Users/alice/docs");
    }

    #[test]
    fn extended_path_prefix_stripped() {
        // Extended-length paths like \\?\C:\path (from canonicalize on Windows)
        let p = PathBuf::from("\\\\?\\C:\\Users\\alice");
        let displayed = display_path(&p);
        #[cfg(windows)]
        assert_eq!(displayed, "C:/Users/alice");
    }
}
