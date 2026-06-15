/// An overlay (floating window) drawn above buffer content.
#[derive(Debug, Clone)]
pub struct Overlay {
    pub id: usize,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    /// Buffer whose content fills the overlay area, if any.
    pub buffer_id: Option<usize>,
    /// Higher z_order is drawn on top.
    pub z_order: i32,
}

/// Snapshot of the last-rendered frame's layout, written by `render_frame`
/// each frame so `input::mouse::dispatch_mouse` can resolve click coordinates
/// without recomputing the gutter / content layout from scratch.
#[derive(Debug, Clone, Default)]
pub struct LayoutSnapshot {
    /// Total gutter width in cells (sum of all visible column widths).
    pub gutter_width: usize,
    /// X column where buffer content begins (gutter_width + prefix_margin).
    pub content_x: u16,
    /// Number of rows consumed by the tab bar + header line at the top.
    pub row_offset: u16,
    /// Scroll offset (first visible buffer line index) at the time of rendering.
    pub scroll_top: usize,
    /// Number of content rows rendered (excluding status bar).
    pub visible_lines: usize,
}
