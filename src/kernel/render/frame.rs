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

/// Compute scroll state from pre-extracted buffer data (no locking).
/// Used by EditorView (which already holds the buffer lock) and by
/// callers that have already extracted text/line-count themselves.
pub fn compute_scroll_state_raw(
    text: &str,
    total_lines: usize,
    cursor_offset: usize,
    visible_lines: usize,
    scrolloff: usize,
    glyph_widths: &std::collections::HashMap<char, u8>,
) -> ScrollState {
    let safe = cursor_offset.min(text.len());
    let cursor_line = text[..safe]
        .chars()
        .filter(|&c| c == '\n')
        .count()
        .min(total_lines.saturating_sub(1));

    let ideal_top = cursor_line.saturating_sub(scrolloff);
    let window_bottom = cursor_line + scrolloff;
    let scroll_top = if window_bottom >= visible_lines {
        ideal_top.max(window_bottom.saturating_sub(visible_lines).saturating_add(1))
    } else {
        ideal_top
    };
    let scroll_top = scroll_top.min(total_lines.saturating_sub(1));

    let cursor_vis_row = cursor_line.saturating_sub(scroll_top);
    let text_before = &text[..safe];
    let last_line = text_before
        .rfind('\n')
        .map(|p| &text_before[p + 1..])
        .unwrap_or(text_before);
    let cursor_col: usize = last_line.chars()
        .map(|c| cell_width(c, glyph_widths))
        .sum();

    ScrollState { scroll_top, cursor_vis_row, cursor_col, cursor_line }
}

/// Compute the scroll state for the focused buffer, acquiring the lock internally.
/// Do NOT call while holding the buffer lock.
pub fn compute_scroll_state(editor: &Editor, buf_id: usize, visible_lines: usize) -> ScrollState {
    let scrolloff = int_option(editor, "scrolloff", 0);
    let (text, total_lines) = editor.buffers.get(buf_id)
        .map(|arc| {
            let buf = arc.lock().unwrap();
            (buf.slice(0, buf.len()), buf.line_count())
        })
        .unwrap_or_default();
    let cursor_offset = editor.views.get(&buf_id)
        .map(|v| v.cursor.offset)
        .unwrap_or(0);
    compute_scroll_state_raw(&text, total_lines, cursor_offset, visible_lines,
                             scrolloff, &editor.font_config.glyph_widths)
}

/// Populate `surface` from editor state by traversing the ViewTree.
///
/// When `editor_pane_only` is true (GUI mode), the tab bar reservation and
/// status bar are omitted — those are rendered as native egui panels instead.
pub fn render_frame(editor: &Editor, surface: &mut Surface, editor_pane_only: bool) {
    surface.clear();

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
        let cursor_offset = editor.views.get(&buf_id).map(|v| v.cursor.offset).unwrap_or(0);
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
