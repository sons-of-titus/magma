//! Mouse event dispatch — resolves click/scroll coordinates to editor elements.

use crate::kernel::state::Editor;
use crate::kernel::render::surface::Surface;
use crate::kernel::render::gutter::GutterRegistry;
use crate::kernel::render::decorations::prefix_margin_width;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

/// Route a mouse event to the correct editor element.
pub fn dispatch_mouse(ed: &mut Editor, surface: &Surface, x: u16, y: u16) {
    dispatch_gutter_click(ed, surface, x, y);
}

/// Resolve a gutter click and emit `gutter-clicked`.
pub fn dispatch_gutter_click(ed: &mut Editor, surface: &Surface, click_x: u16, click_y: u16) {
    if ed.gutter.providers.is_empty() { return; }

    let buf_id = match ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid)) {
        Some(id) => id,
        None => return,
    };

    let tab_bar_rows: u16 = if ed.tab_bar_enabled { 1 } else { 0 };
    let header_rows: u16 = if ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap().header_line.is_some()).unwrap_or(false) { 1 } else { 0 };
    let row_offset = tab_bar_rows + header_rows;

    let prefix_margin = prefix_margin_width(ed, buf_id);
    let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };

    let total_lines = ed.buffers.get(buf_id).map(|a| a.lock().unwrap().line_count()).unwrap_or(1);

    let mut col_x = prefix_cols as u16;
    let mut clicked_provider: Option<String> = None;
    for provider in &ed.gutter.providers {
        let col_w = GutterRegistry::provider_width(provider.as_ref(), total_lines) as u16;
        if click_x >= col_x && click_x < col_x + col_w {
            clicked_provider = Some(provider.name().to_string());
            break;
        }
        col_x += col_w;
    }
    let col_name = match clicked_provider { Some(n) => n, None => return };

    if click_y < row_offset { return; }
    let vis_row = (click_y - row_offset) as usize;
    let visible_lines = surface.height.saturating_sub(1 + tab_bar_rows) as usize;
    let scroll_state = crate::kernel::render::frame::compute_scroll_state(ed, buf_id, visible_lines);
    let abs_line = scroll_state.scroll_top + vis_row;

    ed.events.emit_typed(keys::events::GUTTER_CLICKED, GutterClickedPayload {
        column: col_name,
        line: abs_line.to_string(),
        buf: buf_id.to_string(),
    });
    ed.events.drain_and_dispatch();
}
