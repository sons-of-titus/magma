//! Mouse event dispatch — resolves click/scroll coordinates to editor elements.
//!
//! `dispatch_mouse` is the single entry point for all mouse events.  It
//! determines whether the click landed in the gutter, buffer area, status bar,
//! or an overlay, and emits the appropriate named event through the event bus.
//!
//! This module absorbs `dispatch_gutter_click` from `main.rs` so that gutter
//! layout arithmetic is not duplicated between `render_frame` and the event loop.

use crate::state::Editor;
use crate::render::surface::Surface;

/// Route a mouse event to the correct editor element.
///
/// Currently handles left-button clicks on gutter columns; future variants can
/// add scroll, right-click, drag, and overlay events without touching `main.rs`.
pub fn dispatch_mouse(ed: &mut Editor, surface: &Surface, x: u16, y: u16) {
    dispatch_gutter_click(ed, surface, x, y);
}

/// Resolve a gutter click and emit `gutter-clicked`.
///
/// Reconstructs the gutter layout from Editor state to determine which named
/// column was clicked and which absolute buffer line corresponds to the row.
/// Uses `compute_scroll_state` from `render_frame` so the two are always in sync.
pub fn dispatch_gutter_click(ed: &mut Editor, surface: &Surface, click_x: u16, click_y: u16) {
    if ed.gutter.columns.is_empty() { return; }

    let buf_id = match ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid)) {
        Some(id) => id,
        None => return,
    };

    let tab_bar_rows: u16 = if ed.tab_bar_enabled { 1 } else { 0 };
    let header_rows: u16 = if ed.buffers.get(buf_id)
        .map(|b| b.header_line.is_some()).unwrap_or(false) { 1 } else { 0 };
    let row_offset = tab_bar_rows + header_rows;

    let prefix_margin = crate::render::decorations::prefix_margin_width(ed, buf_id);
    let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };

    let total_lines = ed.buffers.get(buf_id).map(|b| b.line_count()).unwrap_or(1);
    let ln_col_width = (total_lines.max(1).ilog10() as usize + 1).max(2) + 1;

    let mut col_x = prefix_cols as u16;
    let mut clicked_col: Option<String> = None;
    for col in &ed.gutter.columns {
        if !col.visible { continue; }
        let col_w = if col.width == 0 { ln_col_width } else { col.width } as u16;
        if click_x >= col_x && click_x < col_x + col_w {
            clicked_col = Some(col.name.clone());
            break;
        }
        col_x += col_w;
    }
    let col_name = match clicked_col { Some(n) => n, None => return };

    if click_y < row_offset { return; }
    let vis_row = (click_y - row_offset) as usize;
    let visible_lines = surface.height.saturating_sub(1 + tab_bar_rows) as usize;
    let scroll_state = crate::render::frame::compute_scroll_state(ed, buf_id, visible_lines);
    let abs_line = scroll_state.scroll_top + vis_row;

    let mut payload = std::collections::HashMap::new();
    payload.insert("column".to_string(), col_name);
    payload.insert("line".to_string(), abs_line.to_string());
    payload.insert("buf".to_string(), buf_id.to_string());
    ed.events.emit("gutter-clicked", payload);
    ed.events.drain_and_dispatch();
}
