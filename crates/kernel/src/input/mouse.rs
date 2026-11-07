//! Mouse event dispatch — resolves click/scroll coordinates to editor elements.
//!
//! Routes mouse events to the correct editor region:
//! - Gutter clicks → `GUTTER_CLICKED` event
//! - Text area clicks → cursor positioning
//! - Drag → selection extension
//! - Scroll → viewport scrolling (cursor untouched)
//! - Release → state clean-up

use crate::event::keys;
use crate::event::payload::*;
use crate::input::event::MouseButton;
use crate::input::event::MouseEvent;
use crate::render::decorations::prefix_margin_width;
use crate::render::frame::pane_scroll_top;
use crate::render::gutter::GutterRegistry;
use crate::render::surface::Surface;
use crate::render::view_tree::Pane;
use crate::state::{Editor, MouseEventKind};

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
        MouseEventKind::Move => dispatch_move(ed, surface, event),
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
        Some((at, x, y)) if at.elapsed().as_millis() <= 500 && x == event.x && y == event.y => {
            ed.mouse_state.click_count.min(2) + 1
        }
        _ => 1,
    };
    ed.mouse_state.last_click = Some((now, event.x, event.y));
    ed.mouse_state.click_count = count;

    // Any click is an implicit popup dismissal — and must precede the cursor
    // move, or a later accept would insert at the freshly clicked offset.
    crate::completion::dismiss(ed);

    // Store press state.
    ed.mouse_state.pressed = true;
    ed.mouse_state.button = event.button.clone();
    ed.mouse_state.press_x = event.x;
    ed.mouse_state.press_y = event.y;
    ed.mouse_state.last_x = event.x;
    ed.mouse_state.last_y = event.y;
    ed.mouse_state.drag_active = false;
    ed.mouse_state.press_offset = None;

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

    let buf_id = match pane.buffer_id {
        Some(id) => id,
        None => return,
    };

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
    if ed.gutter.providers.is_empty() {
        return;
    }

    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + header_rows;

    let total_lines = ed
        .buffers
        .get(buf_id)
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
    let col_name = match clicked_provider {
        Some(n) => n,
        None => return,
    };

    if click_y < row_offset {
        return;
    }
    let vis_row = (click_y - row_offset) as usize;
    let visible_lines = (pane.height as usize).saturating_sub(header_rows as usize);
    let scroll_top = pane_scroll_top(ed, pane, buf_id, visible_lines);
    let abs_line = visible_row_to_abs_line(ed, buf_id, scroll_top, vis_row, visible_lines)
        .unwrap_or(scroll_top + vis_row)
        .min(total_lines.saturating_sub(1));

    // The breakpoint column's built-in behavior is native Rust — it must
    // not depend on init-registered Janet event handlers.  GUTTER_CLICKED
    // still fires for extension providers.
    if ed.db.console_source(buf_id).is_some()
        && let Some(start) = sql_statement_start_for_line(ed, buf_id, abs_line)
    {
        if let Some(view) = ed.view_of_buffer_mut(buf_id) {
            view.set_cursor(start);
        }
        let _ = crate::command::execute_command(
            ed,
            "db-execute-current",
            &std::collections::HashMap::new(),
        );
    } else if col_name == ":breakpoints"
        && let Some(path) = ed
            .buffers
            .get(buf_id)
            .and_then(|a| a.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
    {
        crate::debug::toggle_breakpoint_at(ed, &path, abs_line + 1);
    }

    ed.events.emit_typed(
        keys::events::GUTTER_CLICKED,
        GutterClickedPayload {
            column: col_name,
            line: abs_line.to_string(),
            buf: buf_id.to_string(),
        },
    );
    crate::script_runtime::dispatch_events(ed);
}

fn sql_statement_start_for_line(ed: &Editor, buf_id: usize, abs_line: usize) -> Option<usize> {
    let arc = ed.buffers.get(buf_id)?;
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let text = buf.slice(0, buf.len());
    for span in db::split_statements(&text) {
        let code_start =
            crate::render::editor_view::first_sql_code_offset(&text, span.start, span.end);
        let start_line = buf.position_of(code_start).line;
        let end_line = buf.position_of(span.end.saturating_sub(1).max(code_start)).line;
        if abs_line >= start_line && abs_line <= end_line {
            return Some(code_start);
        }
    }
    None
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

    // Freeze the selection anchor now, in buffer space: later drag events
    // must not re-derive it from the press cell, whose line changes as soon
    // as the viewport scrolls to extend the selection.
    ed.mouse_state.press_offset = Some((buf_id, word_or_line.map_or(offset, |(start, _)| start)));

    match word_or_line {
        // Double / triple click: select the range, cursor resting on its
        // last character (selection_range counts the cursor byte itself).
        Some((start, end)) => {
            ed.selection = Some(crate::state::mode::Selection::char(start));
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
    if word_or_line.is_none() && ed.selection.is_some() && !event.modifiers.contains("shift") {
        ed.selection = None;
        ed.events
            .emit_typed(keys::events::SELECTION_CLEARED, EmptyPayload);
    }

    ed.events.emit_typed(
        keys::events::CURSOR_MOVED,
        CursorMovedPayload {
            buffer_id: buf_id.to_string(),
            cursor: offset.to_string(),
        },
    );

    let (line, col) = ed
        .buffers
        .get(buf_id)
        .map(|a| {
            let buf = a.lock().unwrap_or_else(|e| e.into_inner());
            let (l, c) = buf.line_col_bytes(offset);
            (l.to_string(), c.to_string())
        })
        .unwrap_or_else(|| ("0".to_string(), "0".to_string()));
    ed.events.emit_typed(
        keys::events::TEXT_CLICKED,
        TextClickedPayload {
            buf: buf_id.to_string(),
            line,
            col,
            button: format!("{:?}", event.button),
        },
    );

    // ctrl/cmd+click jumps to the definition of the symbol under the cursor,
    // mirroring the goto-definition command (tree-sitter index, LSP fallback).
    if event.modifiers.contains("ctrl") {
        let _ = crate::command::execute_command(
            ed,
            "goto-definition",
            &std::collections::HashMap::new(),
        );
    }

    crate::script_runtime::dispatch_events(ed);
}

/// Bounds of the word run around `offset` (alphanumeric + underscore).
/// Works in char space so multi-byte text can never slice mid-character.
pub fn word_range_at(buf: &crate::text_engine::Buffer, offset: usize) -> Option<(usize, usize)> {
    let offset = offset.min(buf.len());
    let line = buf.position_of(offset).line;
    let start = buf.line_start_offset(line)?;
    let end = buf.line_start_offset(line + 1).unwrap_or(buf.len());
    let text = buf.slice(start, end);
    let chars: Vec<char> = text.chars().collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';

    let cursor_char = text[..(offset - start).min(text.len())].chars().count();
    let under = chars.get(cursor_char).copied();
    let before = if cursor_char > 0 {
        chars.get(cursor_char - 1).copied()
    } else {
        None
    };
    if !under.is_some_and(is_word) && !before.is_some_and(is_word) {
        return None;
    }
    let mut lo = cursor_char;
    while lo > 0 && is_word(chars[lo - 1]) {
        lo -= 1;
    }
    let mut hi = cursor_char;
    while hi < chars.len() && is_word(chars[hi]) {
        hi += 1;
    }
    let byte_of = |n: usize| -> usize {
        text.char_indices()
            .nth(n)
            .map(|(i, _)| i)
            .unwrap_or(text.len())
    };
    Some((start + byte_of(lo), start + byte_of(hi)))
}

/// Full-line bounds for triple click: from the line start through the
/// newline (exclusive of the next line).
fn line_range_at(buf: &crate::text_engine::Buffer, offset: usize) -> Option<(usize, usize)> {
    let line = buf.position_of(offset.min(buf.len())).line;
    let start = buf.line_start_offset(line)?;
    let end = buf.line_start_offset(line + 1).unwrap_or(buf.len());
    // Exclude the trailing newline itself.
    Some((start, (end.saturating_sub(1)).max(start)))
}

/// Convert screen coordinates to a buffer byte offset.
pub fn resolve_click_to_offset(
    ed: &Editor,
    buf_id: usize,
    pane: &Pane,
    click_x: u16,
    click_y: u16,
) -> Option<usize> {
    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + header_rows;
    if click_y < row_offset {
        return None;
    }
    let vis_row = (click_y - row_offset) as usize;

    let total_lines = ed
        .buffers
        .get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);
    let gutter_width = compute_gutter_width(ed, buf_id, total_lines);
    let content_x = pane.x + gutter_width as u16;
    if click_x < content_x {
        return None;
    }
    let click_col = (click_x - content_x) as usize + pane.scroll_left;

    let visible_lines = (pane.height as usize).saturating_sub(header_rows as usize);

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
    let tab_w = crate::render::frame::int_option(ed, "tab-width", 4);
    let trimmed = line_text.strip_suffix('\n').unwrap_or(line_text);
    let layout = ed.row_layouts.get_or_build(
        (buf_id, line_start, tab_w),
        trimmed,
        buf.revision(),
        tab_w,
        &ed.font_config.glyph_widths,
    );
    if layout.bidi() {
        for i in 0..layout.widths.len() {
            let col = layout.cols[i] as usize;
            let w = (layout.widths[i] as usize).max(1);
            if click_col >= col && click_col < col + w {
                return Some(line_start + layout.offsets[i] as usize);
            }
        }
    }
    let mut display = 0usize;
    let mut byte_offset = line_start;
    for g in crate::text_engine::text_layout::grapheme_iter(line_text) {
        let w = crate::text_engine::text_layout::grapheme_cell_width(
            g,
            tab_w,
            display,
            &ed.font_config.glyph_widths,
        );
        if display + w > click_col {
            break;
        }
        display += w;
        byte_offset += g.len();
    }
    // Clicks past end-of-line clamp to the newline, not past it — otherwise
    // the cursor lands on the next line.
    let clamp_end = if line_text.ends_with('\n') {
        line_end - 1
    } else {
        line_end
    };
    Some(byte_offset.min(clamp_end))
}

/// Map a viewport row to its buffer line, mirroring EditorView's fold-aware
/// row walk.  A click on a fold header row lands on the fold's first line.
pub fn visible_row_to_abs_line(
    ed: &Editor,
    buf_id: usize,
    scroll_top: usize,
    target_row: usize,
    visible_lines: usize,
) -> Option<usize> {
    let view = ed.view_of_buffer(buf_id)?;
    let fold_map: std::collections::HashMap<usize, usize> = view.folds.iter().copied().collect();
    let arc = ed.buffers.get(buf_id)?;
    let buf = arc.lock().ok()?;
    let total = buf.line_count();
    let mut abs_line = scroll_top;
    let mut row = 0usize;
    while abs_line < total && row < visible_lines {
        if row == target_row {
            return Some(abs_line);
        }
        let line_start = buf.line_start_offset(abs_line).unwrap_or(buf.len());
        if let Some(&fold_end) = fold_map.get(&line_start) {
            abs_line += buf
                .position_of(fold_end)
                .line
                .saturating_sub(abs_line)
                .max(1);
        } else {
            abs_line += 1;
        }
        row += 1;
    }
    None
}

// ── Drag ──────────────────────────────────────────────────────────────────────

fn dispatch_drag(ed: &mut Editor, _surface: &Surface, event: &MouseEvent) {
    if !ed.mouse_state.pressed {
        return;
    }
    ed.mouse_state.drag_active = true;
    ed.mouse_state.last_x = event.x;
    ed.mouse_state.last_y = event.y;

    let (pane, _) = match hit_test_pane(ed, event.x, event.y) {
        Some(p) => p,
        None => return,
    };
    let buf_id = match pane.buffer_id {
        Some(id) => id,
        None => return,
    };

    let Some(cursor) = resolve_click_to_offset(ed, buf_id, &pane, event.x, event.y) else {
        return;
    };
    // Anchor in buffer space, captured at press time: the press cell drifts
    // to another line whenever the viewport scrolls under a held drag.
    let anchor = match ed.mouse_state.press_offset {
        Some((buf, offset)) if buf == buf_id => offset,
        Some(_) => return,
        None => {
            match resolve_click_to_offset(
                ed,
                buf_id,
                &pane,
                ed.mouse_state.press_x,
                ed.mouse_state.press_y,
            ) {
                Some(p) => p,
                None => return,
            }
        }
    };

    ed.selection = Some(crate::state::mode::Selection::char(anchor));
    if let Some(view) = ed.view_of_buffer_mut(buf_id) {
        view.set_cursor(cursor);
    }

    // Auto-scroll: keep the drag point visible at the viewport edge.
    if let Some(pane_mut) = ed.view_tree.pane_mut(pane.id) {
        let total_lines = ed
            .buffers
            .get(buf_id)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
            .unwrap_or(1);
        let visible = (pane_mut.height as usize).saturating_sub(1).max(1);
        let drag_line = ed
            .buffers
            .get(buf_id)
            .map(|a| {
                a.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .position_of(cursor)
                    .line
            })
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
        match ed
            .buffers
            .get(buf_id)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()))
        {
            Some(buf) => {
                let p1 = buf.position_of(from);
                let p2 = buf.position_of(to);
                (
                    p1.line.to_string(),
                    p1.column.to_string(),
                    p2.line.to_string(),
                    p2.column.to_string(),
                )
            }
            None => ("0".into(), "0".into(), "0".into(), "0".into()),
        }
    };

    ed.events
        .emit_typed(keys::events::SELECTION_CHANGED, EmptyPayload);
    ed.events.emit_typed(
        keys::events::MOUSE_DRAGGED,
        MouseDraggedPayload {
            buf: buf_id.to_string(),
            start_line,
            start_col,
            end_line,
            end_col,
        },
    );
    crate::script_runtime::dispatch_events(ed);
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

    let total_lines = ed
        .buffers
        .get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);
    let cursor = ed.view_of_buffer(buf_id).map(|v| v.cursor.offset);
    let Some(pane) = ed.view_tree.pane_mut(pane_id) else {
        return;
    };
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
fn dispatch_move(ed: &mut Editor, _surface: &Surface, event: &MouseEvent) {
    if ed.mouse_state.pressed {
        return;
    }
    // Track pointer position for potential hover features; the gutter can
    // dim its breakpoint affordance while the pointer rests over it.
    ed.mouse_state.last_x = event.x;
    ed.mouse_state.last_y = event.y;

    // Update hover target over the breakpoint column.
    ed.mouse_state.breakpoint_hover = breakpoints_column_hover(ed, event.x, event.y);
    ed.frame_dirty = true;
}

fn breakpoints_column_hover(ed: &Editor, x: u16, y: u16) -> Option<(String, usize)> {
    let (pane, _pane_idx) = hit_test_pane(ed, x, y)?;
    let buf_id = pane.buffer_id?;
    let header_rows: u16 = gutter_header_rows(ed, buf_id);
    let row_offset = pane.y + header_rows;
    if y < row_offset {
        return None;
    }
    let total_lines = ed
        .buffers
        .get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);

    // Determine if the cursor falls inside the `:breakpoints` provider column.
    let prefix_cols = {
        let pm = crate::render::decorations::prefix_margin_width(ed, buf_id);
        if pm > 0 { pm + 1 } else { 0 }
    };
    let mut col_x = pane.x + prefix_cols as u16;
    for provider in &ed.gutter.providers {
        let col_w = GutterRegistry::provider_width(provider.as_ref(), total_lines) as u16;
        if provider.name() == ":breakpoints" {
            if x >= col_x && x < col_x + col_w {
                let vis_row = (y - row_offset) as usize;
                let visible_lines = (pane.height as usize).saturating_sub(header_rows as usize);
                let scroll_top = crate::render::frame::pane_scroll_top(ed, &pane, buf_id, visible_lines);
                if let Some(abs_line) = visible_row_to_abs_line(ed, buf_id, scroll_top, vis_row, visible_lines) {
                    let path = ed
                        .buffers
                        .get(buf_id)
                        .and_then(|a| a.lock().unwrap_or_else(|e| e.into_inner()).path.clone())?;
                    return Some((path, abs_line));
                }
                break;
            }
            break;
        }
        col_x += col_w;
    }
    None
}

fn dispatch_scroll_x(ed: &mut Editor, x: u16, y: u16, delta: i32) {
    let (pane_id, buf_id) = match hit_test_pane(ed, x, y) {
        Some((pane, _)) => (pane.id, pane.buffer_id),
        None => return,
    };
    let Some(buf_id) = buf_id else { return };
    let cursor = ed.view_of_buffer(buf_id).map(|v| v.cursor.offset);
    let max_left = max_visible_cells(ed, buf_id) as usize;
    let Some(pane) = ed.view_tree.pane_mut(pane_id) else {
        return;
    };
    let new_left = (pane.scroll_left as i64 - delta as i64).clamp(0, max_left as i64) as usize;
    pane.scroll_left = new_left;
    pane.scroll_left_pinned = true;
    pane.scroll_buf = Some(buf_id);
    pane.scroll_cursor = cursor;
}

/// Widest visible line in display cells — the horizontal pan bound.  Shared
/// with the render path via `frame::horizontal_extent`.
fn max_visible_cells(ed: &Editor, buf_id: usize) -> u16 {
    crate::render::frame::horizontal_extent(ed, buf_id).min(u16::MAX as usize) as u16
}

// ── Release ───────────────────────────────────────────────────────────────────

fn dispatch_release(ed: &mut Editor) {
    ed.mouse_state.pressed = false;
    ed.mouse_state.drag_active = false;
}

// ── Hit testing ───────────────────────────────────────────────────────────────

/// Find the pane at `(x, y)` screen coordinates.
///
/// Panes paint in registration order, so the last match is the topmost one; a
/// pane showing no buffer must never swallow a click or a wheel event meant
/// for the visible buffer.  Stacked tab panes share an area and only the
/// focused one is visible and clickable.
pub fn hit_test_pane(ed: &Editor, x: u16, y: u16) -> Option<(Pane, usize)> {
    let focused = ed.view_tree.focused_window();
    for (i, pane) in ed.view_tree.panes().iter().enumerate().rev() {
        if pane.buffer_id.is_none()
            || (pane.layout == crate::render::view_tree::LayoutConstraint::Tab
                && focused != Some(pane.id))
        {
            continue;
        }
        if x >= pane.x && x < pane.x + pane.width && y >= pane.y && y < pane.y + pane.height {
            return Some((pane.clone(), i));
        }
    }
    None
}

/// Returns `true` when the x-coordinate falls within the gutter region.
fn is_in_gutter(ed: &Editor, pane: &Pane, buf_id: usize, click_x: u16) -> bool {
    let total_lines = ed
        .buffers
        .get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);
    let gutter_width = compute_gutter_width(ed, buf_id, total_lines);
    click_x < pane.x + gutter_width as u16
}

fn compute_gutter_width(ed: &Editor, buf_id: usize, total_lines: usize) -> usize {
    crate::render::frame::gutter_cols(ed, buf_id, total_lines)
}

fn gutter_header_rows(ed: &Editor, buf_id: usize) -> u16 {
    if ed
        .buffers
        .get(buf_id)
        .map(|a| {
            a.lock()
                .unwrap_or_else(|e| e.into_inner())
                .header_line
                .is_some()
        })
        .unwrap_or(false)
    {
        1
    } else {
        0
    }
}
