//! Buffer decoration render pass.
//!
//! Called from `render_frame` after all text and highlights have been drawn.

use crate::kernel::text_engine::{Buffer, Decoration};
use crate::kernel::state::Editor;
use crate::kernel::render::surface::{Style, Surface};

/// Compute the width of the widest `LinePrefix` decoration across all layers
/// for the given buffer view.  Returns 0 when no prefix decorations exist.
pub fn prefix_margin_width(editor: &Editor, buf_id: usize) -> usize {
    let view = match editor.views.get(&buf_id) {
        Some(v) => v,
        None => return 0,
    };
    view.decoration_layers.values()
        .flat_map(|v| v.iter())
        .filter_map(|d| match d {
            Decoration::LinePrefix { text, .. } => Some(text.chars().count()),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// Render all decoration layers for the focused buffer.
pub fn render_decorations(
    editor: &Editor,
    buf_id: usize,
    buf: &Buffer,
    surface: &mut Surface,
    scroll_top: usize,
    visible_lines: usize,
    prefix_margin: usize,
    gutter_width: usize,
    content_x: u16,
    row_offset: u16,
) {
    let view = match editor.views.get(&buf_id) {
        Some(v) => v,
        None => return,
    };
    if view.decoration_layers.is_empty() { return; }

    // Collect sorted layer names for deterministic render order.
    let mut layer_names: Vec<&str> = view.decoration_layers.keys().map(|s| s.as_str()).collect();
    layer_names.sort_unstable();

    for layer_name in layer_names {
        let layer = match view.decoration_layers.get(layer_name) {
            Some(v) => v,
            None => continue,
        };
        for decor in layer {
            match decor {
                Decoration::LinePrefix { line, text, face } => {
                    if *line < scroll_top || *line >= scroll_top + visible_lines { continue; }
                    let surf_row = row_offset + (*line - scroll_top) as u16;
                    let style = editor.resolve_face_style(face);
                    let prefix_x = gutter_width as u16;
                    let max_len = prefix_margin.min(text.chars().count());
                    let drawn: String = text.chars().take(max_len).collect();
                    surface.set_text(prefix_x, surf_row, &drawn, style);
                }
                Decoration::InlineText { line, col, text, face } => {
                    if *line < scroll_top || *line >= scroll_top + visible_lines { continue; }
                    let surf_row = row_offset + (*line - scroll_top) as u16;
                    let sx = content_x + *col as u16;
                    if sx >= surface.width { continue; }
                    let style = editor.resolve_face_style(face);
                    let max_cols = (surface.width - sx) as usize;
                    let drawn: String = text.chars().take(max_cols).collect();
                    surface.set_text(sx, surf_row, &drawn, style);
                }
                Decoration::EndOfLine { line, text, face } => {
                    if *line < scroll_top || *line >= scroll_top + visible_lines { continue; }
                    let surf_row = row_offset + (*line - scroll_top) as u16;
                    let line_text = buf.line(*line).unwrap_or_default();
                    let eol_col = content_x as usize + line_text.chars().count();
                    let sx = (eol_col + 1) as u16;
                    if sx >= surface.width { continue; }
                    let style = editor.resolve_face_style(face);
                    let max_cols = (surface.width - sx) as usize;
                    let drawn: String = text.chars().take(max_cols).collect();
                    surface.set_text(sx, surf_row, &drawn, style);
                }
            }
        }
    }
}

/// Render overlays drawn above buffer content in z_order.
pub fn render_overlays(editor: &Editor, surface: &mut Surface) {
    if editor.overlays.is_empty() { return; }

    let mut sorted: Vec<&crate::kernel::state::Overlay> = editor.overlays.iter().collect();
    sorted.sort_by_key(|o| o.z_order);

    let overlay_bg = editor.theme_color("bg");
    let overlay_fg = editor.theme_color("fg");

    for ov in sorted {
        let bg_style = Style {
            fg: overlay_fg,
            bg: overlay_bg,
            ..Default::default()
        };
        for dy in 0..ov.height {
            let sy = ov.y + dy;
            if sy >= surface.height { break; }
            for dx in 0..ov.width {
                let sx = ov.x + dx;
                if sx >= surface.width { break; }
                surface.set_cell(sx, sy, ' ', Some(bg_style));
            }
        }

        if let Some(buf_id) = ov.buffer_id {
            if let Some(arc) = editor.buffers.get(buf_id) {
                let buf = arc.lock().unwrap();
                let text = buf.slice(0, buf.len());
                let lines: Vec<&str> = text.split('\n').collect();
                let max_rows = ov.height as usize;
                let max_cols = ov.width as usize;
                for (row, line) in lines.iter().enumerate().take(max_rows) {
                    let sy = ov.y + row as u16;
                    if sy >= surface.height { break; }
                    for (col, ch) in line.chars().enumerate().take(max_cols) {
                        let sx = ov.x + col as u16;
                        if sx >= surface.width { break; }
                        surface.set_cell(sx, sy, ch, Some(bg_style));
                    }
                }
            }
        }
    }
}
