//! Buffer decoration render pass.
//!
//! Called from `render_frame` after all text and highlights have been drawn.

use crate::render::editor_view::VisibleRow;
use crate::render::host::RenderHost;
use crate::render::row_layout::RowLayout;
use crate::render::surface::{Style, Surface};
use crate::state::Editor;
use crate::text_engine::Decoration;

/// Compute the width of the widest `LinePrefix` decoration across all layers
/// for the given buffer view.  Returns 0 when no prefix decorations exist.
pub fn prefix_margin_width(editor: &dyn RenderHost, buf_id: usize) -> usize {
    let view = match editor.view_of_buffer(buf_id) {
        Some(v) => v,
        None => return 0,
    };
    view.decoration_layers
        .values()
        .flat_map(|v| v.iter())
        .filter_map(|d| match d {
            Decoration::LinePrefix { text, .. } => Some(text.chars().count()),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// Display row holding buffer `line`, or `None` when the line is scrolled
/// out or collapsed inside a fold (only a fold's first line keeps its
/// decorations).  Rows are viewport-bounded, so the scan is bounded by the
/// viewport height, never the document.
fn locate_row(rows: &[VisibleRow], line: usize) -> Option<usize> {
    rows.iter().position(|r| r.line == line)
}

/// Render all decoration layers for the focused buffer.
#[allow(clippy::too_many_arguments)]
pub fn render_decorations(
    editor: &Editor,
    buf_id: usize,
    surface: &mut Surface,
    rows: &[VisibleRow],
    row_layouts: &[std::sync::Arc<RowLayout>],
    prefix_margin: usize,
    gutter_width: usize,
    content_x: u16,
    row_offset: u16,
    scroll_left: usize,
) {
    let view = match editor.view_of_buffer(buf_id) {
        Some(v) => v,
        None => return,
    };
    if view.decoration_layers.is_empty() {
        return;
    }

    // The prefix margin occupies surface columns [area.x, area.x +
    // prefix_cols), i.e. everything left of the gutter providers.
    let prefix_x = content_x.saturating_sub(gutter_width as u16);

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
                    let Some(row_idx) = locate_row(rows, *line) else {
                        continue;
                    };
                    let surf_row = row_offset + row_idx as u16;
                    let style = editor.resolve_face_style(face);
                    let max_len = prefix_margin.min(text.chars().count());
                    let drawn: String = text.chars().take(max_len).collect();
                    surface.set_text(prefix_x, surf_row, &drawn, style);
                }
                Decoration::InlineText {
                    line,
                    col,
                    text,
                    face,
                } => {
                    let Some(row_idx) = locate_row(rows, *line) else {
                        continue;
                    };
                    if *col < scroll_left {
                        continue;
                    }
                    let surf_row = row_offset + row_idx as u16;
                    let sx = content_x + (*col - scroll_left) as u16;
                    if sx >= surface.width {
                        continue;
                    }
                    let style = editor.resolve_face_style(face);
                    let max_cols = (surface.width - sx) as usize;
                    let drawn: String = text.chars().take(max_cols).collect();
                    surface.set_text(sx, surf_row, &drawn, style);
                }
                Decoration::EndOfLine { line, text, face } => {
                    let Some(row_idx) = locate_row(rows, *line) else {
                        continue;
                    };
                    let surf_row = row_offset + row_idx as u16;
                    // End column in display cells from the row layout — a
                    // char count misplaces the annotation under tabs and
                    // wide glyphs.
                    let end_col = row_layouts
                        .get(row_idx)
                        .map(|l| l.cells())
                        .unwrap_or_default();
                    if end_col < scroll_left {
                        continue;
                    }
                    let sx = content_x + (end_col - scroll_left + 1) as u16;
                    if sx >= surface.width {
                        continue;
                    }
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
pub fn render_overlays(
    host: &dyn RenderHost,
    overlays: &[crate::state::Overlay],
    overlay_bg: (u8, u8, u8),
    overlay_fg: (u8, u8, u8),
    surface: &mut Surface,
) {
    if overlays.is_empty() {
        return;
    }

    let mut sorted: Vec<&crate::state::Overlay> = overlays.iter().collect();
    sorted.sort_by_key(|o| o.z_order);

    for ov in sorted {
        let bg_style = Style {
            fg: overlay_fg,
            bg: overlay_bg,
            ..Default::default()
        };
        for dy in 0..ov.height {
            let sy = ov.y + dy;
            if sy >= surface.height {
                break;
            }
            for dx in 0..ov.width {
                let sx = ov.x + dx;
                if sx >= surface.width {
                    break;
                }
                surface.set_cell(sx, sy, ' ', Some(bg_style));
            }
        }

        if let Some(buf_id) = ov.buffer_id
            && let Some(arc) = host.buffer(buf_id)
        {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            // Only the rows the overlay can show are extracted.
            let max_rows = ov.height as usize;
            let max_cols = ov.width as usize;
            for (row, line) in (0..max_rows).filter_map(|r| buf.line(r)).enumerate() {
                let sy = ov.y + row as u16;
                if sy >= surface.height {
                    break;
                }
                for (col, ch) in line.chars().enumerate().take(max_cols) {
                    let sx = ov.x + col as u16;
                    if sx >= surface.width {
                        break;
                    }
                    surface.set_cell(sx, sy, ch, Some(bg_style));
                }
            }
        }
    }
}
