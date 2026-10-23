//! egui surface painter for the GUI backend (CPU fallback path).

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

use crate::kernel::render::surface::{Cell, Surface};
use crate::kernel::state::Editor;

/// Render a `Surface` onto an egui `Rect`.
///
/// All surface rows are drawn. Status bar and tab bar are handled as native
/// egui panels; the surface passed here contains editor pane content only.
///
/// Cells are painted as same-style runs — one text call and one rect call per
/// run, not per cell.  A viewport is thousands of cells; per-cell painting
/// dominated the CPU profile.  Each run starts on its own cell boundary, so
/// font-advance drift can never accumulate past one run.
pub fn present(
    ui: &egui::Ui,
    ed: &Editor,
    surface: &Surface,
    rect: Rect,
    char_w: f32,
    line_h: f32,
    surface_y_offset: usize,
) {
    let (r, g, b) = ed.theme_color("gui-bg");
    let bg_color = Color32::from_rgb(r, g, b);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, bg_color);

    let font_id = FontId::new(line_h * 0.85, egui::FontFamily::Monospace);
    let render_rows = surface.height as usize - surface_y_offset;

    for sy in surface_y_offset..surface_y_offset + render_rows {
        let dy = (sy - surface_y_offset) as f32 * line_h;
        let y = rect.min.y + dy;

        // ── Background rects: one per same-bg run, skipping the default bg ──
        let mut bg_run: Option<(u16, Color32)> = None;
        for sx in 0..surface.width as usize {
            let cell_bg = cell_bg(surface.cell(sx as u16, sy as u16));
            if cell_bg == bg_color {
                flush_bg(&painter, &mut bg_run, sx, rect.min.x, y, char_w, line_h);
                continue;
            }
            match &mut bg_run {
                Some((_, color)) if *color == cell_bg => {}
                _ => {
                    flush_bg(&painter, &mut bg_run, sx, rect.min.x, y, char_w, line_h);
                    bg_run = Some((sx as u16, cell_bg));
                }
            }
        }
        flush_bg(&painter, &mut bg_run, surface.width as usize, rect.min.x, y, char_w, line_h);

        // ── Glyphs: one text call per same-style run ────────────────────────
        let mut run: Option<(u16, Color32, String)> = None;
        for sx in 0..surface.width as usize {
            let Some(cell) = surface.cell(sx as u16, sy as u16) else { continue };
            let paintable = cell.ch != ' ' && cell.ch != '\0';
            let fg = Color32::from_rgb(cell.style.fg.0, cell.style.fg.1, cell.style.fg.2);
            match (&mut run, paintable) {
                (Some((_, run_fg, text)), true) if *run_fg == fg => text.push(cell.ch),
                _ => {
                    flush_text(&painter, &mut run, font_id.clone(), rect.min.x, y, char_w);
                    if paintable {
                        run = Some((sx as u16, fg, cell.ch.to_string()));
                    }
                }
            }
        }
        flush_text(&painter, &mut run, font_id.clone(), rect.min.x, y, char_w);
    }
}

fn cell_bg(cell: Option<&Cell>) -> Color32 {
    match cell {
        Some(c) => Color32::from_rgb(c.style.bg.0, c.style.bg.1, c.style.bg.2),
        None => Color32::BLACK,
    }
}

fn flush_bg(
    painter: &egui::Painter,
    run: &mut Option<(u16, Color32)>,
    end_sx: usize,
    x0: f32,
    y: f32,
    char_w: f32,
    line_h: f32,
) {
    if let Some((start, color)) = run.take() {
        let rect = Rect::from_min_size(
            Pos2::new(x0 + start as f32 * char_w, y),
            Vec2::new((end_sx - start as usize) as f32 * char_w, line_h),
        );
        painter.rect_filled(rect, 0.0, color);
    }
}

fn flush_text(
    painter: &egui::Painter,
    run: &mut Option<(u16, Color32, String)>,
    font_id: FontId,
    x0: f32,
    y: f32,
    char_w: f32,
) {
    if let Some((start, fg, text)) = run.take() {
        painter.text(
            Pos2::new(x0 + start as f32 * char_w, y),
            Align2::LEFT_TOP,
            text.as_str(),
            font_id,
            fg,
        );
    }
}
