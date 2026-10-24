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

/// Splice server hover text into the visible hover card (server text
/// first), or surface it as a status message when no card is showing —
/// that is the manual `lsp-hover` path.  Called from the event pump.
pub fn attach_lsp_hover(ed: &mut crate::kernel::state::Editor, buf_key: usize, contents: &str) {
    let focused = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid));
    if focused == Some(buf_key)
        && let Some(card) = ed.hover.as_mut()
    {
        let mut lines: Vec<String> = contents.lines()
            .map(str::trim_end)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        lines.append(&mut card.lines);
        card.lines = lines;
        return;
    }
    let first = contents.lines().find(|l| !l.trim().is_empty()).unwrap_or(contents);
    ed.show_message(crate::kernel::state::MessageKind::Info,
        first.chars().take(160).collect::<String>());
}
