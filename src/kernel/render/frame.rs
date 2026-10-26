//! render_frame — traverses the ViewTree and dispatches to each pane's View.
//!
//! The GUI editor pane calls render_frame to populate a Surface, then paints
//! the Surface (CPU or GPU path).  Window chrome (tabs, status bar, dialogs)
//! is native GUI and rendered outside this pipeline.

use crate::kernel::state::Editor;
use crate::kernel::render::host::{FrameHost, RenderHost, ViewportHost};
use crate::kernel::render::surface::{Style, Surface};
use crate::kernel::render::view::{Rect, RenderCtx, View};
use crate::kernel::render::view_tree::Pane;
use crate::kernel::render::editor_view::EditorView;
use super::decorations::render_overlays;

/// Return the display-column width of a character, consulting the glyph-width
/// override table before falling back to unicode_width.
pub fn cell_width(ch: char, glyph_widths: &std::collections::HashMap<char, u8>) -> usize {
    if let Some(&w) = glyph_widths.get(&ch) {
        return w as usize;
    }
    unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1)
}

pub fn bool_option(editor: &dyn RenderHost, name: &str) -> bool {
    matches!(editor.option(name), Some("true" | "1"))
}

pub fn int_option(editor: &dyn RenderHost, name: &str, default: usize) -> usize {
    editor.option(name)
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Scroll and cursor state for a buffer.
pub struct ScrollState {
    pub scroll_top: usize,
    pub cursor_vis_row: usize,
    pub cursor_col: usize,
    pub cursor_line: usize,
}

/// Scroll state from line-index data — no text scan: the cursor's line
/// comes from the line index and the cell column is summed over the
/// cursor's line only (docs/core-redesign.md §3.3/§9).
pub fn scroll_state_from_lines(
    cursor_line: usize,
    cursor_col: usize,
    total_lines: usize,
    visible_lines: usize,
    scrolloff: usize,
) -> ScrollState {
    let cursor_line = cursor_line.min(total_lines.saturating_sub(1));
    let ideal_top = cursor_line.saturating_sub(scrolloff);
    let window_bottom = cursor_line + scrolloff;
    let scroll_top = if window_bottom >= visible_lines {
        ideal_top.max(window_bottom.saturating_sub(visible_lines).saturating_add(1))
    } else {
        ideal_top
    };
    let scroll_top = scroll_top.min(total_lines.saturating_sub(1));
    let cursor_vis_row = cursor_line.saturating_sub(scroll_top);
    ScrollState { scroll_top, cursor_vis_row, cursor_col, cursor_line }
}

/// (cursor_line, cursor_col in cells) for a cursor offset — O(log n) plus
/// one line of cell math.
pub fn cursor_metrics(
    buf: &crate::kernel::text_engine::Buffer,
    cursor_offset: usize,
    tab_width: usize,
    glyph_widths: &std::collections::HashMap<char, u8>,
) -> (usize, usize) {
    let total_lines = buf.line_count();
    let cursor_offset = cursor_offset.min(buf.len());
    let cursor_line = buf.position_of(cursor_offset).line.min(total_lines.saturating_sub(1));
    let line_start = buf.line_start_offset(cursor_line).unwrap_or(0);
    // Exactly the cursor's line — never the rest of the document.
    let line_end = buf.line_start_offset(cursor_line + 1).unwrap_or(buf.len());
    // The row layout is bidi-aware (F-15): visual_col_of maps the logical
    // byte to the screen column, and is identical to the logical walk for
    // LTR rows.
    let line_text = buf.slice(line_start, line_end);
    let layout = crate::kernel::render::row_layout::RowLayout::build(
        &line_text, 0, tab_width, glyph_widths);
    let cursor_col = layout.visual_col_of(cursor_offset.saturating_sub(line_start));
    (cursor_line, cursor_col)
}

/// Scroll state for a buffer's cursor: O(log n) + one line of cell math.
/// Caller must not hold another borrow of the view.
pub fn scroll_state_for_buffer(
    buf: &crate::kernel::text_engine::Buffer,
    cursor_offset: usize,
    visible_lines: usize,
    scrolloff: usize,
    tab_width: usize,
    glyph_widths: &std::collections::HashMap<char, u8>,
) -> ScrollState {
    let (cursor_line, cursor_col) = cursor_metrics(buf, cursor_offset, tab_width, glyph_widths);
    scroll_state_from_lines(cursor_line, cursor_col, buf.line_count(), visible_lines, scrolloff)
}

/// Compute the scroll state for the focused buffer, acquiring the lock internally.
/// Do NOT call while holding the buffer lock.
pub fn compute_scroll_state(editor: &dyn RenderHost, buf_id: usize, visible_lines: usize) -> ScrollState {
    let scrolloff = int_option(editor, "scrolloff", 0);
    let tab_width = int_option(editor, "tab-width", 4);
    let cursor_offset = editor.view_of_buffer(buf_id)
        .map(|v| v.cursor.offset)
        .unwrap_or(0);
    match editor.buffer(buf_id) {
        Some(arc) => {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            scroll_state_for_buffer(&buf, cursor_offset, visible_lines, scrolloff, tab_width,
                                    editor.glyph_widths())
        }
        None => scroll_state_from_lines(0, 0, 1, visible_lines, scrolloff),
    }
}

/// A pane's effective viewport top line, when the pane has synced state.
///
/// `None` means the caller must derive the top itself — importantly WITHOUT
/// taking the buffer lock, because `EditorView::render` calls this while
/// already holding it.
pub fn pane_scroll_top_synced(pane: &Pane, buf_id: usize) -> Option<usize> {
    (pane.scroll_buf == Some(buf_id) || pane.scroll_pinned).then_some(pane.scroll_offset)
}

/// A pane's effective viewport top line for callers that do not hold the
/// buffer lock.  Falls back to the stateless cursor-derived top before the
/// first sync.
pub fn pane_scroll_top(ed: &dyn RenderHost, pane: &Pane, buf_id: usize, visible_lines: usize) -> usize {
    pane_scroll_top_synced(pane, buf_id)
        .unwrap_or_else(|| compute_scroll_state(ed, buf_id, visible_lines).scroll_top)
}

/// Scroll state against a pane's effective viewport top.
pub fn pane_scroll_state(ed: &dyn RenderHost, pane: &Pane, buf_id: usize, visible_lines: usize) -> ScrollState {
    let scroll_top = pane_scroll_top(ed, pane, buf_id, visible_lines);
    let cursor_offset = ed.view_of_buffer(buf_id)
        .map(|v| v.cursor.offset)
        .unwrap_or(0);
    match ed.buffer(buf_id) {
        Some(arc) => {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let (cursor_line, cursor_col) = cursor_metrics(&buf, cursor_offset,
                int_option(ed, "tab-width", 4), ed.glyph_widths());
            ScrollState {
                scroll_top,
                cursor_vis_row: cursor_line.saturating_sub(scroll_top),
                cursor_col,
                cursor_line,
            }
        }
        None => scroll_state_from_lines(0, 0, 1, visible_lines, int_option(ed, "scrolloff", 0)),
    }
}

/// A pane's effective first visible column, when the pane has synced state.
pub fn pane_scroll_left_synced(pane: &Pane, buf_id: usize) -> Option<usize> {
    (pane.scroll_buf == Some(buf_id) || pane.scroll_left_pinned).then_some(pane.scroll_left)
}

/// A pane's effective first visible column for callers that do not hold the
/// buffer lock.  Falls back to 0 (line start) before the first sync.
pub fn pane_scroll_left(pane: &Pane, buf_id: usize) -> usize {
    pane_scroll_left_synced(pane, buf_id).unwrap_or(0)
}

/// Width in cells of the buffer's fixed gutter (line numbers, fold signs,
/// diff marks) including any line-prefix margin.
pub fn gutter_cols(ed: &dyn RenderHost, buf_id: usize, total_lines: usize) -> usize {
    let prefix_margin = crate::kernel::render::decorations::prefix_margin_width(ed, buf_id);
    let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };
    prefix_cols + ed.gutter_width(total_lines)
}

/// Width in cells of the text area — pane width minus the fixed gutter.
pub(crate) fn content_width(ed: &dyn RenderHost, pane_width: u16, buf_id: usize, total_lines: usize) -> usize {
    (pane_width as usize).saturating_sub(gutter_cols(ed, buf_id, total_lines))
}

/// Widest currently-visible line in display cells — the horizontal pan bound
/// and the extent the scrollbar reflects.  Bounded by the viewport height,
/// never a whole-file scan.
pub fn horizontal_extent(ed: &dyn RenderHost, buf_id: usize) -> usize {
    let Some(arc) = ed.buffer(buf_id) else { return 0 };
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let total_lines = buf.line_count();
    let pane = ed.panes().iter().find(|p| p.buffer_id == Some(buf_id));
    let visible = pane.map(|p| (p.height as usize).saturating_sub(1)).unwrap_or(1).max(1);
    let top = pane.map(|p| p.scroll_offset).unwrap_or(0).min(total_lines.saturating_sub(1));
    let tab_w = int_option(ed, "tab-width", 4);
    let mut max_cells = 0usize;
    for line in top..(top + visible).min(total_lines) {
        if let Some(text) = buf.line(line) {
            max_cells = max_cells.max(
                crate::kernel::render::row_layout::RowLayout::build(
                    &text, 0, tab_w, ed.glyph_widths()).cells());
        }
    }
    max_cells
}

/// Sticky cursor-following scroll, per pane.
///
/// Runs once per frame with `&mut Editor` before `render_frame`.
/// While the
/// cursor stays inside the viewport the top line does not move — clicking or
/// arrowing inside the view must never jump the text.  The viewport snaps by
/// `scrolloff` only when the cursor leaves it.  A manual scroll (wheel,
/// scrollbar, `window/set-scroll-top`) pins the offset until the cursor
/// moves; a buffer switch re-derives the viewport.
pub fn sync_viewport_scroll(host: &mut dyn ViewportHost) {
    let mut scrolloff = viewport_int_option(host, "scrolloff", 0);
    if scrolloff > 0 {
        // Cap like vim: a margin of half the viewport or more would
        // oscillate between the top and bottom snap rules.
        let tallest = host.panes().iter()
            .map(|p| p.height as usize).max().unwrap_or(1);
        scrolloff = scrolloff.min(tallest.saturating_sub(1) / 2);
    }

    for i in 0..host.panes().len() {
        let Some(m) = host.pane_metrics(i) else { continue };
        let total_lines = m.total_lines;
        let cursor_offset = m.cursor_offset;
        let cursor_line = m.cursor_line.min(total_lines.saturating_sub(1));
        let cursor_col = m.cursor_col;
        let visible = (m.height as usize).saturating_sub(1).max(1);
        let content_w = m.content_width;
        let h_margin = viewport_int_option(host, "sidescrolloff", 0)
            .min(content_w.saturating_sub(1) / 2);
        let buf_id = m.buffer_id;
        let pane = &mut host.panes_mut()[i];

        if pane.scroll_buf != Some(buf_id) {
            pane.scroll_buf = Some(buf_id);
            pane.scroll_cursor = Some(cursor_offset);
            pane.scroll_pinned = false;
            pane.scroll_left_pinned = false;
            // Derive both axes from the cursor so a freshly-shown buffer is
            // already scrolled to it.
            pane.scroll_left = follow_col(0, cursor_col, content_w, h_margin);
            pane.scroll_offset = follow_from_scratch(cursor_line, total_lines, visible, scrolloff);
            continue;
        }
        if pane.scroll_cursor != Some(cursor_offset) {
            pane.scroll_cursor = Some(cursor_offset);
            pane.scroll_pinned = false;
            pane.scroll_left_pinned = false;
        }
        if pane.scroll_pinned {
            pane.scroll_offset = pane.scroll_offset.min(total_lines.saturating_sub(1));
        } else {
            pane.scroll_offset = follow_line(pane.scroll_offset, cursor_line, visible, scrolloff)
                .min(total_lines.saturating_sub(1));
        }
        // Horizontal: the caret always stays inside the pane unless a manual
        // pan pinned the view (released when the cursor moves above).
        if !pane.scroll_left_pinned {
            pane.scroll_left = follow_col(pane.scroll_left, cursor_col, content_w, h_margin);
        }
    }

    // ── Linked panes (the two halves of a side-by-side diff) ────────────
    // Whichever half holds the focus drives; the other mirrors it on both
    // axes, so the two file versions stay line-for-line aligned.
    let source = host.focused_window().and_then(|f| {
        let p = host.window(f)?;
        p.scroll_link.map(|link| (f, link, p.scroll_offset, p.scroll_left))
    });
    if let Some((focused_id, link, top, left)) = source {
        for pane in host.panes_mut() {
            if pane.id == focused_id || pane.scroll_link != Some(link) { continue; }
            pane.scroll_offset = top;
            pane.scroll_left = left;
            // Pin so the cursor-follow rules never drag the mirror out of step.
            pane.scroll_pinned = true;
            pane.scroll_left_pinned = true;
            pane.scroll_buf = pane.buffer_id;
        }
    }
}

fn viewport_int_option(host: &dyn ViewportHost, name: &str, default: usize) -> usize {
    host.option(name).and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Viewport top for a cursor entering an unscrolled pane.
fn follow_from_scratch(cursor_line: usize, total_lines: usize, visible: usize, scrolloff: usize) -> usize {
    scroll_state_from_lines(cursor_line, 0, total_lines, visible, scrolloff).scroll_top
}

/// Keep `top` unless the cursor crosses a scrolloff margin at either edge.
fn follow_line(top: usize, cursor_line: usize, visible: usize, scrolloff: usize) -> usize {
    if cursor_line < top.saturating_add(scrolloff) {
        cursor_line.saturating_sub(scrolloff)
    } else if cursor_line + scrolloff >= top + visible.max(1) {
        cursor_line + scrolloff + 1 - visible
    } else {
        top
    }
}

/// Keep `left` unless the cursor crosses a margin at either edge (the
/// horizontal twin of `follow_line`).
fn follow_col(left: usize, col: usize, width: usize, margin: usize) -> usize {
    if width == 0 {
        return 0;
    }
    if col < left.saturating_add(margin) {
        col.saturating_sub(margin)
    } else if col + margin >= left + width {
        (col + margin + 1).saturating_sub(width)
    } else {
        left
    }
}

/// Clear `surface` and seed every cell with the frame background so cells no
/// view paints show the editor background, not `Style::default()`'s black.
pub fn prepare_frame(host: &dyn FrameHost, surface: &mut Surface) {
    surface.clear();
    surface.fill_style(Style {
        fg: host.theme_color("fg"),
        bg: host.theme_color("bg"),
        ..Default::default()
    });
}

/// Populate `surface` from editor state by traversing the ViewTree.
///
/// The surface contains editor pane content only — tab bar, status bar, and
/// dialogs are native GUI widgets rendered around it.
pub fn render_frame(editor: &Editor, surface: &mut Surface) {
    let host: &dyn FrameHost = editor;
    prepare_frame(host, surface);

    // ── Render each pane through its view ────────────────────────────────
    let focused = host.focused_window();
    for pane in host.panes() {
        let Some(buf_id) = pane.buffer_id else { continue };
        // Stacked tabs occupy the same area — only the visible one renders.
        if pane.layout == crate::kernel::render::view_tree::LayoutConstraint::Tab
            && focused != Some(pane.id) {
            continue;
        }

        let area = Rect::new(pane.x, pane.y, pane.width, pane.height);
        let ctx = RenderCtx { editor, area };

        EditorView { buf_id, pane_id: pane.id }.render(surface, &ctx);
    }

    // ── Overlays (z-ordered, above all pane content) ─────────────────────
    render_overlays(editor, surface);
}
