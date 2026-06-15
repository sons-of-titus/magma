//! Highlight and selection rendering passes for the buffer content area.

use crate::buffer::Buffer;
use crate::state::Editor;
use crate::render::surface::{Style, Surface};


/// Apply all highlight layers (base → syntax → semantic → search → custom) and
/// the visual selection highlight onto `surface`.
pub fn apply_highlights(
    editor: &Editor,
    buf: &Buffer,
    surface: &mut Surface,
    text: &str,
    scroll_top: usize,
    max_visible: usize,
    content_x: u16,
    row_offset: u16,
) {
    let layer_order: [&str; 5] = ["base", "syntax", "semantic", "search", "selection"];
    let all_highlights: Vec<(usize, usize, String)> = {
        let mut items = Vec::new();
        for (s, e, f) in &buf.highlights { items.push((*s, *e, f.clone())); }
        for layer_name in &layer_order {
            if let Some(layer) = buf.highlight_layers.get(*layer_name) {
                for (s, e, f) in layer { items.push((*s, *e, f.clone())); }
            }
        }
        for (layer_name, layer) in &buf.highlight_layers {
            if !layer_order.contains(&layer_name.as_str()) {
                for (s, e, f) in layer { items.push((*s, *e, f.clone())); }
            }
        }
        items
    };

    if !all_highlights.is_empty() {
        let row_ranges: Vec<(usize, usize)> = (scroll_top..scroll_top + max_visible)
            .map(|row| {
                let ls = buf.line_start_offset(row).unwrap_or(text.len());
                let le = buf.line_start_offset(row + 1).unwrap_or(text.len());
                (ls, le)
            })
            .collect();

        let visible_byte_start = row_ranges.first().map(|(s, _)| *s).unwrap_or(0);
        let visible_byte_end   = row_ranges.last().map(|(_, e)| *e).unwrap_or(0);
        let fallback_style = Style {
            fg: editor.theme_color("highlight-fg"),
            bg: editor.theme_color("highlight-bg"),
            ..Default::default()
        };

        for (hl_start, hl_end, face_name) in &all_highlights {
            if *hl_end <= visible_byte_start || *hl_start >= visible_byte_end { continue; }
            let style = editor.resolve_face_style(face_name).unwrap_or(fallback_style);

            for (vis_row, (line_start, line_end)) in row_ranges.iter().enumerate() {
                if *hl_end <= *line_start { break; }
                if *hl_start >= *line_end { continue; }
                let surf_row = row_offset + vis_row as u16;
                let line_text = &text[*line_start..*line_end];
                let mut byte = *line_start;
                for (col, ch) in line_text.chars().enumerate() {
                    if byte >= *hl_end { break; }
                    if byte >= *hl_start {
                        let sx = content_x + col as u16;
                        if sx < surface.width {
                            surface.set_cell(sx, surf_row, ch, Some(style));
                        }
                    }
                    byte += ch.len_utf8();
                }
            }
        }
    }

    // Visual-selection highlight
    let sel = editor.selection_range(buf.cursor());
    if let Some((sel_start, sel_end)) = sel {
        let total_lines = buf.line_count();
        let mut byte = 0usize;
        'outer: for row in scroll_top..total_lines {
            let vis_row = row.saturating_sub(scroll_top);
            if vis_row >= max_visible { break; }
            let surf_row = row_offset + vis_row as u16;
            let line_start = buf.line_start_offset(row).unwrap_or(text.len());
            let line_end = buf.line_start_offset(row + 1).unwrap_or(text.len());
            let line_text = &text[line_start..line_end];
            for (col, ch) in line_text.chars().enumerate() {
                if byte >= sel_start && byte < sel_end {
                    let sx = content_x + col as u16;
                    if sx < surface.width {
                        let sel_fg = editor.theme_color("selection-fg");
                        let sel_bg = editor.theme_color("selection-bg");
                        surface.set_cell(sx, surf_row, ch,
                            Some(Style { fg: sel_fg, bg: sel_bg, ..Default::default() }));
                    }
                }
                byte += ch.len_utf8();
            }
            byte += 1;
            if byte > sel_end { break 'outer; }
        }
    }
}
