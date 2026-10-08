//! render_frame — traverses the ViewTree and dispatches to each pane's View.
//!
//! Both the TUI and GUI renderers call render_frame to populate a Surface,
//! then consume the Surface for output.

use crate::kernel::state::Editor;
use crate::kernel::render::surface::{Style, Surface};
use crate::kernel::render::view::{Rect, RenderCtx, View};
use crate::kernel::render::editor_view::EditorView;
use crate::kernel::render::terminal_view::TerminalView;
use super::status_and_popup::{render_status_bar, render_command_completion_popup};
use super::decorations::render_overlays;

/// Return the display-column width of a character, consulting the glyph-width
/// override table before falling back to unicode_width.
pub fn cell_width(ch: char, glyph_widths: &std::collections::HashMap<char, u8>) -> usize {
    if let Some(&w) = glyph_widths.get(&ch) {
        return w as usize;
    }
    unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1)
}

pub fn bool_option(editor: &Editor, name: &str) -> bool {
    matches!(editor.options.get(name).map(|s| s.as_str()), Some("true" | "1"))
}

pub fn int_option(editor: &Editor, name: &str, default: usize) -> usize {
    editor.options.get(name)
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

/// Scroll state for a buffer's cursor: O(log n) + one line of cell math.
/// Caller must not hold another borrow of the view.
pub fn scroll_state_for_buffer(
    buf: &crate::kernel::text_engine::Buffer,
    cursor_offset: usize,
    visible_lines: usize,
    scrolloff: usize,
    glyph_widths: &std::collections::HashMap<char, u8>,
) -> ScrollState {
    let total_lines = buf.line_count();
    let cursor_offset = cursor_offset.min(buf.len());
    let pos = buf.position_of(cursor_offset);
    let cursor_line = pos.line.min(total_lines.saturating_sub(1));
    let line_start = buf.line_start_offset(cursor_line).unwrap_or(0);
    // Exactly the cursor's line — never the rest of the document.
    let line_end = buf.line_start_offset(cursor_line + 1).unwrap_or(buf.len());
    let cursor_col = crate::kernel::text_engine::text_layout::cell_column(
        &buf.slice(line_start, line_end),
        cursor_offset.saturating_sub(line_start),
        4,
        glyph_widths,
    );
    scroll_state_from_lines(cursor_line, cursor_col, total_lines, visible_lines, scrolloff)
}

/// Compute the scroll state for the focused buffer, acquiring the lock internally.
/// Do NOT call while holding the buffer lock.
pub fn compute_scroll_state(editor: &Editor, buf_id: usize, visible_lines: usize) -> ScrollState {
    let scrolloff = int_option(editor, "scrolloff", 0);
    let cursor_offset = editor.view_of_buffer(buf_id)
        .map(|v| v.cursor.offset)
        .unwrap_or(0);
    match editor.buffers.get(buf_id) {
        Some(arc) => {
            let buf = arc.lock().unwrap();
            scroll_state_for_buffer(&buf, cursor_offset, visible_lines, scrolloff,
                                    &editor.font_config.glyph_widths)
        }
        None => scroll_state_from_lines(0, 0, 1, visible_lines, scrolloff),
    }
}

/// Populate `surface` from editor state by traversing the ViewTree.
///
/// When `editor_pane_only` is true (GUI mode), the tab bar reservation and
/// status bar are omitted — those are rendered as native egui panels instead.
pub fn render_frame(editor: &Editor, surface: &mut Surface, editor_pane_only: bool) {
    surface.clear();

    // Surface-based tab bar row — TUI only.  When editor_pane_only is true
    // (GUI mode) the tab bar is a native egui widget in GuiApp::update().
    let tab_bar_rows: u16 = if !editor_pane_only && editor.tab_bar_enabled { 1 } else { 0 };

    if !editor_pane_only && editor.tab_bar_enabled {
        let tb_style = Style {
            fg: editor.theme_color("status-fg"),
            bg: editor.theme_color("status-bg"),
            ..Default::default()
        };
        for x in 0..surface.width {
            surface.set_cell(x, 0, ' ', Some(tb_style));
        }
    }

    // ── Render each pane through its view ────────────────────────────────
    for pane in editor.view_tree.panes() {
        let Some(buf_id) = pane.buffer_id else { continue };

        let area = Rect::new(pane.x, pane.y + tab_bar_rows, pane.width, pane.height);
        let ctx = RenderCtx { editor, area };

        if crate::kernel::terminal::is_terminal(editor, buf_id) {
            TerminalView { buf_id }.render(surface, &ctx);
        } else {
            EditorView { buf_id, pane_id: pane.id }.render(surface, &ctx);
        }
    }

    // ── Overlays (z-ordered, above all pane content) ─────────────────────
    render_overlays(editor, surface);

    if editor_pane_only {
        return;
    }

    // ── Status bar (single global bar for the focused pane) ──────────────
    let focused = editor.view_tree.focused_window();
    let buffer_id = focused.and_then(|wid| editor.view_tree.buffer(wid));
    if let Some(buf_id) = buffer_id
        && let Some(arc) = editor.buffers.get(buf_id)
    {
        let buf = arc.lock().unwrap();
        let cursor_offset = editor.view_of_buffer(buf_id).map(|v| v.cursor.offset).unwrap_or(0);
        let is_terminal_view = crate::kernel::terminal::is_terminal(editor, buf_id);
        render_status_bar(editor, &*buf, buf_id, cursor_offset, surface, is_terminal_view);
    }

    // ── Command-mode completion popup ─────────────────────────────────────
    if editor.editor_mode.minibuffer.is_some()
        && editor.completion.visible
        && !editor.completion.items.is_empty()
    {
        render_command_completion_popup(editor, surface);
    }
}
