//! ViewTree — manages visible panes with layout constraints (split, tab, float).
//!
//! Absorbs the layout logic previously in kernel/window/mod.rs.
//! WindowTree → ViewTree; Window → Pane.

use crate::kernel::state::id::WindowId;

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// How a pane participates in the layout tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutConstraint {
    /// Side-by-side (Horizontal) or stacked (Vertical) tiling split.
    Split(SplitDirection),
    /// Tabbed group — only one member visible at a time.
    Tab,
    /// Floating overlay at an explicit screen position.
    Float,
}

impl Default for LayoutConstraint {
    fn default() -> Self {
        LayoutConstraint::Split(SplitDirection::Horizontal)
    }
}

/// A single pane in the ViewTree (replaces Window).
#[derive(Debug, Clone)]
pub struct Pane {
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
    /// When true, scroll_offset is the explicit top line (not cursor-following).
    pub scroll_pinned: bool,
    /// How this pane was created in the layout tree.
    pub layout: LayoutConstraint,
}

impl Pane {
    pub fn new(id: WindowId) -> Self {
        Pane {
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
            layout: LayoutConstraint::default(),
        }
    }
}

/// ViewTree — manages all visible panes and their geometry.
///
/// The layout engine (redistribute) derives each pane's (x, y, width, height)
/// from fractional weights and terminal dimensions.  LayoutConstraint tags how
/// each pane was created; the engine currently implements horizontal weight-based
/// tiling and preserves the tag for future recursive layout passes.
pub struct ViewTree {
    panes: Vec<Pane>,
    focused: usize,
    next_id: u64,
    term_width: u16,
    term_height: u16,
}

impl Default for ViewTree {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewTree {
    pub fn new() -> Self {
        let pane = Pane::new(WindowId(1));
        ViewTree {
            panes: vec![pane],
            focused: 0,
            next_id: 2,
            term_width: 80,
            term_height: 24,
        }
    }

    /// Recompute pane geometry from weights — horizontal tiling, status-bar row reserved.
    fn redistribute(&mut self) {
        let tw = self.term_width;
        let th = self.term_height.saturating_sub(1);
        if self.panes.is_empty() { return; }
        if self.panes.len() == 1 {
            let p = &mut self.panes[0];
            p.x = 0; p.y = 0; p.width = tw; p.height = th;
            return;
        }
        let total: f32 = self.panes.iter().map(|p| p.weight.max(0.01)).sum();
        let n = self.panes.len();
        let mut x: u16 = 0;
        for (i, pane) in self.panes.iter_mut().enumerate() {
            let frac = pane.weight.max(0.01) / total;
            let win_w = if i == n - 1 {
                tw.saturating_sub(x)
            } else {
                ((frac * tw as f32) as u16).max(1)
            };
            pane.x = x; pane.y = 0;
            pane.width = win_w; pane.height = th;
            x += win_w;
        }
    }

    fn add_pane(&mut self, buf_id: Option<usize>, constraint: LayoutConstraint) -> WindowId {
        let new_id = WindowId(self.next_id);
        self.next_id += 1;
        let count = (self.panes.len() + 1) as f32;
        let equal_weight = 1.0 / count;
        for p in &mut self.panes {
            p.weight = equal_weight;
        }
        let mut new_pane = Pane::new(new_id);
        new_pane.buffer_id = buf_id;
        new_pane.weight = equal_weight;
        new_pane.layout = constraint;
        self.panes.push(new_pane);
        self.redistribute();
        new_id
    }

    pub fn resize_weighted(&mut self, id: WindowId, w_weight: f32) {
        if let Some(p) = self.pane_mut(id) {
            p.weight = w_weight.clamp(0.01, 1.0);
        }
        self.redistribute();
    }

    pub fn focused_index(&self) -> usize {
        self.focused
    }

    pub fn restore(&mut self, panes: Vec<Pane>, focused_idx: usize) {
        self.panes = panes;
        self.focused = focused_idx.min(self.panes.len().saturating_sub(1));
    }

    pub fn focused_window(&self) -> Option<WindowId> {
        self.panes.get(self.focused).map(|p| p.id)
    }

    pub fn focused_window_mut(&mut self) -> Option<&mut Pane> {
        self.panes.get_mut(self.focused)
    }

    pub fn pane(&self, id: WindowId) -> Option<&Pane> {
        self.panes.iter().find(|p| p.id == id)
    }

    pub fn pane_mut(&mut self, id: WindowId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// Alias kept for call-site compatibility.
    pub fn window(&self, id: WindowId) -> Option<&Pane> {
        self.pane(id)
    }

    /// Alias kept for call-site compatibility.
    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut Pane> {
        self.pane_mut(id)
    }

    pub fn buffer(&self, id: WindowId) -> Option<usize> {
        self.pane(id).and_then(|p| p.buffer_id)
    }

    pub fn set_buffer(&mut self, id: WindowId, buf_id: usize) {
        if let Some(p) = self.pane_mut(id) {
            p.buffer_id = Some(buf_id);
        }
    }

    pub fn close(&mut self, id: WindowId) {
        if self.panes.len() <= 1 { return; }
        if let Some(idx) = self.panes.iter().position(|p| p.id == id) {
            self.panes.remove(idx);
            if self.focused >= idx && self.focused > 0 {
                self.focused -= 1;
            }
            self.focused = self.focused.min(self.panes.len().saturating_sub(1));
            self.redistribute();
        }
    }

    pub fn focus_next(&mut self) {
        if !self.panes.is_empty() {
            self.focused = (self.focused + 1) % self.panes.len();
        }
    }

    pub fn focus(&mut self, id: WindowId) {
        if let Some(idx) = self.panes.iter().position(|p| p.id == id) {
            self.focused = idx;
        }
    }

    pub fn split_horizontal(&mut self, _id: WindowId) -> Option<WindowId> {
        let buf_id = self.focused_window().and_then(|id| self.buffer(id));
        Some(self.add_pane(buf_id, LayoutConstraint::Split(SplitDirection::Horizontal)))
    }

    pub fn split_vertical(&mut self, _id: WindowId) -> Option<WindowId> {
        let buf_id = self.focused_window().and_then(|id| self.buffer(id));
        Some(self.add_pane(buf_id, LayoutConstraint::Split(SplitDirection::Vertical)))
    }

    pub fn close_window(&mut self, id: WindowId) {
        self.close(id);
    }

    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    pub fn panes_mut(&mut self) -> &mut [Pane] {
        &mut self.panes
    }

    /// Alias kept for call-site compatibility.
    pub fn windows(&self) -> &[Pane] {
        &self.panes
    }

    /// Alias kept for call-site compatibility.
    pub fn windows_mut(&mut self) -> &mut [Pane] {
        &mut self.panes
    }

    pub fn len(&self) -> usize {
        self.panes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.term_width = width;
        self.term_height = height;
        self.redistribute();
    }

    /// Buffer ID shown in the focused pane, if any.
    pub fn focused_buffer(&self) -> Option<usize> {
        self.focused_window().and_then(|id| self.buffer(id))
    }

    /// Swap the positions of two panes by ID (used for tab drag-to-reorder).
    pub fn reorder(&mut self, from: WindowId, to: WindowId) {
        if let (Some(a), Some(b)) = (
            self.panes.iter().position(|p| p.id == from),
            self.panes.iter().position(|p| p.id == to),
        ) {
            self.panes.swap(a, b);
        }
    }
}
