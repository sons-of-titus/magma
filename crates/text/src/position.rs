/// Canonical position type: byte offset + line + column.
/// Line and column are 0-indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub fn zero() -> Self {
        Self::default()
    }
}
