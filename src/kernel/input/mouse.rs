//! Mouse event dispatch — resolves click/scroll coordinates to editor elements.
//!
//! Routes mouse events to the correct editor region:
//! - Gutter clicks → `GUTTER_CLICKED` event
//! - Text area clicks → cursor positioning
//! - Drag → selection extension
//! - Scroll → viewport scrolling (cursor untouched)
//! - Release → state clean-up

use crate::kernel::state::{Editor, MouseEventKind};
use crate::kernel::input::event::MouseButton;
use crate::kernel::input::event::MouseEvent;
use crate::kernel::render::surface::Surface;
use crate::kernel::render::gutter::GutterRegistry;
use crate::kernel::render::decorations::prefix_margin_width;
use crate::kernel::render::frame::pane_scroll_top;
use crate::kernel::render::view_tree::Pane;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

/// Route a mouse event to the correct editor element.
pub fn dispatch_mouse(ed: &mut Editor, surface: &Surface, event: &MouseEvent) {
    // Gate on the `:mouse-support` Janet option.
    if ed.options.get("mouse-support").map(|s| s.as_str()) != Some("true") {
        return;
    }
    ed.frame_dirty = true;

    match event.kind {
        MouseEventKind::Click => dispatch_click(ed, surface, event),
        MouseEventKind::Drag => dispatch_drag(ed, surface, event),
        MouseEventKind::Scroll(delta) => dispatch_scroll(ed, event.x, event.y, delta),
        MouseEventKind::ScrollX(delta) => dispatch_scroll_x(ed, event.x, event.y, delta),
        MouseEventKind::Release => dispatch_release(ed),
    }
}

// ── Click ─────────────────────────────────────────────────────────────────────

fn dispatch_click(ed: &mut Editor, surface: &Surface, event: &MouseEvent) {
    // Multi-click counting: same cell within 500ms continues the sequence.
    let now = std::time::Instant::now();
    let count = match ed.mouse_state.last_click {
        Some((at, x, y)) if at.elapsed().as_millis() <= 500
            && x == event.x && y == event.y => ed.mouse_state.click_count.min(2) + 1,
        _ => 1,
    };
    ed.mouse_state.last_click = Some((now, event.x, event.y));
    ed.mouse_state.click_count = count;

    // Store press state.
    ed.mouse_state.pressed = true;
    ed.mouse_state.button = event.button.clone();
    ed.mouse_state.press_x = event.x;
    ed.mouse_state.press_y = event.y;
    ed.mouse_state.last_x = event.x;
    ed.mouse_state.last_y = event.y;
    ed.mouse_state.drag_active = false;

    // Only the primary button places the cursor; secondary is the context
    // menu's, middle is unused.
    if event.button != MouseButton::Left {
        return;
    }

    // Hit-test to find which pane, if any, was clicked.
    let (pane, pane_idx) = match hit_test_pane(ed, event.x, event.y) {
        Some(p) => p,
        None => return,
    };

    // Focus the clicked pane.
    ed.view_tree.focus(pane.id);

    let buf_id = match pane.buffer_id { Some(id) => id, None => return };

    // Header row (e.g. buffer/terminal header) above the text area.
    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + header_rows;
    if event.y < row_offset {
        // Click in tab bar or header — ignore for now.
        return;
    }

    // Check gutter area vs text area.
    if is_in_gutter(ed, &pane, buf_id, event.x) {
        dispatch_gutter_click(ed, surface, event.x, event.y, &pane, pane_idx, buf_id);
    } else {
        dispatch_text_click(ed, event, &pane, buf_id, count);
    }
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

    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + header_rows;

    let total_lines = ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);

    // Determine which gutter provider column was clicked.  Provider columns
    // start after the decoration prefix margin, exactly as in EditorView.
    let prefix_cols = {
        let pm = prefix_margin_width(ed, buf_id);
        if pm > 0 { pm + 1 } else { 0 }
    };
    let mut col_x = pane.x + prefix_cols as u16;
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
        .saturating_sub(header_rows as usize);
    let scroll_top = pane_scroll_top(ed, pane, buf_id, visible_lines);
    let abs_line = visible_row_to_abs_line(ed, buf_id, scroll_top, vis_row, visible_lines)
        .unwrap_or(scroll_top + vis_row)
        .min(total_lines.saturating_sub(1));

    // The breakpoint column's built-in behavior is native Rust — it must
    // not depend on init-registered Janet event handlers.  GUTTER_CLICKED
    // still fires for extension providers.
    if col_name == ":breakpoints"
        && let Some(path) = ed.buffers.get(buf_id)
            .and_then(|a| a.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
    {
        crate::kernel::debug::toggle_breakpoint_at(ed, &path, abs_line + 1);
    }

    ed.events.emit_typed(keys::events::GUTTER_CLICKED, GutterClickedPayload {
        column: col_name,
        line: abs_line.to_string(),
        buf: buf_id.to_string(),
    });
    ed.events.drain_and_dispatch();
}

/// Move cursor to the clicked text position and fire events.
///
/// Click count drives the GUI selection idioms: single places the cursor,
/// double selects the word, triple selects the line.
fn dispatch_text_click(
    ed: &mut Editor,
    event: &MouseEvent,
    pane: &Pane,
    buf_id: usize,
    click_count: u32,
) {
    let offset = match resolve_click_to_offset(ed, buf_id, pane, event.x, event.y) {
        Some(o) => o,
        None => return,
    };

    let word_or_line: Option<(usize, usize)> = match click_count {
        2 => ed.buffers.get(buf_id).and_then(|a| {
            let buf = a.lock().unwrap_or_else(|e| e.into_inner());
            word_range_at(&buf, offset)
        }),
        _ if click_count >= 3 => ed.buffers.get(buf_id).and_then(|a| {
            let buf = a.lock().unwrap_or_else(|e| e.into_inner());
            line_range_at(&buf, offset)
        }),
        _ => None,
    };

    match word_or_line {
        // Double / triple click: select the range, cursor resting on its
        // last character (selection_range counts the cursor byte itself).
        Some((start, end)) => {
            ed.selection = Some(crate::kernel::state::mode::Selection::char(start));
            if let Some(view) = ed.view_of_buffer_mut(buf_id) {
                view.set_cursor(end.saturating_sub(1).max(start));
            }
        }
        None => {
            if let Some(view) = ed.view_of_buffer_mut(buf_id) {
                view.set_cursor(offset);
            }
        }
    }

    // Clear selection unless a multi-click just established one or shift is
    // held (future: extend-selection on shift-click).
    if word_or_line.is_none()
        && ed.selection.is_some()
        && !event.modifiers.contains("shift") {
        ed.selection = None;
        ed.events.emit_typed(keys::events::SELECTION_CLEARED, EmptyPayload);
    }

    ed.events.emit_typed(keys::events::CURSOR_MOVED, CursorMovedPayload {
        buffer_id: buf_id.to_string(),
        cursor: offset.to_string(),
    });

    let (line, col) = ed.buffers.get(buf_id)
        .map(|a| {
            let buf = a.lock().unwrap_or_else(|e| e.into_inner());
            let (l, c) = buf.line_col_bytes(offset);
            (l.to_string(), c.to_string())
        })
        .unwrap_or_else(|| ("0".to_string(), "0".to_string()));
    ed.events.emit_typed(keys::events::TEXT_CLICKED, TextClickedPayload {
        buf: buf_id.to_string(),
        line,
        col,
        button: format!("{:?}", event.button),
    });
    ed.events.drain_and_dispatch();
}

/// Bounds of the word run around `offset` (alphanumeric + underscore).
/// Works in char space so multi-byte text can never slice mid-character.
pub(crate) fn word_range_at(buf: &crate::kernel::text_engine::Buffer, offset: usize) -> Option<(usize, usize)> {
    let offset = offset.min(buf.len());
    let line = buf.position_of(offset).line;
    let start = buf.line_start_offset(line)?;
    let end = buf.line_start_offset(line + 1).unwrap_or(buf.len());
    let text = buf.slice(start, end);
    let chars: Vec<char> = text.chars().collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';

    let cursor_char = text[..(offset - start).min(text.len())].chars().count();
    let under = chars.get(cursor_char).copied();
    let before = if cursor_char > 0 { chars.get(cursor_char - 1).copied() } else { None };
    if !under.map_or(false, is_word) && !before.map_or(false, is_word) {
        return None;
    }
    let mut lo = cursor_char;
    while lo > 0 && is_word(chars[lo - 1]) { lo -= 1; }
    let mut hi = cursor_char;
    while hi < chars.len() && is_word(chars[hi]) { hi += 1; }
    let byte_of = |n: usize| -> usize {
        text.char_indices().nth(n).map(|(i, _)| i).unwrap_or(text.len())
    };
    Some((start + byte_of(lo), start + byte_of(hi)))
}

/// Full-line bounds for triple click: from the line start through the
/// newline (exclusive of the next line).
fn line_range_at(buf: &crate::kernel::text_engine::Buffer, offset: usize) -> Option<(usize, usize)> {
    let line = buf.position_of(offset.min(buf.len())).line;
    let start = buf.line_start_offset(line)?;
    let end = buf.line_start_offset(line + 1).unwrap_or(buf.len());
    // Exclude the trailing newline itself.
    Some((start, (end.saturating_sub(1)).max(start)))
}

/// Convert screen coordinates to a buffer byte offset.
pub(crate) fn resolve_click_to_offset(
    ed: &Editor,
    buf_id: usize,
    pane: &Pane,
    click_x: u16,
    click_y: u16,
) -> Option<usize> {
    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + header_rows;
    if click_y < row_offset { return None; }
    let vis_row = (click_y - row_offset) as usize;

    let total_lines = ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);
    let gutter_width = compute_gutter_width(ed, buf_id, total_lines);
    let content_x = pane.x + gutter_width as u16;
    if click_x < content_x { return None; }
    let click_col = (click_x - content_x) as usize + pane.scroll_left;

    let visible_lines = (pane.height as usize)
        .saturating_sub(header_rows as usize);

    // The pane's viewport top must match what was painted this frame —
    // the stateless cursor-derived top would mis-map every click after a
    // manual or sticky scroll.
    let scroll_top = pane_scroll_top(ed, pane, buf_id, visible_lines);
    // Clicking the empty space under a short buffer lands on its last line.
    let abs_line = visible_row_to_abs_line(ed, buf_id, scroll_top, vis_row, visible_lines)
        .unwrap_or(scroll_top + vis_row)
        .min(total_lines.saturating_sub(1));

    let arc = ed.buffers.get(buf_id)?;
    let buf = arc.lock().ok()?;

    let line_start = buf.line_start_offset(abs_line)?;
    let line_end = buf.line_start_offset(abs_line + 1).unwrap_or(buf.len());
    let line_text = &buf.slice(line_start, line_end);

    // Walk graphemes accumulating display width, with tabs advancing to
    // their stops — the same placement walk the renderer uses, so clicks
    // land on the glyph they point at in tab-indented files.
    let tab_w = crate::kernel::render::frame::int_option(ed, "tab-width", 4);
    let mut display = 0usize;
    let mut byte_offset = line_start;
    for g in crate::kernel::text_engine::text_layout::grapheme_iter(line_text) {
        let w = crate::kernel::text_engine::text_layout::grapheme_cell_width(
            g, tab_w, display, &ed.font_config.glyph_widths);
        if display + w > click_col { break; }
        display += w;
        byte_offset += g.len();
    }
    // Clicks past end-of-line clamp to the newline, not past it — otherwise
    // the cursor lands on the next line.
    let clamp_end = if line_text.ends_with('\n') { line_end - 1 } else { line_end };
    Some(byte_offset.min(clamp_end))
}

/// Map a viewport row to its buffer line, mirroring EditorView's fold-aware
/// row walk.  A click on a fold header row lands on the fold's first line.
pub(crate) fn visible_row_to_abs_line(
    ed: &Editor,
    buf_id: usize,
    scroll_top: usize,
    target_row: usize,
    visible_lines: usize,
) -> Option<usize> {
    let view = ed.view_of_buffer(buf_id)?;
    let fold_map: std::collections::HashMap<usize, usize> =
        view.folds.iter().copied().collect();
    let arc = ed.buffers.get(buf_id)?;
    let buf = arc.lock().ok()?;
    let total = buf.line_count();
    let mut abs_line = scroll_top;
    let mut row = 0usize;
    while abs_line < total && row < visible_lines {
        if row == target_row { return Some(abs_line); }
        let line_start = buf.line_start_offset(abs_line).unwrap_or(buf.len());
        if let Some(&fold_end) = fold_map.get(&line_start) {
            abs_line += buf.position_of(fold_end).line.saturating_sub(abs_line).max(1);
        } else {
            abs_line += 1;
        }
        row += 1;
    }
    None
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
    if let Some(view) = ed.view_of_buffer_mut(buf_id) {
        view.set_cursor(cursor);
    }

    // Auto-scroll: keep the drag point visible at the viewport edge.
    if let Some(pane_mut) = ed.view_tree.pane_mut(pane.id) {
        let total_lines = ed.buffers.get(buf_id)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
            .unwrap_or(1);
        let visible = (pane_mut.height as usize).saturating_sub(1).max(1);
        let drag_line = ed.buffers.get(buf_id)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).position_of(cursor).line)
            .unwrap_or(0);
        let top = pane_mut.scroll_offset;
        if drag_line < top {
            pane_mut.scroll_offset = drag_line;
            pane_mut.scroll_pinned = true;
            pane_mut.scroll_buf = Some(buf_id);
        } else if drag_line >= top + visible && top + visible < total_lines {
            pane_mut.scroll_offset = (drag_line + 1 - visible).min(total_lines.saturating_sub(1));
            pane_mut.scroll_pinned = true;
            pane_mut.scroll_buf = Some(buf_id);
        }
    }

    // Real drag extents for listeners (selection sync, plugins).
    let (start_line, start_col, end_line, end_col) = {
        let (from, to) = (anchor.min(cursor), anchor.max(cursor));
        match ed.buffers.get(buf_id).map(|a| a.lock().unwrap_or_else(|e| e.into_inner())) {
            Some(buf) => {
                let p1 = buf.position_of(from);
                let p2 = buf.position_of(to);
                (
                    p1.line.to_string(), p1.column.to_string(),
                    p2.line.to_string(), p2.column.to_string(),
                )
            }
            None => ("0".into(), "0".into(), "0".into(), "0".into()),
        }
    };

    ed.events.emit_typed(keys::events::SELECTION_CHANGED, EmptyPayload);
    ed.events.emit_typed(keys::events::MOUSE_DRAGGED, MouseDraggedPayload {
        buf: buf_id.to_string(),
        start_line, start_col, end_line, end_col,
    });
    ed.events.drain_and_dispatch();
}

// ── Scroll ────────────────────────────────────────────────────────────────────

/// Wheel scrolling moves the viewport, never the cursor (GUI convention).
/// Positive `delta` reveals earlier lines.  The offset is pinned until the
/// cursor moves, so typing snaps the view back to the cursor.
fn dispatch_scroll(ed: &mut Editor, x: u16, y: u16, delta: i32) {
    let (pane_id, buf_id) = match hit_test_pane(ed, x, y) {
        Some((pane, _)) => (pane.id, pane.buffer_id),
        None => return,
    };
    let Some(buf_id) = buf_id else { return };

    let total_lines = ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);
    let cursor = ed.view_of_buffer(buf_id).map(|v| v.cursor.offset);
    let Some(pane) = ed.view_tree.pane_mut(pane_id) else { return };
    let max_top = total_lines.saturating_sub(1);
    let new_top = (pane.scroll_offset as i64 - delta as i64).clamp(0, max_top as i64) as usize;
    pane.scroll_offset = new_top;
    pane.scroll_pinned = true;
    pane.scroll_buf = Some(buf_id);
    pane.scroll_cursor = cursor;
}

/// Horizontal wheel/trackpad pan of the pane under the pointer.  Sign mirrors
/// the vertical wheel: positive `delta` reveals earlier columns.  Pinned until
/// the cursor moves, like the vertical scroll.
fn dispatch_scroll_x(ed: &mut Editor, x: u16, y: u16, delta: i32) {
    let (pane_id, buf_id) = match hit_test_pane(ed, x, y) {
        Some((pane, _)) => (pane.id, pane.buffer_id),
        None => return,
    };
    let Some(buf_id) = buf_id else { return };
    let cursor = ed.view_of_buffer(buf_id).map(|v| v.cursor.offset);
    let max_left = max_visible_cells(ed, buf_id) as usize;
    let Some(pane) = ed.view_tree.pane_mut(pane_id) else { return };
    let new_left = (pane.scroll_left as i64 - delta as i64).clamp(0, max_left as i64) as usize;
    pane.scroll_left = new_left;
    pane.scroll_left_pinned = true;
    pane.scroll_buf = Some(buf_id);
    pane.scroll_cursor = cursor;
}

/// Widest visible line in display cells — the horizontal pan bound.  Shared
/// with the render path via `frame::horizontal_extent`.
fn max_visible_cells(ed: &Editor, buf_id: usize) -> u16 {
    crate::kernel::render::frame::horizontal_extent(ed, buf_id).min(u16::MAX as usize) as u16
}

// ── Release ───────────────────────────────────────────────────────────────────

fn dispatch_release(ed: &mut Editor) {
    ed.mouse_state.pressed = false;
    ed.mouse_state.drag_active = false;
}

// ── Hit testing ───────────────────────────────────────────────────────────────

/// Find the pane at `(x, y)` screen coordinates.
///
/// Stacked tab panes share the same area; only the focused one is visible
/// and clickable.
pub(crate) fn hit_test_pane(ed: &Editor, x: u16, y: u16) -> Option<(Pane, usize)> {
    let focused = ed.view_tree.focused_window();
    for (i, pane) in ed.view_tree.panes().iter().enumerate() {
        if pane.layout == crate::kernel::render::view_tree::LayoutConstraint::Tab
            && focused != Some(pane.id) {
            continue;
        }
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
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);
    let gutter_width = compute_gutter_width(ed, buf_id, total_lines);
    click_x < pane.x + gutter_width as u16
}

fn compute_gutter_width(ed: &Editor, buf_id: usize, total_lines: usize) -> usize {
    crate::kernel::render::frame::gutter_cols(ed, buf_id, total_lines)
}

fn gutter_header_rows(ed: &Editor, buf_id: usize) -> u16 {
    if ed.buffers.get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).header_line.is_some())
        .unwrap_or(false)
    { 1 } else { 0 }
}
