/// Preprocessing: normalize markdown tables to prose for better embedding quality.
///
/// Converts markdown pipe tables to `"Key: Value. Key: Value."` prose sentences,
/// which embed more semantically than raw table markup.

/// Normalize markdown tables in a document to prose form for embedding.
///
/// Lines that are not part of a table are passed through unchanged.
///
/// # Algorithm
///
/// 1. Scan for consecutive lines starting with `|` (table blocks).
/// 2. Identify header row → separator row → data rows.
/// 3. Emit each data row as `Header1: value1. Header2: value2.`
/// 4. Strip backtick fences from cell values.
///
/// # Example
///
/// ```text
/// | Variable | Default | Description |
/// |----------|---------|-------------|
/// | `PORT` | `8080` | HTTP listen port |
/// ```
/// becomes:
/// ```text
/// Variable: PORT. Default: 8080. Description: HTTP listen port.
/// ```
pub fn normalize_markdown_tables(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let n = lines.len();
    let mut result = String::new();
    let mut i = 0;

    while i < n {
        if is_table_line(lines[i]) {
            let start = i;
            while i < n && is_table_line(lines[i]) {
                i += 1;
            }
            result.push_str(&process_table(&lines[start..i]));
        } else {
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
        }
    }

    result
}

/// Returns `true` when `line` (possibly with leading whitespace) starts with `|`.
fn is_table_line(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

/// Returns `true` when a table line is a separator row (e.g. `|---|---|`).
fn is_separator_line(line: &str) -> bool {
    let cells = parse_cells_raw(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let t = c.trim();
            !t.is_empty() && t.chars().all(|ch| matches!(ch, '-' | ':'))
        })
}

/// Split a `| A | B | C |` line into raw (un-stripped) cell slices.
fn parse_cells_raw(line: &str) -> Vec<&str> {
    let trimmed = line.trim();
    let inner = if trimmed.starts_with('|') {
        &trimmed[1..]
    } else {
        trimmed
    };
    let inner = if inner.ends_with('|') {
        &inner[..inner.len() - 1]
    } else {
        inner
    };
    inner.split('|').collect()
}

/// Split a table line into trimmed, backtick-stripped cell values.
fn parse_cells(line: &str) -> Vec<String> {
    parse_cells_raw(line)
        .into_iter()
        .map(|c| strip_backticks(c.trim()))
        .collect()
}

/// Remove a single layer of surrounding backticks from a cell value.
///
/// `` `PORT` `` → `PORT`; `foo` → `foo`
fn strip_backticks(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('`') && s.ends_with('`') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Convert a slice of consecutive table lines to prose sentences.
fn process_table(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    // Locate the separator row.
    let sep_idx = lines.iter().position(|l| is_separator_line(l));

    // Determine headers and which lines are data rows.
    let (headers, data_lines): (Vec<String>, &[&str]) = match sep_idx {
        Some(sep) => {
            // Standard markdown table: header before separator, data after.
            let headers = if sep > 0 {
                parse_cells(lines[sep - 1])
            } else {
                // Separator is the very first line — no explicit header row.
                vec![]
            };
            (headers, &lines[sep + 1..])
        }
        None => {
            // No separator: treat the first line as header, rest as data.
            if lines.len() >= 2 {
                (parse_cells(lines[0]), &lines[1..])
            } else {
                // Single table line with no separator — no headers.
                (vec![], lines)
            }
        }
    };

    let mut result = String::new();

    for line in data_lines {
        let cells = parse_cells(line);
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }

        let parts: Vec<String> = if headers.is_empty() {
            // Fallback: label columns by 1-based index.
            cells
                .iter()
                .enumerate()
                .filter(|(_, c)| !c.is_empty())
                .map(|(i, c)| format!("Col{}: {}", i + 1, c))
                .collect()
        } else {
            headers
                .iter()
                .zip(cells.iter())
                .filter(|(_, c)| !c.is_empty())
                .map(|(h, c)| {
                    if h.is_empty() {
                        c.clone()
                    } else {
                        format!("{}: {}", h, c)
                    }
                })
                .collect()
        };

        if !parts.is_empty() {
            result.push_str(&parts.join(". "));
            result.push_str(".\n");
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_table_to_prose() {
        let input = "| Variable | Default | Description |\n\
                     |----------|---------|-------------|\n\
                     | `PORT` | `8080` | HTTP listen port |\n";
        let output = normalize_markdown_tables(input);
        assert_eq!(
            output,
            "Variable: PORT. Default: 8080. Description: HTTP listen port.\n"
        );
    }

    #[test]
    fn test_non_table_lines_pass_through() {
        let input = "# Heading\nSome prose text.\nAnother line.\n";
        let output = normalize_markdown_tables(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_mixed_table_and_prose() {
        let input = "Before table.\n\
                     | A | B |\n\
                     |---|---|\n\
                     | 1 | 2 |\n\
                     After table.\n";
        let output = normalize_markdown_tables(input);
        assert_eq!(output, "Before table.\nA: 1. B: 2.\nAfter table.\n");
    }

    #[test]
    fn test_table_without_header_uses_col_labels() {
        // Separator as first line → no explicit header.
        let input = "|---|\n\
                     | foo |\n";
        let output = normalize_markdown_tables(input);
        assert!(output.contains("Col1: foo"), "got: {output:?}");
    }

    #[test]
    fn test_empty_cells_are_skipped() {
        let input = "| A | B | C |\n\
                     |---|---|---|\n\
                     | x |   | z |\n";
        let output = normalize_markdown_tables(input);
        assert!(output.contains("A: x"), "got: {output:?}");
        assert!(output.contains("C: z"), "got: {output:?}");
        assert!(!output.contains("B:"), "got: {output:?}");
    }

    #[test]
    fn test_backtick_stripping() {
        let input = "| Key | Value |\n\
                     |-----|-------|\n\
                     | `HOST` | `localhost` |\n";
        let output = normalize_markdown_tables(input);
        assert_eq!(output, "Key: HOST. Value: localhost.\n");
    }

    #[test]
    fn test_multiple_data_rows() {
        let input = "| Name | Age |\n\
                     |------|-----|\n\
                     | Alice | 30 |\n\
                     | Bob | 25 |\n";
        let output = normalize_markdown_tables(input);
        assert!(output.contains("Name: Alice. Age: 30."), "got: {output:?}");
        assert!(output.contains("Name: Bob. Age: 25."), "got: {output:?}");
    }

    #[test]
    fn test_no_data_rows_produces_empty() {
        // Table with only header + separator, no data rows.
        let input = "| A | B |\n\
                     |---|---|\n";
        let output = normalize_markdown_tables(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(normalize_markdown_tables(""), "");
    }

    #[test]
    fn test_aligned_separators() {
        // Aligned columns use `:---:` style separators.
        let input = "| Left | Center | Right |\n\
                     |:-----|:------:|------:|\n\
                     | a | b | c |\n";
        let output = normalize_markdown_tables(input);
        assert!(output.contains("Left: a"), "got: {output:?}");
        assert!(output.contains("Center: b"), "got: {output:?}");
        assert!(output.contains("Right: c"), "got: {output:?}");
    }

    #[test]
    fn test_two_tables_in_one_document() {
        let input = "# Section 1\n\
                     | K | V |\n\
                     |---|---|\n\
                     | x | 1 |\n\
                     # Section 2\n\
                     | P | Q |\n\
                     |---|---|\n\
                     | y | 2 |\n";
        let output = normalize_markdown_tables(input);
        assert!(output.contains("K: x. V: 1."), "got: {output:?}");
        assert!(output.contains("P: y. Q: 2."), "got: {output:?}");
        assert!(output.contains("# Section 1"), "got: {output:?}");
        assert!(output.contains("# Section 2"), "got: {output:?}");
    }
}
