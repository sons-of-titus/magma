/// Window and layout management.
use crate::kernel::state::id::WindowId;

#[derive(Debug, Clone)]
pub struct Window {
    pub id: WindowId,
    pub buffer_id: Option<usize>,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub scroll_offset: usize,
    /// Fractional layout weight [0.0, 1.0] for proportional horizontal sizing.
    pub weight: f32,
    /// When true, `scroll_offset` is the explicit top line set by `window/set-scroll-top`.
    pub scroll_pinned: bool,
}

impl Window {
    pub fn new(id: WindowId) -> Self {
        Window {
            id,
            buffer_id: None,
            x: 0,
            y: 0,
            width: 80,
            height: 24,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            weight: 1.0,
            scroll_pinned: false,
        }
    }
}

#[derive(Debug)]
pub struct WindowTree {
    windows: Vec<Window>,
    focused: usize,
    next_id: u64,
    term_width: u16,
    term_height: u16,
}

impl Default for WindowTree {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowTree {
    pub fn new() -> Self {
        let win = Window::new(WindowId(1));
        WindowTree {
            windows: vec![win],
            focused: 0,
            next_id: 2,
            term_width: 80,
            term_height: 24,
        }
    }

    /// Proportionally redistribute window widths based on each window's `weight`.
    /// All windows share the same height (terminal height minus the status bar row).
    fn redistribute(&mut self) {
        let tw = self.term_width;
        let th = self.term_height.saturating_sub(1);
        if self.windows.is_empty() { return; }
        if self.windows.len() == 1 {
            let w = &mut self.windows[0];
            w.x = 0; w.y = 0; w.width = tw; w.height = th;
            return;
        }
        let total: f32 = self.windows.iter().map(|w| w.weight.max(0.01)).sum();
        let n = self.windows.len();
        let mut x: u16 = 0;
        for (i, win) in self.windows.iter_mut().enumerate() {
            let frac = win.weight.max(0.01) / total;
            let win_w = if i == n - 1 {
                tw.saturating_sub(x)
            } else {
                ((frac * tw as f32) as u16).max(1)
            };
            win.x = x; win.y = 0;
            win.width = win_w; win.height = th;
            x += win_w;
        }
    }

    /// Add a new window beside the focused one with equal proportional weight.
    fn add_window(&mut self, buf_id: Option<usize>) -> WindowId {
        let new_id = WindowId(self.next_id);
        self.next_id += 1;
        let count = (self.windows.len() + 1) as f32;
        let equal_weight = 1.0 / count;
        for win in &mut self.windows {
            win.weight = equal_weight;
        }
        let mut new_win = Window::new(new_id);
        new_win.buffer_id = buf_id;
        new_win.weight = equal_weight;
        self.windows.push(new_win);
        self.redistribute();
        new_id
    }

    /// Set the fractional horizontal weight for a window and redistribute layout.
    pub fn resize_weighted(&mut self, id: WindowId, w_weight: f32) {
        if let Some(win) = self.window_mut(id) {
            win.weight = w_weight.clamp(0.01, 1.0);
        }
        self.redistribute();
    }

    /// Return the internal index of the focused window (for layout snapshots).
    pub fn focused_index(&self) -> usize {
        self.focused
    }

    /// Restore a previously saved window list and focused index.
    pub fn restore(&mut self, windows: Vec<Window>, focused_idx: usize) {
        self.windows = windows;
        self.focused = focused_idx.min(self.windows.len().saturating_sub(1));
    }

    pub fn focused_window(&self) -> Option<WindowId> {
        self.windows.get(self.focused).map(|w| w.id)
    }

    pub fn focused_window_mut(&mut self) -> Option<&mut Window> {
        self.windows.get_mut(self.focused)
    }

    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    pub fn buffer(&self, id: WindowId) -> Option<usize> {
        self.window(id).and_then(|w| w.buffer_id)
    }

    pub fn set_buffer(&mut self, id: WindowId, buffer_id: usize) {
        if let Some(win) = self.window_mut(id) {
            win.buffer_id = Some(buffer_id);
        }
    }

    pub fn close(&mut self, id: WindowId) {
        if self.windows.len() <= 1 {
            return;
        }
        let pos = self.windows.iter().position(|w| w.id == id);
        if let Some(idx) = pos {
            self.windows.remove(idx);
            if self.focused >= idx && self.focused > 0 {
                self.focused -= 1;
            }
            self.focused = self.focused.min(self.windows.len().saturating_sub(1));
            self.redistribute();
        }
    }

    pub fn focus_next(&mut self) {
        if !self.windows.is_empty() {
            self.focused = (self.focused + 1) % self.windows.len();
        }
    }

    pub fn focus(&mut self, id: WindowId) {
        if let Some(idx) = self.windows.iter().position(|w| w.id == id) {
            self.focused = idx;
        }
    }

    pub fn split_horizontal(&mut self, _id: WindowId) -> Option<WindowId> {
        let buf_id = self.focused_window().and_then(|id| self.buffer(id));
        Some(self.add_window(buf_id))
    }

    pub fn split_vertical(&mut self, _id: WindowId) -> Option<WindowId> {
        let buf_id = self.focused_window().and_then(|id| self.buffer(id));
        Some(self.add_window(buf_id))
    }

    pub fn close_window(&mut self, id: WindowId) {
        self.close(id);
    }

    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    pub fn windows_mut(&mut self) -> &mut [Window] {
        &mut self.windows
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.term_width = width;
        self.term_height = height;
        self.redistribute();
    }
}
