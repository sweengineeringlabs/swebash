use std::io::{self, Write};
use std::path::PathBuf;

use super::tab::TabManager;

/// Set the terminal scroll region to rows 2..height so that row 1 (0-indexed
/// row 0) is pinned above the scrollable area.
pub fn setup_scroll_region() {
    if let Ok((_, height)) = crossterm::terminal::size() {
        // CSI scroll region: rows are 1-indexed
        print!("\x1b[2;{}r", height);
        // Move cursor to row 2 (the first scrollable row)
        print!("\x1b[2;1H");
        let _ = io::stdout().flush();
    }
}

/// Reset the scroll region to the full terminal.
pub fn reset_scroll_region() {
    print!("\x1b[r");
    let _ = io::stdout().flush();
}

/// Render the tab bar at terminal row 0.
///
/// Format: `[1:>:~/proj]  [*2:AI:shell]  [3:>:/tmp]`
/// Active tab: bold white. Inactive: dark grey.
pub fn render_tab_bar(tab_mgr: &TabManager, home: Option<&PathBuf>) {
    let term_width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);

    // Save cursor position
    print!("\x1b7");
    // Move to row 1, col 1 (1-indexed)
    print!("\x1b[1;1H");
    // Clear the entire line
    print!("\x1b[2K");

    let mut bar = String::new();
    let mut visible_len: usize = 0;

    for (i, tab) in tab_mgr.tabs.iter().enumerate() {
        let is_active = i == tab_mgr.active;
        let label = tab.display_label(home);
        let num = i + 1;
        let entry = format!("[{}:{}]", num, label);
        let entry_len = entry.len();

        // Check if adding this tab would exceed terminal width
        let separator_len = if bar.is_empty() { 0 } else { 2 };
        if visible_len + separator_len + entry_len > term_width {
            // Truncate: show "..." for remaining tabs
            if visible_len + 5 <= term_width {
                bar.push_str("  ...");
            }
            break;
        }

        if !bar.is_empty() {
            bar.push_str("  ");
            visible_len += 2;
        }

        if is_active {
            // Bold white on default background
            bar.push_str(&format!("\x1b[1;37m{}\x1b[0m", entry));
        } else {
            // Dark grey
            bar.push_str(&format!("\x1b[90m{}\x1b[0m", entry));
        }
        visible_len += entry_len;
    }

    print!("{bar}");
    // Restore cursor position
    print!("\x1b8");
    let _ = io::stdout().flush();
}
