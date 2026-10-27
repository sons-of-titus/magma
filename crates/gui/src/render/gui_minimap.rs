//! Minimap code overview — Phase 7.
//!
//! A Sublime/VS-Code-style scaled-down view of the buffer drawn in a strip
//! on the right of the editor pane.  Each source line becomes a thin
//! horizontal bar whose length tracks the line length; colour runs come
//! from the buffer's syntax highlight layer.  A stroked rectangle marks the
//! current viewport.  Clicking (or dragging) the minimap jumps to that line.

use eframe::egui::{self, Color32, Pos2, Rect, CornerRadius, Sense, Vec2};
use magma::kernel::state::Editor;
use super::gui_theme::GuiTheme;

pub const STRIP_W:    f32 = 90.0;
const MIN_ROW_H:      f32 = 1.5;
const MAX_ROW_H:      f32 = 3.0;
const X_PAD:          f32 = 4.0;
const INDENT_SCALE:   f32 = 1.8;
const MAX_INDENT:     usize = 32;
const MAX_CHARS:      usize = 64;
const MIN_BAR_PX:     f32 = 1.0;

/// Sampling stride so the whole file fits the strip: 1 when it fits at
/// `MIN_ROW_H`, growing once `total_lines` exceeds `max_rows`.
pub fn minimap_step(total_lines: usize, max_rows: usize) -> usize {
    if total_lines <= max_rows { return 1; }
    total_lines.div_ceil(max_rows)
}

/// Map a click `frac_y` pixels below the strip top to a 0-based line.
pub fn click_line(frac_y: f32, row_h: f32, step: usize, total_lines: usize) -> usize {
    let idx = ((frac_y / row_h).max(0.0) as usize).saturating_mul(step);
    idx.min(total_lines.saturating_sub(1))
}

/// Bar width in pixels for a line of `chars` non-trailing characters.
pub fn line_bar_px(chars: usize) -> f32 {
    ((chars.min(MAX_CHARS) as f32 / MAX_CHARS as f32) * (STRIP_W - X_PAD * 2.0))
        .max(MIN_BAR_PX)
}

/// Sampled-line cache type lives in `state::minimap` so the kernel holds it
/// without the gui feature.
use magma::kernel::state::minimap::MinimapCache;

pub struct Minimap;

impl Minimap {
    /// Draw the minimap into `rect` for buffer `buf_id`.
    ///
    /// Returns a 0-based target line when the user clicks or drags inside
    /// the strip, `None` otherwise.
    pub fn show(
        ui:           &mut egui::Ui,
        editor:       &mut Editor,
        buf_id:       usize,
        rect:         Rect,
        theme:        &GuiTheme,
        scroll_top:   usize,
        visible_rows: usize,
    ) -> Option<usize> {
        let arc = editor.buffers.get(buf_id)?;
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let total = buf.line_count();
        if total == 0 || rect.height() < 8.0 { return None; }

        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let _ = resp;

        ui.painter().rect_filled(rect, CornerRadius::ZERO, theme.crust);

        let max_rows = ((rect.height() / MIN_ROW_H).floor() as usize).max(1);
        let step     = minimap_step(total, max_rows);
        let row_h    = (rect.height() / total as f32).clamp(MIN_ROW_H, MAX_ROW_H);

        let hl = collect_highlights(editor, buf_id);

        let base_fg = theme.subtext0;
        // Sampled lines are cached per (revision, strip size); the frame
        // only paints them.
        let cache = editor.minimap_cache.entry(buf_id).or_default();
        if cache.revision != buf.revision() || cache.total != total || cache.max_rows != max_rows {
            let mut lines = Vec::with_capacity(total / step + 1);
            let mut line_idx = 0usize;
            while line_idx < total {
                if line_idx.is_multiple_of(step) {
                    let span = buf.line_span(line_idx).unwrap_or(0..0);
                    lines.push((span.start, buf.slice(span.start, span.end)));
                }
                line_idx += 1;
            }
            *cache = MinimapCache { revision: buf.revision(), total, max_rows, lines };
        }
        let lines = std::mem::take(&mut cache.lines);
        for (row, (start, text)) in lines.iter().enumerate() {
            Self::draw_line(
                ui, text, *start,
                &hl, editor, base_fg,
                rect.left(), rect.top() + row as f32 * row_h, row_h,
            );
        }
        if let Some(cache) = editor.minimap_cache.get_mut(&buf_id) {
            cache.lines = lines;
        }
        Self::viewport_rect(ui, rect, theme, row_h, step, scroll_top, visible_rows);

        // ── Click / drag → jump ──────────────────────────────────────────
        let ptr = ui.ctx().pointer_hover_pos()?;
        if !rect.contains(ptr) { return None; }
        let pressed = ui.ctx().input(|i| i.pointer.primary_down());
        if !pressed { return None; }
        Some(click_line(ptr.y - rect.top(), row_h, step, total))
    }

    fn viewport_rect(
        ui:           &mut egui::Ui,
        rect:         Rect,
        theme:        &GuiTheme,
        row_h:        f32,
        step:         usize,
        scroll_top:   usize,
        visible_rows: usize,
    ) {
        let vp_top  = rect.top() + (scroll_top / step) as f32 * row_h;
        let vp_rows = ((visible_rows / step).max(1) as f32 * row_h).max(row_h);
        ui.painter().rect_stroke(
            Rect::from_min_size(
                Pos2::new(rect.left() + 1.0, vp_top),
                Vec2::new(STRIP_W - 2.0, vp_rows.min(rect.height())),
            ),
            CornerRadius::same(2),
            egui::Stroke::new(1.0, theme.overlay0),
            egui::StrokeKind::Inside,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_line(
        ui:         &mut egui::Ui,
        line:       &str,
        line_start: usize,
        hl:         &[(usize, usize, String)],
        editor:     &Editor,
        base_fg:    Color32,
        strip_left: f32,
        y:          f32,
        row_h:      f32,
    ) {
        let trimmed = line.trim_end();
        let chars = trimmed.chars().count();
        if chars == 0 { return; }

        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        let indent_px = indent.min(MAX_INDENT) as f32 / 2.0 * INDENT_SCALE;
        let bar_px = line_bar_px(chars);
        let total = chars as f32;
        let h = (row_h * 0.66).max(1.0);

        // Same-colour runs via highlight ranges overlapping this line.
        let mut runs: Vec<(usize, Color32)> = Vec::new();
        for (i, _) in trimmed.char_indices() {
            let byte = line_start + i;
            let face = hl.iter().rev().find(|(s, e, _)| *s <= byte && byte < *e)
                .map(|(_, _, f)| f.as_str());
            let color = face
                .and_then(|f| editor.faces.get(f))
                .map(|st| Color32::from_rgb(st.fg.0, st.fg.1, st.fg.2))
                .unwrap_or(base_fg);
            let col = trimmed[..i].chars().count();
            match runs.last_mut() {
                Some((_, c)) if *c == color => {}
                _ => runs.push((col, color)),
            }
        }

        let mut prev_col = 0usize;
        for (i, (col, color)) in runs.iter().enumerate() {
            let next_col = runs.get(i + 1).map(|(c, _)| *c).unwrap_or(chars);
            let run_w = ((next_col - prev_col) as f32 / total) * bar_px;
            if run_w >= 0.5 {
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        Pos2::new(
                            strip_left + X_PAD + indent_px
                                + (prev_col as f32 / total) * bar_px,
                            y,
                        ),
                        Vec2::new(run_w.max(MIN_BAR_PX), h),
                    ),
                    CornerRadius::same(1),
                    *color,
                );
            }
            prev_col = *col;
        }
    }
}

fn collect_highlights(editor: &Editor, buf_id: usize) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    if let Some(v) = editor.view_of_buffer(buf_id) {
        for (s, e, f) in &v.highlights { out.push((*s, *e, f.clone())); }
        for layer in v.highlight_layers.values() {
            for (s, e, f) in layer { out.push((*s, *e, f.clone())); }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_one_when_file_fits() {
        assert_eq!(minimap_step(100, 400), 1);
    }

    #[test]
    fn step_grows_for_long_files() {
        let step = minimap_step(1000, 400);
        assert!(step >= 3, "expected stride ≥ 3, got {step}");
        // Every sampled row maps back into the file after clamping.
        for row in 0..400 {
            assert!(click_line(row as f32 * 2.0, 2.0, step, 1000) < 1000);
        }
    }

    #[test]
    fn click_line_clamps_to_file() {
        assert_eq!(click_line(1e9, 2.0, 1, 500), 499);
        assert_eq!(click_line(-5.0, 2.0, 1, 500), 0);
    }

    #[test]
    fn click_line_respects_step() {
        assert_eq!(click_line(0.0, 2.0, 3, 100), 0);
        assert_eq!(click_line(4.0, 2.0, 3, 100), 6);
    }

    #[test]
    fn bar_width_scales_and_clamps() {
        assert_eq!(line_bar_px(0), MIN_BAR_PX);
        assert_eq!(line_bar_px(64), line_bar_px(500));
        assert!(line_bar_px(32) > MIN_BAR_PX);
    }
}
