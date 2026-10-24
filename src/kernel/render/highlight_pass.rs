//! Highlight and selection rendering passes for the buffer content area.
//!
//! Both walks consume the row-layout cache (R-12) — the same
//! grapheme→cell table the text pass painted from — so a highlight column
//! can never disagree with the painted glyph column, and no pass
//! re-segments the row.

use crate::kernel::render::row_layout::RowLayout;
use crate::kernel::text_engine::BufferView;
use crate::kernel::state::Editor;
use crate::kernel::render::surface::{Style, Surface};

/// Restyle `w` cells starting at column `col` with `style`, stamping the
/// grapheme's lead char on the first cell (already there from the text
/// pass, but the restyle must not drop it) and blanks on trailing cells.
fn paint_cells(surface: &mut Surface, content_x: u16, surf_row: u16, col: usize, w: usize,
               lead: char, style: Style) {
    for k in 0..w {
        let sx = content_x + (col + k) as u16;
        if sx < surface.width {
            let ch = if k == 0 { lead } else { ' ' };
            surface.set_cell(sx, surf_row, ch, Some(style));
        }
    }
}

/// Apply all highlight layers (base → syntax → semantic → search → custom) and
/// the visual selection highlight onto `surface`.
pub fn apply_highlights(
    editor: &Editor,
    view: Option<&BufferView>,
    surface: &mut Surface,
    rows: &[(usize, usize, &str)],
    layouts: &[std::sync::Arc<RowLayout>],
    _scroll_top: usize,
    _max_visible: usize,
    content_x: u16,
    row_offset: u16,
    cursor_offset: usize,
) {
    let layer_order: [&str; 5] = ["base", "syntax", "semantic", "search", "selection"];
    let empty_vec = Vec::new();
    let empty_map = std::collections::HashMap::new();
    let highlights = view.map(|v| &v.highlights).unwrap_or(&empty_vec);
    let highlight_layers = view.map(|v| &v.highlight_layers).unwrap_or(&empty_map);

    // Borrowed, in layer order.  No per-frame clones: syntax layers can hold
    // thousands of ranges, and this runs every repaint.
    let mut all_highlights: Vec<(usize, usize, &str)> = Vec::new();
    for (s, e, f) in highlights { all_highlights.push((*s, *e, f)); }
    for layer_name in &layer_order {
        if let Some(layer) = highlight_layers.get(*layer_name) {
            for (s, e, f) in layer { all_highlights.push((*s, *e, f)); }
        }
    }
    for (layer_name, layer) in highlight_layers {
        if !layer_order.contains(&layer_name.as_str()) {
            for (s, e, f) in layer { all_highlights.push((*s, *e, f)); }
        }
    }

    if !all_highlights.is_empty() {
        let row_ranges: Vec<(usize, usize)> =
            rows.iter().map(|(s, e, _)| (*s, *e)).collect();

        let visible_byte_start = row_ranges.first().map(|(s, _)| *s).unwrap_or(0);
        let visible_byte_end   = row_ranges.last().map(|(_, e)| *e).unwrap_or(0);
        let fallback_style = Style {
            fg: editor.theme_color("highlight-fg"),
            bg: editor.theme_color("highlight-bg"),
            ..Default::default()
        };

        for &(hl_start, hl_end, face_name) in &all_highlights {
            if hl_end <= visible_byte_start || hl_start >= visible_byte_end { continue; }
            let style = editor.resolve_face_style(face_name).unwrap_or(fallback_style);

            for (vis_row, (line_start, line_end)) in row_ranges.iter().enumerate() {
                if hl_end <= *line_start { break; }
                if hl_start >= *line_end { continue; }
                let surf_row = row_offset + vis_row as u16;
                let Some(layout) = layouts.get(vis_row) else { continue };
                for i in 0..layout.widths.len() {
                    let byte = *line_start + layout.offsets[i] as usize;
                    if byte >= hl_end { break; }
                    if byte < hl_start { continue; }
                    paint_cells(surface, content_x, surf_row,
                        layout.cols[i] as usize, layout.widths[i] as usize,
                        layout.leads[i], style);
                }
            }
        }
    }

    // Visual-selection highlight — row text already includes its newline,
    // so the byte cursor advances exactly once per row terminator.
    let sel = editor.selection_range(cursor_offset);
    if let Some((sel_start, sel_end)) = sel {
        let sel_fg = editor.theme_color("selection-fg");
        let sel_bg = editor.theme_color("selection-bg");
        let sel_style = Style { fg: sel_fg, bg: sel_bg, ..Default::default() };
        for (vis_row, (line_start, line_end, _text)) in rows.iter().enumerate() {
            let surf_row = row_offset + vis_row as u16;
            let Some(layout) = layouts.get(vis_row) else { continue };
            for i in 0..layout.widths.len() {
                let byte = *line_start + layout.offsets[i] as usize;
                if byte >= *line_end { break; }
                if byte >= sel_start && byte < sel_end {
                    paint_cells(surface, content_x, surf_row,
                        layout.cols[i] as usize, layout.widths[i] as usize,
                        layout.leads[i], sel_style);
                }
            }
            if *line_end > sel_end { break; }
        }
    }
}
