//! A tiny, dependency-free column-aligned table for human terminal output.
//!
//! Cells carry both their plain text (used to measure column width) and their
//! styled text (what is actually printed). Width is always computed from the
//! plain text, so ANSI color codes never disturb alignment. Columns are
//! left-aligned and separated by a fixed gap; the renderer targets fixed-width
//! terminal fonts and does not attempt Unicode grapheme width.

/// A single table cell: `width` text for alignment, `display` text for output.
#[derive(Debug, Clone)]
pub struct Cell {
    width: String,
    display: String,
}

impl Cell {
    /// A plain cell whose displayed text is also its measured width.
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            width: text.clone(),
            display: text,
        }
    }

    /// A styled cell: `plain` is measured for alignment, `display` is printed
    /// (typically the same text wrapped in ANSI codes).
    pub fn styled(plain: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            width: plain.into(),
            display: display.into(),
        }
    }
}

/// Render `rows` as a left-aligned table separated by `gap` spaces between
/// columns. The last column is not padded (no trailing spaces). Each row should
/// have the same number of cells; short rows are padded with blanks.
pub fn render(rows: &[Vec<Cell>], gap: usize) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.width.chars().count());
        }
    }

    let separator = " ".repeat(gap);
    let mut output = String::new();
    for row in rows {
        let mut line = String::new();
        let last = row.len().saturating_sub(1);
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                line.push_str(&separator);
            }
            line.push_str(&cell.display);
            // Pad every column except the last in the row, so the right edge
            // stays ragged (no trailing whitespace).
            if index != last {
                let pad = widths[index].saturating_sub(cell.width.chars().count());
                line.push_str(&" ".repeat(pad));
            }
        }
        output.push_str(line.trim_end());
        output.push('\n');
    }
    output
}
