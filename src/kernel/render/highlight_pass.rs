//! Highlight and selection rendering passes for the buffer content area.

use crate::kernel::text_engine::BufferView;
use crate::kernel::state::Editor;
use crate::kernel::render::surface::{Style, Surface};
use crate::kernel::text_engine::text_layout::{grapheme_cell_width, grapheme_iter};

/// One row's cell-mapping state: advance over graphemes exactly the way
/// the text pass places them (tabs to stops, wide glyphs two cells), so a
/// highlight column can never disagree with the painted glyph column.
struct CellWalk {
    byte: usize,
    col: usize,
}

fn advance(walk: &mut CellWalk, g: &str, tab_w: usize,
           glyph_widths: &std::collections::HashMap<char, u8>) -> usize {
    let w = grapheme_cell_width(g, tab_w, walk.col, glyph_widths);
    walk.col += w;
    walk.byte += g.len();
    w
}

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

    let tab_w = super::frame::int_option(editor, "tab-width", 4);
    let glyph_widths = &editor.font_config.glyph_widths;

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
                let Some((_, _, line_text)) = rows.get(vis_row) else { continue };
                let line_text: &str = line_text;
                let mut walk = CellWalk { byte: *line_start, col: 0 };
                for g in grapheme_iter(line_text) {
                    if walk.byte >= hl_end { break; }
                    let col = walk.col;
                    if walk.byte >= hl_start {
                        let lead = g.chars().next().unwrap_or(' ');
                        paint_cells(surface, content_x, surf_row, col,
                            advance(&mut walk, g, tab_w, glyph_widths), lead, style);
                    } else {
                        advance(&mut walk, g, tab_w, glyph_widths);
                    }
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
        let mut walk = CellWalk { byte: rows.first().map(|(s, _, _)| *s).unwrap_or(0), col: 0 };
        for (vis_row, (_line_start, _, line_text)) in rows.iter().enumerate() {
            let surf_row = row_offset + vis_row as u16;
            for g in grapheme_iter(line_text) {
                let start_byte = walk.byte;
                let col = walk.col;
                let w = advance(&mut walk, g, tab_w, glyph_widths);
                if start_byte >= sel_start && start_byte < sel_end {
                    let lead = g.chars().next().unwrap_or(' ');
                    paint_cells(surface, content_x, surf_row, col, w, lead, sel_style);
                }
            }
            if walk.byte > sel_end { break; }
        }
    }
}
