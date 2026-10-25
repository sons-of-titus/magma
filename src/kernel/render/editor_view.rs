//! EditorView — renders a buffer's text, gutter, cursor, and decorations.

use std::collections::HashMap;

use crate::kernel::state::Editor;
use crate::kernel::state::id::WindowId;
use crate::kernel::render::surface::{Style, Surface};
use crate::kernel::render::view::{View, ViewKind, RenderCtx};
use crate::kernel::render::frame::{
    int_option, pane_scroll_left, pane_scroll_top_synced, scroll_state_for_buffer,
};
use crate::kernel::render::decorations::{prefix_margin_width, render_decorations};
use crate::kernel::render::highlight_pass::apply_highlights;
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
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let view = editor.view_of_buffer(buf_id);
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
        let content_cols = (area.width as usize).saturating_sub(gutter_width);

        let scrolloff = int_option(editor, "scrolloff", 0);

        let is_focused = editor.view_tree.focused_window() == Some(self.pane_id);
        let st = scroll_state_for_buffer(
            &buf, cursor_offset,
            visible_lines, scrolloff, int_option(editor, "tab-width", 4),
            &editor.font_config.glyph_widths,
        );
        let cursor_line = st.cursor_line;
        // The synced pane state is authoritative; the stateless fallback
        // comes from the `st` we already computed — never a second buffer
        // lock (render holds one).
        let scroll_top = editor.view_tree.window(self.pane_id)
            .and_then(|p| pane_scroll_top_synced(p, buf_id))
            .unwrap_or(st.scroll_top);
        // First visible content column (0 = line start).  The gutter stays
        // fixed; only text columns pan.
        let scroll_left = editor.view_tree.window(self.pane_id)
            .map(|p| pane_scroll_left(p, buf_id))
            .unwrap_or(0);
        // The cursor may sit above a manually scrolled viewport.
        let cursor_vis_row = cursor_line.checked_sub(scroll_top);
        let cursor_col = st.cursor_col;

        let max_visible = (total_lines - scroll_top).min(visible_lines);
        let text_style = Style {
            fg: editor.theme_color("fg"), bg: (bg_r, bg_g, bg_b), ..Default::default()
        };
        for row in 0..visible_lines.min(area.height as usize) {
            for x in 0..area.width {
                surface.set_cell(area.x + x, row_offset + row as u16, ' ', Some(text_style));
            }
        }

        // Highest severity per line, from the typed diagnostics.
        let mut diag_lines: HashMap<usize, char> = HashMap::new();
        for d in buf.get_diagnostics() {
            diag_lines.entry(d.line).or_insert(d.severity.code());
        }

        let buf_path_owned: Option<String> = buf.path.clone();
        let tab_w = int_option(editor, "tab-width", 4);

        // ── Pass 1: gutter + text (fold-aware) ───────────────────────────────
        let empty_folds = Vec::new();
        let folds = view.map(|v| &v.folds).unwrap_or(&empty_folds);
        let fold_map: HashMap<usize, usize> =
            folds.iter().map(|(s, e)| (*s, *e)).collect();

        // Visible rows extracted once: (line_start, line_end_incl_newline, text).
        // No whole-document copy is made to render a viewport (§9.1).
        struct Row {
            start: usize,
            end: usize,
            text: String,
            fold: bool,
        }
        let mut rows: Vec<Row> = Vec::new();
        {
            let mut abs_line = scroll_top;
            while abs_line < total_lines && rows.len() < visible_lines {
                let line_start = buf.line_start_offset(abs_line).unwrap_or(buf.len());
                let line_end = buf.line_start_offset(abs_line + 1).unwrap_or(buf.len());
                let is_fold_start = fold_map.contains_key(&line_start);
                if is_fold_start {
                    let fold_end = fold_map[&line_start];
                    let folded = buf.position_of(fold_end).line.saturating_sub(abs_line).max(1);
                    rows.push(Row {
                        start: line_start,
                        end: line_end,
                        text: format!("--- {} lines folded ---", folded),
                        fold: true,
                    });
                    abs_line += folded;
                } else {
                    rows.push(Row {
                        start: line_start,
                        end: line_end,
                        text: buf.slice(line_start, line_end),
                        fold: false,
                    });
                    abs_line += 1;
                }
            }
        }

        let mut display_row = 0usize;
        let mut abs_line = scroll_top;
        for row in &rows {
            let line_start = row.start;
            let line_text = row.text.as_str();
            let surf_row = row_offset + display_row as u16;
            let is_fold_start = row.fold;

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

            // Grapheme clusters fill their cell width (CJK = 2 cells,
            // combining marks = 0), so wide glyphs align (§10).  The
            // grapheme→cell mapping comes from the row-layout cache (R-12):
            // one segmentation per (line, revision), reused by the
            // highlight passes.  Fold rows render synthetic text and stay
            // uncached — their key would collide with the real line.
            let max_col = content_cols;
            {
                let layout = if is_fold_start {
                    std::sync::Arc::new(crate::kernel::render::row_layout::RowLayout::build(
                        line_text, 0, tab_w, &editor.font_config.glyph_widths))
                } else {
                    crate::kernel::render::row_layout::row_layout(
                        editor, buf_id, line_start, line_text, buf.revision(), tab_w)
                };
                for i in 0..layout.widths.len() {
                    let col = layout.cols[i] as usize;
                    if col < scroll_left { continue; }
                    let screen = col - scroll_left;
                    if screen >= max_col { break; }
                    let w = layout.widths[i] as usize;
                    surface.set_cell(content_x + screen as u16, surf_row, layout.leads[i], None);
                    // Zero-width trailing marks stay on the lead cell; wide
                    // glyphs blank their second cell so highlights look right.
                    for k in 1..w {
                        if screen + k < max_col {
                            surface.set_cell(content_x + (screen + k) as u16, surf_row, ' ', None);
                        }
                    }
                }
            }
            display_row += 1;
            if !is_fold_start { abs_line += 1; }
            let _ = abs_line;
        }

        // ── Pass 2: highlights + visual selection ─────────────────────────
        let row_refs: Vec<(usize, usize, &str)> =
            rows.iter().map(|r| (r.start, r.end, r.text.as_str())).collect();
        let row_layouts: Vec<std::sync::Arc<crate::kernel::render::row_layout::RowLayout>> =
            rows.iter().map(|r| {
                if r.fold {
                    std::sync::Arc::new(crate::kernel::render::row_layout::RowLayout::build(
                        &r.text, 0, tab_w, &editor.font_config.glyph_widths))
                } else {
                    crate::kernel::render::row_layout::row_layout(
                        editor, buf_id, r.start, &r.text, buf.revision(), tab_w)
                }
            }).collect();
        apply_highlights(editor, view, surface, &row_refs, &row_layouts,
                         scroll_top, max_visible, content_x, row_offset, cursor_offset,
                         scroll_left);

        // ── Current execution line — the stopped debuggee's location ──────
        if let Some((path, line)) = editor.debug.active_session()
            .and_then(|s| s.stopped_line.clone())
            && buf_path_owned.as_deref() == Some(path.as_str()) {
            let stop_offset = buf.line_start_offset(line.saturating_sub(1));
            if let Some(stop_offset) = stop_offset
                && let Some(row_idx) = rows.iter()
                    .position(|r| r.start <= stop_offset && stop_offset < r.end) {
                    let surf_row = row_offset + row_idx as u16;
                    let (hl_r, hl_g, hl_b) = editor.theme_color("highlight-bg");
                    for x in content_x..area.x + area.width {
                        if let Some(cell) = surface.cell(x, surf_row) {
                            let style = Style { bg: (hl_r, hl_g, hl_b), ..cell.style };
                            surface.set_cell(x, surf_row, cell.ch, Some(style));
                        }
                    }
                }
        }

        // ── Column ruler ──────────────────────────────────────────────────
        let col_col = int_option(editor, "colorcolumn", 0);
        if col_col > 0 && col_col >= scroll_left {
            let ruler_x = content_x + (col_col - scroll_left) as u16;
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
        render_decorations(editor, buf_id, &buf, surface,
            scroll_top, visible_lines, prefix_margin, gutter_width, content_x, row_offset,
            scroll_left);

        // ── Cursor + multi-cursor + completion popup (focused pane only) ──
        // With `cursor_overlay` the GUI paints a native cursor rect on top;
        // stamping the full-cell glyph would double-draw.
        if is_focused && editor.cursor_visible && !editor.cursor_overlay {
            if let Some(vis_row) = cursor_vis_row
                && vis_row < visible_lines
                && cursor_col >= scroll_left
                && cursor_col - scroll_left < content_cols
            {
                let cursor_ch = match editor.cursor_shape.as_str() {
                    "underline" => '▔',
                    "beam" => '▏',
                    _ => '█',
                };
                surface.set_cell(content_x + (cursor_col - scroll_left) as u16,
                    row_offset + vis_row as u16, cursor_ch, None);
            }

            if editor.multi_cursor.active {
                let tab_w = int_option(editor, "tab-width", 4);
                for ec in &editor.multi_cursor.extra_cursors {
                    let ec_off = ec.pos.min(buf.len());
                    let ec_line = buf.position_of(ec_off).line
                        .min(total_lines.saturating_sub(1));
                    let Some(ec_vis) = ec_line.checked_sub(scroll_top) else { continue };
                    if ec_vis < visible_lines {
                        let ec_line_start = buf.line_start_offset(ec_line).unwrap_or(0);
                        let ec_line_end = buf.line_start_offset(ec_line + 1).unwrap_or(buf.len());
                        let ec_col = crate::kernel::text_engine::text_layout::cell_column(
                            &buf.slice(ec_line_start, ec_line_end),
                            ec_off.saturating_sub(ec_line_start),
                            tab_w,
                            &editor.font_config.glyph_widths,
                        );
                        if ec_col >= scroll_left && ec_col - scroll_left < content_cols {
                            surface.set_cell(content_x + (ec_col - scroll_left) as u16,
                                row_offset + ec_vis as u16, '▌', None);
                        }
                    }
                }
            }

            if !editor.cursor_overlay {
                render_insert_completion_popup(editor, surface, row_offset, visible_lines);
            }
        }
    }
}

/// Draw the insert-mode completion popup (top-right of buffer area).
fn render_insert_completion_popup(editor: &Editor, surface: &mut Surface, row_offset: u16, visible_lines: usize) {
    if !editor.completion.visible || editor.completion.items.is_empty() {
        return;
    }
    let popup_lines = editor.completion.items.len().min(10);
    let popup_start = visible_lines.saturating_sub(popup_lines + 1);
    let popup_width = editor.completion.items.iter()
        .map(|i| i.label.len() + i.kind.len() + 1).max().unwrap_or(20).min(60);
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
        let text = if item.kind.is_empty() {
            item.label.clone()
        } else {
            format!("{} {}", item.label, item.kind)
        };
        surface.set_text(popup_x + 1, surf_row, &text, Some(s));
    }
}
