//! EditorView — renders a buffer's text, gutter, cursor, and decorations.

use std::collections::HashMap;

use crate::kernel::state::id::WindowId;
use crate::kernel::render::surface::{Style, Surface};
use crate::kernel::render::view::{View, ViewKind, RenderCtx};
use crate::kernel::render::frame::{cell_width, compute_scroll_state_raw, int_option};
use crate::kernel::render::decorations::{prefix_margin_width, render_decorations};
use crate::kernel::render::highlight_pass::apply_highlights;
use crate::kernel::render::status_and_popup::render_insert_completion_popup;
use crate::kernel::render::gutter::{GutterCtx, GutterRegistry};

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

        let header_rows: u16 = if buf.header_line.is_some() { 1 } else { 0 };

        let style_theme = |fg_key: &str, bg_key: &str| -> Style {
            Style { fg: editor.theme_color(fg_key), bg: editor.theme_color(bg_key), ..Default::default() }
        };
        let (bg_r, bg_g, bg_b) = editor.theme_color("bg");

        if let Some(ref hl_text) = buf.header_line {
            let hl_style = style_theme("status-fg", "status-bg");
            for x in 0..area.width {
                surface.set_cell(area.x + x, area.y, ' ', Some(hl_style));
            }
            surface.set_text(area.x, area.y, hl_text, Some(hl_style));
        }

        let row_offset = area.y + header_rows;
        let visible_lines = (area.height as usize)
            .saturating_sub(header_rows as usize);

        let prefix_margin = prefix_margin_width(editor, buf_id);
        let prefix_cols = if prefix_margin > 0 { prefix_margin + 1 } else { 0 };

        let total_lines = buf.line_count();
        let gutter_width = prefix_cols + editor.gutter.total_width(total_lines);
        let content_x = area.x + gutter_width as u16;

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

        // Pre-compute diagnostic severity per line from buf.diagnostics strings.
        let mut diag_lines: HashMap<usize, char> = HashMap::new();
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

        let buf_path_owned: Option<String> = buf.path.clone();

        // ── Pass 1: gutter + text (fold-aware) ───────────────────────────────
        let empty_folds = Vec::new();
        let folds = view.map(|v| &v.folds).unwrap_or(&empty_folds);
        let fold_map: HashMap<usize, usize> =
            folds.iter().map(|(s, e)| (*s, *e)).collect();
        let mut display_row = 0usize;
        let mut abs_line = scroll_top;
        while abs_line < total_lines && display_row < visible_lines {
            let line_start = buf.line_start_offset(abs_line).unwrap_or(text.len());
            let line_end   = buf.line_start_offset(abs_line + 1).unwrap_or(text.len());
            let mut line_text = &text[line_start..line_end];
            let surf_row = row_offset + display_row as u16;

            let is_fold_start = fold_map.contains_key(&line_start);
            let fold_str;
            if is_fold_start {
                let fold_end = fold_map[&line_start];
                fold_str = format!("--- {} lines folded ---",
                    text[line_start..fold_end].chars().filter(|&c| c == '\n').count());
                line_text = &fold_str;
                abs_line += text[line_start..fold_end]
                    .chars().filter(|&c| c == '\n').count().max(1);
            }

            // ── Render gutter providers ───────────────────────────────────
            if !editor.gutter.providers.is_empty() {
                let gutter_ctx = GutterCtx {
                    editor,
                    buf_id,
                    buf_path: buf_path_owned.as_deref(),
                    cursor_line,
                    line_count: total_lines,
                    line_start_offset: line_start,
                    is_fold_start,
                    diag_lines: &diag_lines,
                };
                let gutter_bg = style_theme("line-num", "line-num-bg");
                let mut col_x = area.x + prefix_cols as u16;
                for provider in &editor.gutter.providers {
                    let col_w = GutterRegistry::provider_width(provider.as_ref(), total_lines);
                    // Fill background.
                    for dx in 0..col_w as u16 {
                        surface.set_cell(col_x + dx, surf_row, ' ', Some(gutter_bg));
                    }
                    // Render provider cell.
                    if let Some(cell) = provider.render(abs_line, &gutter_ctx) {
                        surface.set_text(col_x, surf_row, &cell.text, Some(cell.style));
                    }
                    col_x += col_w as u16;
                }
            }

            let max_col = area.width.saturating_sub(gutter_width as u16) as usize;
            for (i, ch) in line_text.chars().enumerate().take(max_col) {
                surface.set_cell(content_x + i as u16, surf_row, ch, None);
            }
            display_row += 1;
            if !is_fold_start { abs_line += 1; }
        }

        // ── Pass 2: highlights + visual selection ─────────────────────────
        apply_highlights(editor, &*buf, view, surface, &text, scroll_top, max_visible,
                         content_x, row_offset, cursor_offset);

        // ── Column ruler ──────────────────────────────────────────────────
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

        // ── Decorations ───────────────────────────────────────────────────
        render_decorations(editor, buf_id, &*buf, surface,
            scroll_top, visible_lines, prefix_margin, gutter_width, content_x, row_offset);

        // ── Cursor + multi-cursor + completion popup (focused pane only) ──
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
