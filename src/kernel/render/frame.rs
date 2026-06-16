//! Converts editor state into a `Surface` for the current frame.
//!
//! Both the TUI and GUI renderers read from the Surface after this runs.

use crate::kernel::state::Editor;
use crate::kernel::render::surface::{Style, Surface};
use super::status_and_popup::{render_status_bar, render_command_completion_popup,
                               render_terminal_frame, render_insert_completion_popup};
use super::decorations::{prefix_margin_width, render_decorations, render_overlays};
use super::highlight_pass::apply_highlights;

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

/// Scroll and cursor state for the focused buffer.
pub struct ScrollState {
    pub scroll_top: usize,
    pub cursor_vis_row: usize,
    pub cursor_col: usize,
    pub cursor_line: usize,
}

/// Compute scroll state from pre-extracted buffer data (no locking).
/// Used by `render_frame` (which already holds the buffer lock) and by
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

/// Compute the scroll offset so the cursor stays `scrolloff` lines from the
/// scroll window edges.  Also returns cursor visual row, column, and line.
/// Acquires the buffer lock internally — do NOT call while holding the lock.
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
    compute_scroll_state_raw(&text, total_lines, cursor_offset, visible_lines, scrolloff, &editor.font_config.glyph_widths)
}

pub fn render_frame(editor: &Editor, surface: &mut Surface) {
    surface.clear();

    let style_theme = |fg_key: &str, bg_key: &str| {
        let fg = editor.theme_color(fg_key);
        let bg = editor.theme_color(bg_key);
        Style { fg, bg, ..Default::default() }
    };
    let (bg_r, bg_g, bg_b) = editor.theme_color("bg");

    let tab_bar_rows: u16 = if editor.tab_bar_enabled { 1 } else { 0 };
    if editor.tab_bar_enabled {
        let tb_style = style_theme("status-fg", "status-bg");
        for x in 0..surface.width {
            surface.set_cell(x, 0, ' ', Some(tb_style));
        }
    }

    let focused   = editor.windows.focused_window();
    let buffer_id = focused.and_then(|wid| editor.windows.buffer(wid));

    if let Some(buf_id) = buffer_id
        && let Some(arc) = editor.buffers.get(buf_id) {
            let buf = arc.lock().unwrap();
            let view = editor.views.get(&buf_id);
            let cursor_offset = view.map(|v| v.cursor.offset).unwrap_or(0);
            let visible_lines = surface.height.saturating_sub(1 + tab_bar_rows) as usize;
            let is_terminal_view = crate::kernel::terminal::is_terminal(editor, buf_id);

            if is_terminal_view {
                render_terminal_frame(editor, &*buf, surface, visible_lines);
                render_status_bar(editor, &*buf, buf_id, cursor_offset, surface, true);
                return;
            }

            let header_rows: u16 = if buf.header_line.is_some() { 1 } else { 0 };
            if let Some(ref hl_text) = buf.header_line {
                let hl_style = style_theme("status-fg", "status-bg");
                for x in 0..surface.width {
                    surface.set_cell(x, tab_bar_rows, ' ', Some(hl_style));
                }
                surface.set_text(0, tab_bar_rows, hl_text, Some(hl_style));
            }
            let row_offset = tab_bar_rows + header_rows;

            let prefix_margin = prefix_margin_width(editor, buf_id);
            let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };

            let show_number  = bool_option(editor, "number");
            let show_rel     = bool_option(editor, "relativenumber");
            let use_col_gutter = !editor.gutter.columns.is_empty();
            let ln_col_width = {
                let max_line = buf.line_count().max(1);
                (max_line.ilog10() as usize + 1).max(2) + 1
            };
            let gutter_width = prefix_cols + if use_col_gutter {
                editor.gutter.columns.iter()
                    .filter(|c| c.visible)
                    .map(|c| if c.width == 0 { ln_col_width } else { c.width })
                    .sum::<usize>()
            } else if show_number || show_rel {
                ln_col_width
            } else {
                0
            };
            let content_x = gutter_width as u16;

            let total_lines   = buf.line_count();
            let text          = buf.slice(0, buf.len());
            let scrolloff     = int_option(editor, "scrolloff", 0);

            let st = compute_scroll_state_raw(&text, total_lines, cursor_offset, visible_lines, scrolloff, &editor.font_config.glyph_widths);
            let scroll_top = focused
                .and_then(|wid| editor.windows.window(wid))
                .filter(|w| w.scroll_pinned)
                .map(|w| w.scroll_offset)
                .unwrap_or(st.scroll_top);
            let cursor_vis_row = st.cursor_vis_row;
            let cursor_col = st.cursor_col;
            let cursor_line = st.cursor_line;

            let max_visible = (total_lines - scroll_top).min(visible_lines);

            let text_style = Style {
                fg: editor.theme_color("fg"),
                bg: (bg_r, bg_g, bg_b),
                ..Default::default()
            };
            for row in 0..visible_lines.min((surface.height.saturating_sub(1 + tab_bar_rows)) as usize) {
                for col in 0..surface.width {
                    surface.set_cell(col, row_offset + row as u16, ' ', Some(text_style));
                }
            }

            // Build line→severity map from diagnostics
            let mut diag_lines: std::collections::HashMap<usize, char> = std::collections::HashMap::new();
            for d in &buf.diagnostics {
                if let Some(rest) = d.strip_prefix('[') {
                    let sev = rest.chars().next().unwrap_or('?');
                    if let Some(line_part) = rest.strip_prefix(|c: char| c != ']')
                        && let Some(line_str) = line_part.trim_start_matches("] line ").split(':').next()
                            && let Ok(line) = line_str.parse::<usize>() {
                                let display_line = line.saturating_sub(1);
                                diag_lines.entry(display_line).or_insert(sev);
                            }
                }
            }

            // ── Pass 1: draw line numbers and text (fold-aware) ─────────
            let empty_folds = Vec::new();
            let folds = view.map(|v| &v.folds).unwrap_or(&empty_folds);
            let fold_map: std::collections::HashMap<usize, usize> = folds.iter()
                .map(|(s, e)| (*s, *e)).collect();
            {
                let mut display_row = 0usize;
                let mut abs_line = scroll_top;
                while abs_line < total_lines && display_row < visible_lines {
                    let line_start = buf.line_start_offset(abs_line).unwrap_or(text.len());
                    let line_end = buf.line_start_offset(abs_line + 1).unwrap_or(text.len());
                    let mut line_text = &text[line_start..line_end];
                    let surf_row = row_offset + display_row as u16;

                    let is_folded = fold_map.contains_key(&line_start);
                    let fold_str;
                    if is_folded {
                        let fold_end = fold_map[&line_start];
                        fold_str = format!("--- {} lines folded ---",
                            text[line_start..fold_end].chars().filter(|&c| c == '\n').count());
                        line_text = &fold_str;
                        abs_line += text[line_start..fold_end]
                            .chars().filter(|&c| c == '\n').count().max(1);
                    }

                    if use_col_gutter {
                        let mut col_x = prefix_cols as u16;
                        for col in editor.gutter.columns.iter().filter(|c| c.visible) {
                            let col_w = if col.width == 0 { ln_col_width } else { col.width };
                            let col_bg_style = editor.resolve_face_style(&col.face)
                                .unwrap_or_else(|| style_theme("line-num", "line-num-bg"));
                            for dx in 0..col_w as u16 {
                                surface.set_cell(col_x + dx, surf_row, ' ', Some(col_bg_style));
                            }
                            match col.name.as_str() {
                                ":line-numbers" => {
                                    let display_num = if show_rel && abs_line != cursor_line {
                                        abs_line.abs_diff(cursor_line)
                                    } else { abs_line + 1 };
                                    let num_style = if abs_line == cursor_line {
                                        style_theme("line-num-current", "line-num-bg")
                                    } else {
                                        style_theme("line-num", "line-num-bg")
                                    };
                                    let num_str = format!("{:>width$} ", display_num,
                                        width = col_w.saturating_sub(1));
                                    surface.set_text(col_x, surf_row, &num_str, Some(num_style));
                                }
                                ":folding" => {
                                    let icon = if fold_map.contains_key(&line_start) {
                                        &editor.gutter.fold_icons.closed
                                    } else {
                                        " "
                                    };
                                    let fold_style = editor.resolve_face_style(&editor.gutter.fold_icons.face)
                                        .unwrap_or(col_bg_style);
                                    surface.set_text(col_x, surf_row, icon, Some(fold_style));
                                }
                                col_name => {
                                    let key = (col_name.to_string(), buf_id);
                                    if let Some(line_map) = editor.gutter.column_signs.get(&key)
                                        && let Some(signs) = line_map.get(&abs_line)
                                        && let Some(top) = signs.iter().max_by_key(|s| s.priority)
                                    {
                                        let s = editor.resolve_face_style(&top.face)
                                            .unwrap_or(col_bg_style);
                                        surface.set_text(col_x, surf_row, &top.text, Some(s));
                                    }
                                }
                            }
                            col_x += col_w as u16;
                        }
                    } else if gutter_width > 0 {
                        let display_num = if show_rel && abs_line != cursor_line {
                            abs_line.abs_diff(cursor_line)
                        } else { abs_line + 1 };
                        let num_style = if abs_line == cursor_line {
                            style_theme("line-num-current", "line-num-bg")
                        } else {
                            style_theme("line-num", "line-num-bg")
                        };
                        let num_start = prefix_cols as u16;
                        let num_chars = gutter_width - prefix_cols;
                        let num_str = format!("{:>width$} ", display_num,
                            width = num_chars.saturating_sub(1));
                        surface.set_text(num_start, surf_row, &num_str, Some(num_style));

                        let sign_col = (gutter_width - 1) as u16;
                        let sign_drawn = if let Some(line_signs) = editor.gutter.signs
                            .get(&buf_id)
                            .and_then(|lm| lm.get(&abs_line))
                        {
                            if let Some(top_sign) = line_signs.iter().max_by_key(|s| s.priority) {
                                let s = editor.resolve_face_style(&top_sign.face)
                                    .unwrap_or_else(|| Style {
                                        fg: editor.theme_color("fg"),
                                        bg: (bg_r, bg_g, bg_b),
                                        ..Default::default()
                                    });
                                surface.set_text(sign_col, surf_row, &top_sign.text, Some(s));
                                true
                            } else { false }
                        } else { false };

                        if !sign_drawn {
                            if let Some(sev) = diag_lines.get(&abs_line) {
                                let dstyle = if *sev == 'E' {
                                    Style { fg: editor.theme_color("diag-error"), bg: (bg_r, bg_g, bg_b), bold: true, ..Default::default() }
                                } else {
                                    Style { fg: editor.theme_color("diag-warn"), bg: (bg_r, bg_g, bg_b), bold: true, ..Default::default() }
                                };
                                surface.set_cell(sign_col, surf_row, *sev, Some(dstyle));
                            }
                        }
                    }

                    let max_col = surface.width.saturating_sub(content_x) as usize;
                    for (i, ch) in line_text.chars().enumerate().take(max_col) {
                        surface.set_cell(content_x + i as u16, surf_row, ch, None);
                    }

                    display_row += 1;
                    if !is_folded { abs_line += 1; }
                }
            }

            // ── Pass 2: highlights and visual selection ──────────────────
            apply_highlights(editor, &*buf, view, surface, &text, scroll_top, max_visible,
                             content_x, row_offset, cursor_offset);

            // ── Column ruler (colorcolumn) ──────────────────────────────
            let col_col = int_option(editor, "colorcolumn", 0);
            if col_col > 0 && col_col < surface.width as usize {
                let ruler_x = col_col as u16 + content_x;
                for row in 0..visible_lines.min(surface.height as usize - 1) {
                    let surf_row = row_offset + row as u16;
                    if let Some(cell) = surface.cell(ruler_x, surf_row) {
                        let style = Style { bg: (60, 60, 80), ..cell.style };
                        surface.set_cell(ruler_x, surf_row, cell.ch, Some(style));
                    }
                }
            }

            // ── Decoration pass ──────────────────────────────────────────
            render_decorations(editor, buf_id, &*buf, surface,
                scroll_top, visible_lines, prefix_margin, gutter_width, content_x, row_offset);

            // ── Cursor (shape-aware) ─────────────────────────────────────
            if cursor_vis_row < visible_lines
                && cursor_col < (surface.width.saturating_sub(content_x)) as usize
            {
                let cursor_ch = match editor.cursor_shape.as_str() {
                    "underline" => '▔',
                    "beam" => '▏',
                    _ => '█',
                };
                let surf_row = row_offset + cursor_vis_row as u16;
                surface.set_cell(content_x + cursor_col as u16, surf_row, cursor_ch, None);
            }

            // ── Extra cursors (multi-cursor) ─────────────────────────────
            if editor.multi_cursor.active {
                for ec in &editor.multi_cursor.extra_cursors {
                    let ec_offset = ec.pos.min(text.len());
                    let ec_line = text[..ec_offset].chars()
                        .filter(|&c| c == '\n').count()
                        .min(total_lines.saturating_sub(1));
                    let ec_vis_row = ec_line.saturating_sub(scroll_top);
                    if ec_vis_row < visible_lines {
                        let ec_before = &text[..ec_offset];
                        let ec_last_line = ec_before.rfind('\n')
                            .map(|p| &ec_before[p + 1..])
                            .unwrap_or(ec_before);
                        let ec_col: usize = ec_last_line.chars()
                            .map(|c| cell_width(c, &editor.font_config.glyph_widths))
                            .sum();
                        if ec_col < (surface.width.saturating_sub(content_x)) as usize {
                            let surf_row = row_offset + ec_vis_row as u16;
                            surface.set_cell(content_x + ec_col as u16, surf_row, '▌', None);
                        }
                    }
                }
            }

            // ── Insert-mode completion popup ─────────────────────────────
            render_insert_completion_popup(editor, surface, row_offset, visible_lines);

            // ── Overlays and status bar ──────────────────────────────────
            render_overlays(editor, surface);
            render_status_bar(editor, &*buf, buf_id, cursor_offset, surface, false);
        }

    // ── Command-mode completion popup ────────────────────────────────────
    if editor.editor_mode.minibuffer.is_some()
        && editor.completion.visible
        && !editor.completion.items.is_empty()
    {
        render_command_completion_popup(editor, surface);
    }
}
