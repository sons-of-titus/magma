//! Status bar, terminal frame, and popup rendering.

use crate::kernel::text_engine::Buffer;
use crate::kernel::state::Editor;
use crate::kernel::render::surface::{Style, Surface};

/// Render the terminal buffer area with a dedicated look.
pub(super) fn render_terminal_frame(
    editor: &Editor,
    buf: &Buffer,
    surface: &mut Surface,
    visible_lines: usize,
) {
    let tf = editor.theme_color("terminal-fg");
    let t_style = Style { fg: tf, bg: (0, 0, 0), ..Default::default() };
    let total_lines = buf.line_count();
    let text = buf.slice(0, buf.len());
    let scroll_top = total_lines.saturating_sub(visible_lines);
    let max_visible = (total_lines - scroll_top).min(visible_lines);
    for i in 0..max_visible {
        let abs_line = scroll_top + i;
        let line_start = buf.line_start_offset(abs_line).unwrap_or(text.len());
        let line_end = buf.line_start_offset(abs_line + 1).unwrap_or(text.len());
        let line_text = &text[line_start..line_end];
        for (col, ch) in line_text.chars().enumerate().take(surface.width as usize) {
            surface.set_cell(col as u16, i as u16, ch, Some(t_style));
        }
    }
}

/// Draw the insert-mode completion popup (top-right of buffer area).
pub(super) fn render_insert_completion_popup(editor: &Editor, surface: &mut Surface, row_offset: u16, visible_lines: usize) {
    if !editor.completion.visible || editor.completion.items.is_empty()
        || editor.editor_mode.minibuffer.is_some()
    {
        return;
    }
    let popup_lines = editor.completion.items.len().min(10);
    let popup_start = visible_lines.saturating_sub(popup_lines + 1);
    let popup_width = editor.completion.items.iter().map(|s| s.len()).max().unwrap_or(20).min(60);
    let popup_x = surface.width.saturating_sub(popup_width as u16 + 2);
    for i in 0..popup_lines {
        let abs_row = popup_start + i;
        if abs_row >= visible_lines || i >= editor.completion.items.len() { break; }
        let item = &editor.completion.items[i];
        let is_sel = i == editor.completion.idx;
        let bg = if is_sel { (80, 80, 180) } else { (40, 40, 40) };
        let fg = if is_sel { (255, 255, 255) } else { (200, 200, 200) };
        let s = Style { fg, bg, ..Default::default() };
        let surf_row = row_offset + abs_row as u16;
        for x in 0..popup_width + 2 {
            let sx = popup_x + x as u16;
            if sx < surface.width { surface.set_cell(sx, surf_row, ' ', Some(s)); }
        }
        surface.set_text(popup_x + 1, surf_row, item, Some(s));
    }
}

/// Shared status bar renderer used by both text and terminal frames.
pub(super) fn render_status_bar(
    editor: &Editor,
    buf: &Buffer,
    buf_id: usize,
    cursor_offset: usize,
    surface: &mut Surface,
    is_terminal_view: bool,
) {
    let text = buf.slice(0, buf.len());
    let cursor_line = text[..cursor_offset.min(text.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count();
    let cursor_col = {
        let safe = cursor_offset.min(text.len());
        let before = &text[..safe];
        let last_line = before.rfind('\n').map(|p| &before[p + 1..]).unwrap_or(before);
        unicode_width::UnicodeWidthStr::width(last_line)
    };
    let total_lines = buf.line_count();

    let status = if let Some(ref mb) = editor.editor_mode.minibuffer {
        if mb.prompt == ":" {
            format!(" COMMAND  :{}", mb.input)
        } else {
            format!(" {}", mb.prompt)
        }
    } else if is_terminal_view {
        let running = if editor.io.terminals.contains_key(&buf_id) { " RUNNING" } else { " EXITED" };
        format!(" TERMINAL{}  {}", running, buf.name)
    } else if !editor.modeline_rendered.is_empty() {
        editor.modeline_rendered.clone()
    } else {
        let modified = if buf.modified() { " [+]" } else { "" };
        let pos = format!("{}:{}", cursor_line + 1, cursor_col + 1);
        #[allow(clippy::manual_checked_ops)]
        let pct = if total_lines > 0 {
            format!("{}%", (cursor_line + 1) * 100 / total_lines)
        } else {
            "All".to_string()
        };
        let major = buf.major_mode.name();
        format!(" {}  ({})  {}{}  {}  {}",
            editor.vim_mode_name(),
            major,
            buf.name, modified, pos, pct)
    };

    let status_fg = editor.theme_color("status-fg");
    let status_bg = editor.theme_color("status-bg");
    let style = Style { fg: status_fg, bg: status_bg, ..Default::default() };
    for x in 0..surface.width {
        surface.set_cell(x, surface.height - 1, ' ', Some(style));
    }
    surface.set_text(0, surface.height - 1, &status, Some(style));
}

/// Draw the command-mode completion popup above the status bar.
pub(super) fn render_command_completion_popup(editor: &Editor, surface: &mut Surface) {
    let items = &editor.completion.items;
    if items.is_empty() { return; }

    let max_rows = surface.height.saturating_sub(3) as usize;
    let n = items.len().min(10).min(max_rows);
    if n == 0 { return; }

    let popup_w = (items.iter().map(|s| s.len()).max().unwrap_or(4) + 4)
        .min(surface.width as usize) as u16;

    let status_y = surface.height.saturating_sub(1);
    let start_y  = status_y.saturating_sub(n as u16);

    let view_start = if editor.completion.idx + 1 > n {
        editor.completion.idx + 1 - n
    } else {
        0
    };

    for slot in 0..n {
        let item_idx = view_start + slot;
        if item_idx >= items.len() { break; }
        let y = start_y + slot as u16;
        let item = &items[item_idx];
        let is_sel = item_idx == editor.completion.idx;

        let (bg, fg) = if is_sel {
            (editor.theme_color("selection-bg"), editor.theme_color("selection-fg"))
        } else {
            (editor.theme_color("status-bg"), editor.theme_color("status-fg"))
        };
        let style = Style { fg, bg, ..Default::default() };

        for x in 0..popup_w {
            if x < surface.width {
                surface.set_cell(x, y, ' ', Some(style));
            }
        }
        let prefix = if is_sel { "> " } else { "  " };
        surface.set_text(0, y, &format!("{prefix}{item}"), Some(style));
    }
}
