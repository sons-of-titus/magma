//! 2D character grid for rendering.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            fg: (255, 255, 255),
            bg: (0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            dim: false,
        }
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug)]
pub struct Surface {
    pub width: u16,
    pub height: u16,
    cells: Vec<Cell>,
}

impl Surface {
    pub fn new(width: u16, height: u16) -> Self {
        Surface {
            width,
            height,
            cells: vec![Cell::default(); (width as usize) * (height as usize)],
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.cells = vec![Cell::default(); (width as usize) * (height as usize)];
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.ch = ' ';
            cell.style = Style::default();
        }
    }

    pub fn set_cell(&mut self, x: u16, y: u16, ch: char, style: Option<Style>) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        if idx < self.cells.len() {
            self.cells[idx].ch = ch;
            if let Some(s) = style {
                self.cells[idx].style = s;
            }
        }
    }

    pub fn set_text(&mut self, x: u16, y: u16, text: &str, style: Option<Style>) {
        for (i, ch) in text.chars().enumerate() {
            self.set_cell(x + i as u16, y, ch, style);
        }
    }

    pub fn cell(&self, x: u16, y: u16) -> Option<&Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.cells.get(idx)
    }

    pub fn rows(&self) -> Vec<Vec<&Cell>> {
        let mut rows = Vec::with_capacity(self.height as usize);
        for y in 0..self.height {
            let start = (y as usize) * (self.width as usize);
            let end = start + (self.width as usize);
            rows.push(self.cells[start..end].iter().collect());
        }
        rows
    }
}
