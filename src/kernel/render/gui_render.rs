//! egui surface painter for the GUI backend (CPU fallback path).

use std::sync::{Arc, RwLock};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

use crate::kernel::render::surface::Surface;
use crate::kernel::state::Editor;

pub fn safe_boundary(s: &str, mut offset: usize) -> usize {
    offset = offset.min(s.len());
    while offset > 0 && !s.is_char_boundary(offset) { offset -= 1; }
    offset
}

/// Render a `Surface` onto an egui `Rect`.
///
/// All surface rows are drawn. Status bar and tab bar are handled as native
/// egui panels; the surface passed here contains editor pane content only.
pub fn present(
    ui: &egui::Ui,
    editor_ref: &Arc<RwLock<Editor>>,
    surface: &Surface,
    rect: Rect,
    char_w: f32,
    line_h: f32,
    surface_y_offset: usize,
) {
    let bg_color = {
        let ed = editor_ref.read().unwrap_or_else(|e| e.into_inner());
        let (r, g, b) = ed.theme_color("gui-bg");
        Color32::from_rgb(r, g, b)
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, bg_color);

    let font_id = FontId::new(line_h * 0.85, egui::FontFamily::Monospace);
    let render_rows = surface.height as usize - surface_y_offset;

    for sy in surface_y_offset..surface_y_offset + render_rows {
        let dy = (sy - surface_y_offset) as f32 * line_h;
        let y = rect.min.y + dy;

        for sx in 0..surface.width as usize {
            let Some(cell) = surface.cell(sx as u16, sy as u16) else { continue };
            let cx = rect.min.x + sx as f32 * char_w;
            let cell_rect = Rect::from_min_size(
                Pos2::new(cx, y),
                Vec2::new(char_w, line_h),
            );

            let cell_bg = Color32::from_rgb(cell.style.bg.0, cell.style.bg.1, cell.style.bg.2);
            if cell_bg != bg_color {
                painter.rect_filled(cell_rect, 0.0, cell_bg);
            }

            let fg = Color32::from_rgb(cell.style.fg.0, cell.style.fg.1, cell.style.fg.2);
            if cell.ch != ' ' && cell.ch != '\0' {
                let ch_str: String = cell.ch.into();
                painter.text(
                    Pos2::new(cx, y),
                    Align2::LEFT_TOP,
                    &ch_str,
                    font_id.clone(),
                    fg,
                );
            }
        }
    }
}
