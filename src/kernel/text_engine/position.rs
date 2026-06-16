/// Canonical position type: byte offset + line + column.
/// Line and column are 0-indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub fn zero() -> Self { Self::default() }

    /// Compute position from a byte offset in the given rope text.
    pub fn from_offset(text: &str, offset: usize) -> Self {
        let offset = offset.min(text.len());
        let before = &text[..offset];
        let line = before.chars().filter(|&c| c == '\n').count();
        let column = before.rfind('\n')
            .map(|p| before[p + 1..].chars().count())
            .unwrap_or_else(|| before.chars().count());
        Position { line, column, offset }
    }

    /// Compute position from line+col in the given rope text.
    pub fn from_line_col(text: &str, line: usize, col: usize) -> Self {
        let mut line_start = 0;
        let mut current_line = 0;
        for (i, ch) in text.char_indices() {
            if current_line == line {
                let line_text = &text[line_start..];
                let byte_col = line_text.char_indices()
                    .nth(col)
                    .map(|(bi, _)| bi)
                    .unwrap_or_else(|| {
                        line_text.find('\n').unwrap_or(line_text.len())
                    });
                let offset = line_start + byte_col;
                return Position { line, column: col, offset };
            }
            if ch == '\n' {
                current_line += 1;
                line_start = i + 1;
            }
        }
        // line is at or past end
        let offset = text.len();
        let col_at_end = text.rfind('\n')
            .map(|p| text[p+1..].chars().count())
            .unwrap_or_else(|| text.chars().count());
        Position { line: current_line, column: col_at_end, offset }
    }
}
