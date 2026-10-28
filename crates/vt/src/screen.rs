//! TerminalScreen — a real ANSI emulator grid (rework.md §6).
//!
//! Sessions own their screen; the dock renders it directly.  The grid is
//! cells (char + style), with scrollback, scroll regions, alternate
//! screen, and 256-color/RGB SGR support — vim, htop, and TUIs render as
//! themselves, not as stripped text.  Parsing lives in `ansi.rs`.

/// Cap on retained scrollback lines; the front is dropped beyond this.
pub(super) const SCROLLBACK_LINES: usize = 10_000;

/// A terminal color: terminal default, a palette index (256-color), or RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// SGR text attributes for one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
    pub strike: bool,
}

impl Style {
    pub(super) fn reset(&mut self) {
        *self = Style::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            ch: ' ',
            style: Style::default(),
        }
    }
}

/// One styled run within a rendered line: columns `[start, start + len)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: u16,
    pub len: u16,
    pub style: Style,
}

/// A rendered row: flattened text plus its styled runs.
#[derive(Debug, Clone, Default)]
pub struct StyledLine {
    pub text: String,
    pub spans: Vec<Span>,
}

/// Parser state carried across `apply` calls — a PTY read can split an
/// escape sequence at any byte.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ParserState {
    pub phase: super::ansi::Phase,
    pub private: bool,
    pub params: String,
    pub intermediates: String,
}

impl Default for ParserState {
    fn default() -> Self {
        ParserState {
            phase: super::ansi::Phase::Ground,
            private: false,
            params: String::new(),
            intermediates: String::new(),
        }
    }
}

pub struct TerminalScreen {
    pub(super) parser: ParserState,
    pub(super) cols: u16,
    pub(super) rows: u16,
    pub(super) cells: Vec<Cell>,
    pub(super) cursor_row: u16,
    pub(super) cursor_col: u16,
    /// xterm wrap-pending: a write in the last column defers the wrap.
    pub(super) pending_wrap: bool,
    pub(super) autowrap: bool,
    pub(super) cursor_visible: bool,
    pub(super) style: Style,
    pub(super) saved_cursor: (u16, u16),
    pub(super) scroll_top: u16,
    pub(super) scroll_bottom: u16,
    pub(super) scrollback: std::collections::VecDeque<Vec<Cell>>,
    pub(super) alt: Option<AltScreen>,
    /// Bumped on every mutation; repaint scheduling mixes it.
    pub revision: u64,
}

#[derive(Debug)]
pub(super) struct AltScreen {
    cells: Vec<Cell>,
    cursor: (u16, u16),
    scrollback: std::collections::VecDeque<Vec<Cell>>,
    scroll_top: u16,
    scroll_bottom: u16,
}

impl Default for TerminalScreen {
    fn default() -> Self {
        Self::with_size(80, 24)
    }
}

impl TerminalScreen {
    /// Ingest raw PTY output — the ANSI parser drives the grid.
    pub fn apply(&mut self, data: &str) {
        super::ansi::feed(self, data);
    }
}

impl TerminalScreen {
    pub fn with_size(cols: u16, rows: u16) -> Self {
        TerminalScreen {
            parser: ParserState::default(),
            cols,
            rows,
            cells: vec![Cell::default(); cols as usize * rows as usize],
            cursor_row: 0,
            cursor_col: 0,
            pending_wrap: false,
            autowrap: true,
            cursor_visible: true,
            style: Style::default(),
            saved_cursor: (0, 0),
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            scrollback: std::collections::VecDeque::new(),
            alt: None,
            revision: 0,
        }
    }

    // ── Grid operations (pub(super): the ANSI parser drives these) ──

    pub(super) fn cell(&self, row: u16, col: u16) -> Cell {
        self.cells[row as usize * self.cols as usize + col as usize]
    }

    pub(super) fn put_cell(&mut self, row: u16, col: u16, cell: Cell) {
        let idx = row as usize * self.cols as usize + col as usize;
        if idx < self.cells.len() {
            self.cells[idx] = cell;
        }
    }

    pub(super) fn blank_with(&self, style: Style) -> Vec<Cell> {
        vec![Cell { ch: ' ', style }; self.cols as usize]
    }

    /// LF: advance within the scroll region, scrolling at its bottom.
    pub(super) fn index(&mut self) {
        if self.cursor_row == self.scroll_bottom {
            self.scroll_up(1);
        } else if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        }
        self.pending_wrap = false;
    }

    /// Reverse LF: move up within the region, scrolling at its top.
    pub(super) fn reverse_index(&mut self) {
        if self.cursor_row == self.scroll_top {
            self.scroll_down(1);
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
        }
        self.pending_wrap = false;
    }

    pub(super) fn scroll_up(&mut self, n: u16) {
        let n = n.min(self.scroll_bottom - self.scroll_top + 1) as usize;
        let top = self.scroll_top as usize;
        let bottom = self.scroll_bottom as usize;
        let full_row = |s: &Self, row: usize| {
            s.cells[row * s.cols as usize..(row + 1) * s.cols as usize].to_vec()
        };
        // Rows leaving a full-screen region become scrollback.
        if self.scroll_top == 0 && self.scroll_bottom == self.rows - 1 && self.alt.is_none() {
            for i in 0..n {
                self.scrollback.push_back(full_row(self, top + i));
            }
            while self.scrollback.len() > SCROLLBACK_LINES {
                self.scrollback.pop_front();
            }
        }
        for row in top..=bottom - n {
            let src = (row + n) * self.cols as usize;
            let dst = row * self.cols as usize;
            self.cells.copy_within(src..src + self.cols as usize, dst);
        }
        let blank = self.blank_with(self.style);
        for row in (bottom - n + 1)..=bottom {
            let dst = row * self.cols as usize;
            self.cells[dst..dst + self.cols as usize].copy_from_slice(&blank);
        }
        self.revision += 1;
    }

    pub(super) fn scroll_down(&mut self, n: u16) {
        let n = n.min(self.scroll_bottom - self.scroll_top + 1) as usize;
        let top = self.scroll_top as usize;
        let bottom = self.scroll_bottom as usize;
        for row in ((top + n)..=bottom).rev() {
            let src = (row - n) * self.cols as usize;
            let dst = row * self.cols as usize;
            self.cells.copy_within(src..src + self.cols as usize, dst);
        }
        let blank = self.blank_with(self.style);
        for row in top..top + n {
            let dst = row * self.cols as usize;
            self.cells[dst..dst + self.cols as usize].copy_from_slice(&blank);
        }
        self.revision += 1;
    }

    pub(super) fn erase_in_display(&mut self, mode: u16) {
        match mode {
            0 => {
                self.erase_in_line(0);
                let start = (self.cursor_row as usize + 1) * self.cols as usize;
                let blank = self.blank_with(self.style);
                for i in start..self.cells.len() {
                    self.cells[i] = blank[i % self.cols as usize];
                }
            }
            1 => {
                let end =
                    self.cursor_row as usize * self.cols as usize + self.cursor_col as usize + 1;
                let blank = self.blank_with(self.style);
                for i in 0..end {
                    self.cells[i] = blank[i % self.cols as usize];
                }
            }
            _ => {
                let blank = self.blank_with(self.style);
                for i in 0..self.cells.len() {
                    self.cells[i] = blank[i % self.cols as usize];
                }
                if mode == 3 {
                    self.scrollback.clear();
                }
            }
        }
        self.revision += 1;
    }

    pub(super) fn erase_in_line(&mut self, mode: u16) {
        let row = self.cursor_row as usize;
        let (start, end) = match mode {
            0 => (self.cursor_col as usize, self.cols as usize),
            1 => (0, self.cursor_col as usize + 1),
            _ => (0, self.cols as usize),
        };
        let blank = Cell {
            ch: ' ',
            style: self.style,
        };
        for col in start..end {
            self.put_cell(row as u16, col as u16, blank);
        }
        self.revision += 1;
    }

    /// Alternate screen (1049): save the main grid + scrollback, swap in a
    /// blank full-size screen.  Leaving restores exactly what was there.
    pub(super) fn enter_alt(&mut self) {
        if self.alt.is_some() {
            return;
        }
        let blank_grid = vec![Cell::default(); self.cells.len()];
        self.alt = Some(AltScreen {
            cells: std::mem::replace(&mut self.cells, blank_grid),
            cursor: (self.cursor_row, self.cursor_col),
            scrollback: std::mem::take(&mut self.scrollback),
            scroll_top: self.scroll_top,
            scroll_bottom: self.scroll_bottom,
        });
        self.scrollback = std::collections::VecDeque::new();
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.pending_wrap = false;
        self.revision += 1;
    }

    pub(super) fn leave_alt(&mut self) {
        if let Some(alt) = self.alt.take() {
            self.cells = alt.cells;
            (self.cursor_row, self.cursor_col) = alt.cursor;
            self.scrollback = alt.scrollback;
            self.scroll_top = alt.scroll_top;
            self.scroll_bottom = alt.scroll_bottom;
            self.pending_wrap = false;
            self.revision += 1;
        }
    }

    /// Grow/shrink to the PTY's size.  Rows are kept (bottom dropped on
    /// shrink, blanks appended on grow); no reflow — an honest limitation.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == self.cols && rows == self.rows {
            return;
        }
        let mut next = vec![Cell::default(); cols as usize * rows as usize];
        for row in 0..self.rows.min(rows) {
            for col in 0..self.cols.min(cols) {
                next[row as usize * cols as usize + col as usize] = self.cell(row, col);
            }
        }
        self.cells = next;
        self.cols = cols;
        self.rows = rows;
        self.scroll_top = 0;
        self.scroll_bottom = rows.saturating_sub(1);
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(cols - 1);
        self.revision += 1;
    }

    // ── Rendering surface ──

    pub fn line_count(&self) -> usize {
        self.scrollback.len() + self.rows as usize
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// Flattened text of one logical line (scrollback + screen).
    pub fn line(&self, i: usize) -> String {
        self.styled_line(i).text
    }

    /// Styled rendering of one logical line.
    pub fn styled_line(&self, i: usize) -> StyledLine {
        let cells: &[Cell] = if i < self.scrollback.len() {
            &self.scrollback[i]
        } else {
            let row = i - self.scrollback.len();
            if row >= self.rows as usize {
                return StyledLine::default();
            }
            &self.cells[row * self.cols as usize..(row + 1) * self.cols as usize]
        };
        let mut text = String::with_capacity(cells.len());
        let mut spans = Vec::new();
        let mut last: Option<Style> = None;
        for (col, cell) in cells.iter().enumerate() {
            text.push(cell.ch);
            let style = if cell.style.reverse {
                Style {
                    fg: cell.style.bg,
                    bg: cell.style.fg,
                    ..cell.style
                }
            } else {
                cell.style
            };
            if last != Some(style) {
                spans.push(Span {
                    start: col as u16,
                    len: 1,
                    style,
                });
                last = Some(style);
            } else if let Some(last_span) = spans.last_mut() {
                last_span.len += 1;
            }
        }
        while text.ends_with(' ') {
            text.pop();
        }
        StyledLine { text, spans }
    }

    /// The last `n` logical lines, oldest first.
    pub fn tail(&self, n: usize) -> Vec<String> {
        let total = self.line_count();
        (total.saturating_sub(n)..total)
            .map(|i| self.line(i))
            .collect()
    }

    pub fn tail_styled(&self, n: usize) -> Vec<StyledLine> {
        let total = self.line_count();
        (total.saturating_sub(n)..total)
            .map(|i| self.styled_line(i))
            .collect()
    }

    pub fn text(&self) -> String {
        (0..self.line_count())
            .map(|i| self.line(i))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn cursor_screen_row(&self) -> u16 {
        self.cursor_row
    }

    pub fn cursor_col(&self) -> u16 {
        self.cursor_col
    }

    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }
}
