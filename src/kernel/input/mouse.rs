//! Mouse event dispatch — resolves click/scroll coordinates to editor elements.
//!
//! Routes mouse events to the correct editor region:
//! - Gutter clicks → `GUTTER_CLICKED` event
//! - Text area clicks → cursor positioning
//! - Drag → selection extension
//! - Scroll → scroll commands
//! - Release → state clean-up

use std::collections::HashMap;

use crate::kernel::state::{Editor, MouseState, MouseEventKind};
use crate::kernel::input::event::MouseEvent;
use crate::kernel::render::surface::Surface;
use crate::kernel::render::gutter::GutterRegistry;
use crate::kernel::render::decorations::prefix_margin_width;
use crate::kernel::render::frame::{compute_scroll_state, cell_width};
use crate::kernel::render::view_tree::Pane;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;
use crate::kernel::command::execute_command;

/// Route a mouse event to the correct editor element.
pub fn dispatch_mouse(ed: &mut Editor, surface: &Surface, event: &MouseEvent) {
    // Gate on the `:mouse-support` Janet option.
    if ed.options.get("mouse-support").map(|s| s.as_str()) != Some("true") {
        return;
    }

    match event.kind {
        MouseEventKind::Click => dispatch_click(ed, surface, event),
        MouseEventKind::Drag => dispatch_drag(ed, surface, event),
        MouseEventKind::Scroll(delta) => dispatch_scroll(ed, delta),
        MouseEventKind::Release => dispatch_release(ed),
    }
}

// ── Click ─────────────────────────────────────────────────────────────────────

fn dispatch_click(ed: &mut Editor, surface: &Surface, event: &MouseEvent) {
    // Store press state.
    ed.mouse_state = MouseState {
        pressed: true,
        button: event.button.clone(),
        press_x: event.x,
        press_y: event.y,
        last_x: event.x,
        last_y: event.y,
        drag_active: false,
    };

    // Hit-test to find which pane, if any, was clicked.
    let (pane, pane_idx) = match hit_test_pane(ed, event.x, event.y) {
        Some(p) => p,
        None => return,
    };

    // Focus the clicked pane.
    ed.view_tree.focus(pane.id);

    let buf_id = match pane.buffer_id { Some(id) => id, None => return };

    // Check if the click is in the tab-bar / header row area.
    let tab_bar_rows: u16 = if ed.tab_bar_enabled { 1 } else { 0 };
    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + tab_bar_rows + header_rows;
    if event.y < row_offset {
        // Click in tab bar or header — ignore for now.
        return;
    }

    // Detect double-click on same position (coordinate-based, ~500ms heuristic
    // stored as a plugin_state flag since we don't have a timer service here).
    let is_double = ed.mouse_state.press_x == event.x
        && ed.mouse_state.press_y == event.y
        && ed.plugin_state.get("_last_click_pos") == Some(&format!("{},{}", event.x, event.y));

    // Check gutter area vs text area.
    if is_in_gutter(ed, &pane, buf_id, event.x) {
        dispatch_gutter_click(ed, surface, event.x, event.y, &pane, pane_idx, buf_id);
    } else {
        dispatch_text_click(ed, event, &pane, buf_id, is_double);
    }

    // Stamp the click position for double-click detection (the check uses the
    // MouseState fields which are set before this call).
    ed.plugin_state.insert("_last_click_pos".to_string(), format!("{},{}", event.x, event.y));
}

/// Resolve and emit a gutter click.
fn dispatch_gutter_click(
    ed: &mut Editor,
    _surface: &Surface,
    click_x: u16,
    click_y: u16,
    pane: &Pane,
    _pane_idx: usize,
    buf_id: usize,
) {
    if ed.gutter.providers.is_empty() { return; }

    let tab_bar_rows: u16 = if ed.tab_bar_enabled { 1 } else { 0 };
    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + tab_bar_rows + header_rows;

    let total_lines = ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap().line_count())
        .unwrap_or(1);

    // Determine which gutter provider column was clicked.
    let mut col_x = pane.x;
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
    let visible_lines = (pane.height as usize)
        .saturating_sub((tab_bar_rows + header_rows) as usize);
    let scroll_state = compute_scroll_state(ed, buf_id, visible_lines);
    let abs_line = scroll_state.scroll_top + vis_row;

    ed.events.emit_typed(keys::events::GUTTER_CLICKED, GutterClickedPayload {
        column: col_name,
        line: abs_line.to_string(),
        buf: buf_id.to_string(),
    });
    ed.events.drain_and_dispatch();
}

/// Move cursor to the clicked text position and fire events.
fn dispatch_text_click(
    ed: &mut Editor,
    event: &MouseEvent,
    pane: &Pane,
    buf_id: usize,
    _is_double: bool,
) {
    let offset = match resolve_click_to_offset(ed, buf_id, pane, event.x, event.y) {
        Some(o) => o,
        None => return,
    };

    if let Some(view) = ed.views.get_mut(&buf_id) {
        view.set_cursor(offset);
    }

    // Clear selection unless shift is held (future: extend-selection on shift-click).
    if ed.selection.is_some() && !event.modifiers.contains("shift") {
        ed.selection = None;
        ed.events.emit_typed(keys::events::SELECTION_CLEARED, EmptyPayload);
    }

    ed.events.emit_typed(keys::events::CURSOR_MOVED, CursorMovedPayload {
        buffer_id: buf_id.to_string(),
        cursor: offset.to_string(),
    });
    ed.events.emit_typed(keys::events::TEXT_CLICKED, TextClickedPayload {
        buf: buf_id.to_string(),
        line: "0".to_string(),
        col: "0".to_string(),
        button: format!("{:?}", event.button),
    });
    ed.events.drain_and_dispatch();
}

/// Convert screen coordinates to a buffer byte offset.
pub(crate) fn resolve_click_to_offset(
    ed: &Editor,
    buf_id: usize,
    pane: &Pane,
    click_x: u16,
    click_y: u16,
) -> Option<usize> {
    let tab_bar_rows: u16 = if ed.tab_bar_enabled { 1 } else { 0 };
    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + tab_bar_rows + header_rows;
    if click_y < row_offset { return None; }
    let vis_row = (click_y - row_offset) as usize;

    let total_lines = ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap().line_count())
        .unwrap_or(1);
    let gutter_width = compute_gutter_width(ed, buf_id, total_lines);
    let content_x = pane.x + gutter_width as u16;
    if click_x < content_x { return None; }
    let click_col = (click_x - content_x) as usize;

    let visible_lines = (pane.height as usize)
        .saturating_sub((tab_bar_rows + header_rows) as usize);

    // Compute scroll state BEFORE taking the buffer lock, to avoid deadlock.
    let scroll_state = compute_scroll_state(ed, buf_id, visible_lines);
    let abs_line = scroll_state.scroll_top + vis_row;

    let arc = ed.buffers.get(buf_id)?;
    let buf = arc.lock().ok()?;
    if abs_line >= buf.line_count() { return None; }
    let text = buf.slice(0, buf.len());

    let line_start = buf.line_start_offset(abs_line)?;
    let line_end = buf.line_start_offset(abs_line + 1).unwrap_or(buf.len());
    let line_text = &text[line_start..line_end];

    // Walk characters accumulating display width.
    let mut display = 0usize;
    let mut byte_offset = line_start;
    for ch in line_text.chars() {
        let w = cell_width(ch, &ed.font_config.glyph_widths);
        if display + w > click_col { break; }
        display += w;
        byte_offset += ch.len_utf8();
    }
    Some(byte_offset.min(line_end))
}

// ── Drag ──────────────────────────────────────────────────────────────────────

fn dispatch_drag(ed: &mut Editor, _surface: &Surface, event: &MouseEvent) {
    if !ed.mouse_state.pressed { return; }
    ed.mouse_state.drag_active = true;
    ed.mouse_state.last_x = event.x;
    ed.mouse_state.last_y = event.y;

    let (pane, _) = match hit_test_pane(ed, event.x, event.y) {
        Some(p) => p,
        None => return,
    };
    let buf_id = match pane.buffer_id { Some(id) => id, None => return };

    let current = resolve_click_to_offset(ed, buf_id, &pane, event.x, event.y);
    let press = resolve_click_to_offset(ed, buf_id, &pane, ed.mouse_state.press_x, ed.mouse_state.press_y);
    let (anchor, cursor) = match (press, current) {
        (Some(p), Some(c)) => (p, c),
        _ => return,
    };

    ed.selection = Some(crate::kernel::state::mode::Selection::char(anchor));
    if let Some(view) = ed.views.get_mut(&buf_id) {
        view.set_cursor(cursor);
    }

    ed.events.emit_typed(keys::events::SELECTION_CHANGED, /* minimal payload */ EmptyPayload);
    // Emit the drag event.
    ed.events.emit_typed(keys::events::MOUSE_DRAGGED, MouseDraggedPayload {
        buf: buf_id.to_string(),
        start_line: "0".to_string(),
        start_col: "0".to_string(),
        end_line: "0".to_string(),
        end_col: "0".to_string(),
    });
    ed.events.drain_and_dispatch();
}

// ── Scroll ────────────────────────────────────────────────────────────────────

fn dispatch_scroll(ed: &mut Editor, delta: i32) {
    let cmd = if delta > 0 { "scroll-line-down" } else { "scroll-line-up" };
    let _ = execute_command(ed, cmd, &HashMap::new());
    ed.events.drain_and_dispatch();
}

// ── Release ───────────────────────────────────────────────────────────────────

fn dispatch_release(ed: &mut Editor) {
    ed.mouse_state.pressed = false;
    ed.mouse_state.drag_active = false;
}

// ── Hit testing ───────────────────────────────────────────────────────────────

/// Find the pane at `(x, y)` screen coordinates.
pub(crate) fn hit_test_pane(ed: &Editor, x: u16, y: u16) -> Option<(Pane, usize)> {
    for (i, pane) in ed.view_tree.panes().iter().enumerate() {
        if x >= pane.x && x < pane.x + pane.width
            && y >= pane.y && y < pane.y + pane.height
        {
            return Some((pane.clone(), i));
        }
    }
    None
}

/// Returns `true` when the x-coordinate falls within the gutter region.
fn is_in_gutter(ed: &Editor, pane: &Pane, buf_id: usize, click_x: u16) -> bool {
    let total_lines = ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap().line_count())
        .unwrap_or(1);
    let gutter_width = compute_gutter_width(ed, buf_id, total_lines);
    click_x < pane.x + gutter_width as u16
}

fn compute_gutter_width(ed: &Editor, buf_id: usize, total_lines: usize) -> usize {
    let prefix_margin = prefix_margin_width(ed, buf_id);
    let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };
    prefix_cols + ed.gutter.total_width(total_lines)
}

fn gutter_header_rows(ed: &Editor, buf_id: usize) -> u16 {
    if ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap().header_line.is_some())
        .unwrap_or(false)
    { 1 } else { 0 }
}
