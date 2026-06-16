//! egui surface painter and status bar for the GUI backend.

use std::sync::{Arc, RwLock};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

use crate::kernel::render::surface::Surface;
use crate::kernel::state::Editor;

use super::gui_fonts::mode_color;

pub fn safe_boundary(s: &str, mut offset: usize) -> usize {
    offset = offset.min(s.len());
    while offset > 0 && !s.is_char_boundary(offset) { offset -= 1; }
    offset
}

/// Render a `Surface` onto an egui `Rect`.
///
/// The last row of the surface is assumed to be the status bar and is
/// not drawn here (handled by `draw_status` instead).
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
    let render_rows = surface.height as usize - surface_y_offset - 1;

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

pub fn draw_status(
    ui: &egui::Ui,
    editor_ref: &Arc<RwLock<Editor>>,
    rect: Rect,
    font_id: &FontId,
    line_h: f32,
) {
    let ed = editor_ref.read().unwrap_or_else(|e| e.into_inner());
    let mode = ed.vim_mode_name();
    let bg   = mode_color(&mode);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, bg);

    let text = if let Some(ref mb) = ed.editor_mode.minibuffer {
        if mb.prompt == ":" {
            format!(":{}", mb.input)
        } else {
            format!("{}{}", mb.prompt, mb.input)
        }
    } else if let Some(slab) = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
    {
        if let Some(buf_arc) = ed.buffers.get(slab) {
            let buf     = buf_arc.lock().unwrap();
            let cursor  = ed.views.get(&slab).map(|v| v.cursor_offset()).unwrap_or(0);
            let content = buf.slice(0, buf.len());
            let safe    = safe_boundary(&content, cursor);
            let before  = &content[..safe];
            let row     = before.chars().filter(|&c| c == '\n').count() + 1;
            let last    = before.rfind('\n').map(|p| &before[p + 1..]).unwrap_or(before);
            let col     = last.chars().count() + 1;
            let dirty   = if buf.modified() { " [+]" } else { "" };

            let sel_info = if let Some((s, e)) = ed.selection_range(cursor) {
                let chars = content.get(s..e).map(|t| t.chars().count()).unwrap_or(0);
                format!("  ({} chars selected)", chars)
            } else {
                String::new()
            };
            let name = buf.name.clone();
            drop(buf);

            format!("  {}  {}{}{}   Ln {}, Col {}", mode, name, dirty, sel_info, row, col)
        } else {
            format!("  {}  No buffer", mode)
        }
    } else {
        format!("  {}", mode)
    };

    let y = rect.min.y + (rect.height() - line_h) * 0.5;
    painter.text(Pos2::new(rect.min.x + 8.0, y), Align2::LEFT_TOP, &text, font_id.clone(), Color32::WHITE);

    if mode == "COMMAND" || mode == "SEARCH" {
        let text_w = ui.fonts(|f| f.glyph_width(font_id, ' ') * text.chars().count() as f32);
        let cx = rect.min.x + 8.0 + text_w;
        let cw = ui.fonts(|f| f.glyph_width(font_id, ' '));
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(cx, y), Vec2::new(cw, line_h)),
            0.0, Color32::WHITE,
        );
    }
}
