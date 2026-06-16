//! EditorView — renders a buffer's text, gutter, cursor, and decorations.

use crate::kernel::state::id::WindowId;
use crate::kernel::render::surface::{Style, Surface};
use crate::kernel::render::view::{View, ViewKind, RenderCtx};
use crate::kernel::render::frame::{cell_width, compute_scroll_state_raw, int_option, bool_option};
use crate::kernel::render::decorations::{prefix_margin_width, render_decorations};
use crate::kernel::render::highlight_pass::apply_highlights;
use crate::kernel::render::status_and_popup::render_insert_completion_popup;

pub struct EditorView {
    pub buf_id: usize,
    pub pane_id: WindowId,
}

impl View for EditorView {
    fn kind(&self) -> ViewKind { ViewKind::Editor }

    fn render(&self, surface: &mut Surface, ctx: &RenderCtx) {
        let editor = ctx.editor;
        let buf_id = self.buf_id;
        let area = ctx.area;

        let arc = match editor.buffers.get(buf_id) {
            Some(a) => a,
            None => return,
        };
        let buf = arc.lock().unwrap();
        let view = editor.views.get(&buf_id);
        let cursor_offset = view.map(|v| v.cursor.offset).unwrap_or(0);

        let tab_bar_rows: u16 = if editor.tab_bar_enabled { 1 } else { 0 };
        let header_rows: u16 = if buf.header_line.is_some() { 1 } else { 0 };

        let style_theme = |fg_key: &str, bg_key: &str| -> Style {
            Style { fg: editor.theme_color(fg_key), bg: editor.theme_color(bg_key), ..Default::default() }
        };
        let (bg_r, bg_g, bg_b) = editor.theme_color("bg");

        if let Some(ref hl_text) = buf.header_line {
            let hl_style = style_theme("status-fg", "status-bg");
            for x in 0..area.width {
                surface.set_cell(area.x + x, tab_bar_rows, ' ', Some(hl_style));
            }
            surface.set_text(area.x, tab_bar_rows, hl_text, Some(hl_style));
        }

        let row_offset = area.y + tab_bar_rows + header_rows;
        let visible_lines = (area.height as usize)
            .saturating_sub((tab_bar_rows + header_rows) as usize);

        let prefix_margin = prefix_margin_width(editor, buf_id);
        let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };

        let show_number = bool_option(editor, "number");
        let show_rel = bool_option(editor, "relativenumber");
        let use_col_gutter = !editor.gutter.columns.is_empty();
        let ln_col_width = (buf.line_count().max(1).ilog10() as usize + 1).max(2) + 1;
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
        let content_x = area.x + gutter_width as u16;

        let total_lines = buf.line_count();
        let text = buf.slice(0, buf.len());
        let scrolloff = int_option(editor, "scrolloff", 0);

        let is_focused = editor.view_tree.focused_window() == Some(self.pane_id);
        let st = compute_scroll_state_raw(
            &text, total_lines, cursor_offset,
            visible_lines, scrolloff, &editor.font_config.glyph_widths,
        );
        let scroll_top = editor.view_tree.window(self.pane_id)
            .filter(|p| p.scroll_pinned)
            .map(|p| p.scroll_offset)
            .unwrap_or(st.scroll_top);
        let cursor_vis_row = st.cursor_vis_row;
        let cursor_col = st.cursor_col;
        let cursor_line = st.cursor_line;

        let max_visible = (total_lines - scroll_top).min(visible_lines);
        let text_style = Style {
            fg: editor.theme_color("fg"), bg: (bg_r, bg_g, bg_b), ..Default::default()
        };
        for row in 0..visible_lines.min(area.height as usize) {
            for x in 0..area.width {
                surface.set_cell(area.x + x, row_offset + row as u16, ' ', Some(text_style));
            }
        }

        let mut diag_lines: std::collections::HashMap<usize, char> = std::collections::HashMap::new();
        for d in &buf.diagnostics {
            if let Some(rest) = d.strip_prefix('[') {
                let sev = rest.chars().next().unwrap_or('?');
                if let Some(line_part) = rest.strip_prefix(|c: char| c != ']')
                    && let Some(line_str) = line_part.trim_start_matches("] line ").split(':').next()
                    && let Ok(line) = line_str.parse::<usize>() {
                        diag_lines.entry(line.saturating_sub(1)).or_insert(sev);
                    }
            }
        }

        // ── Pass 1: line numbers + text (fold-aware) ──────────────────────
        let empty_folds = Vec::new();
        let folds = view.map(|v| &v.folds).unwrap_or(&empty_folds);
        let fold_map: std::collections::HashMap<usize, usize> =
            folds.iter().map(|(s, e)| (*s, *e)).collect();
        let mut display_row = 0usize;
        let mut abs_line = scroll_top;
        while abs_line < total_lines && display_row < visible_lines {
            let line_start = buf.line_start_offset(abs_line).unwrap_or(text.len());
            let line_end   = buf.line_start_offset(abs_line + 1).unwrap_or(text.len());
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
                let mut col_x = area.x + prefix_cols as u16;
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
                            surface.set_text(col_x, surf_row,
                                &format!("{:>width$} ", display_num, width = col_w.saturating_sub(1)),
                                Some(num_style));
                        }
                        ":folding" => {
                            let icon = if fold_map.contains_key(&line_start) {
                                &editor.gutter.fold_icons.closed
                            } else { " " };
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
                                let s = editor.resolve_face_style(&top.face).unwrap_or(col_bg_style);
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
                let num_start = area.x + prefix_cols as u16;
                let num_chars = gutter_width - prefix_cols;
                surface.set_text(num_start, surf_row,
                    &format!("{:>width$} ", display_num, width = num_chars.saturating_sub(1)),
                    Some(num_style));

                let sign_col = area.x + (gutter_width - 1) as u16;
                let sign_drawn = if let Some(line_signs) = editor.gutter.signs
                    .get(&buf_id).and_then(|lm| lm.get(&abs_line))
                {
                    if let Some(top) = line_signs.iter().max_by_key(|s| s.priority) {
                        let s = editor.resolve_face_style(&top.face)
                            .unwrap_or_else(|| Style {
                                fg: editor.theme_color("fg"), bg: (bg_r, bg_g, bg_b), ..Default::default()
                            });
                        surface.set_text(sign_col, surf_row, &top.text, Some(s));
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

            let max_col = area.width.saturating_sub(gutter_width as u16) as usize;
            for (i, ch) in line_text.chars().enumerate().take(max_col) {
                surface.set_cell(content_x + i as u16, surf_row, ch, None);
            }
            display_row += 1;
            if !is_folded { abs_line += 1; }
        }

        // ── Pass 2: highlights + visual selection ──────────────────────────
        apply_highlights(editor, &*buf, view, surface, &text, scroll_top, max_visible,
                         content_x, row_offset, cursor_offset);

        // ── Column ruler ───────────────────────────────────────────────────
        let col_col = int_option(editor, "colorcolumn", 0);
        if col_col > 0 {
            let ruler_x = col_col as u16 + content_x;
            if ruler_x < area.x + area.width {
                for row in 0..visible_lines.min(area.height as usize) {
                    let surf_row = row_offset + row as u16;
                    if let Some(cell) = surface.cell(ruler_x, surf_row) {
                        let style = Style { bg: (60, 60, 80), ..cell.style };
                        surface.set_cell(ruler_x, surf_row, cell.ch, Some(style));
                    }
                }
            }
        }

        // ── Decorations ────────────────────────────────────────────────────
        render_decorations(editor, buf_id, &*buf, surface,
            scroll_top, visible_lines, prefix_margin, gutter_width, content_x, row_offset);

        // ── Cursor + multi-cursor + completion popup (focused pane only) ───
        if is_focused {
            if cursor_vis_row < visible_lines
                && cursor_col < area.width.saturating_sub(gutter_width as u16) as usize
            {
                let cursor_ch = match editor.cursor_shape.as_str() {
                    "underline" => '▔',
                    "beam" => '▏',
                    _ => '█',
                };
                surface.set_cell(content_x + cursor_col as u16,
                    row_offset + cursor_vis_row as u16, cursor_ch, None);
            }

            if editor.multi_cursor.active {
                for ec in &editor.multi_cursor.extra_cursors {
                    let ec_off = ec.pos.min(text.len());
                    let ec_line = text[..ec_off].chars()
                        .filter(|&c| c == '\n').count()
                        .min(total_lines.saturating_sub(1));
                    let ec_vis = ec_line.saturating_sub(scroll_top);
                    if ec_vis < visible_lines {
                        let ec_before = &text[..ec_off];
                        let ec_last = ec_before.rfind('\n')
                            .map(|p| &ec_before[p + 1..]).unwrap_or(ec_before);
                        let ec_col: usize = ec_last.chars()
                            .map(|c| cell_width(c, &editor.font_config.glyph_widths)).sum();
                        if ec_col < area.width.saturating_sub(gutter_width as u16) as usize {
                            surface.set_cell(content_x + ec_col as u16,
                                row_offset + ec_vis as u16, '▌', None);
                        }
                    }
                }
            }

            render_insert_completion_popup(editor, surface, row_offset, visible_lines);
        }
    }
}
