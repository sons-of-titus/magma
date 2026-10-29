//! The insert-mode completion popup — item list plus a signature footer for
//! the selected candidate.

use crate::render::surface::{Style, Surface};
use crate::state::Editor;

/// Draw the completion popup (top-right of buffer area).  Under the list a
/// footer line shows the selected item's signature (`detail`, or its
/// documentation when the server sent no signature).
pub fn render(editor: &Editor, surface: &mut Surface, row_offset: u16, visible_lines: usize) {
    if !editor.completion.visible || editor.completion.items.is_empty() {
        return;
    }
    let popup_lines = editor.completion.items.len().min(10);
    let selected = editor.completion.items.get(editor.completion.idx);
    let detail: String = selected
        .map(|i| {
            if i.detail.is_empty() {
                i.docs.lines().next().unwrap_or_default().to_string()
            } else {
                i.detail.clone()
            }
        })
        .unwrap_or_default();
    let footer = !detail.is_empty();
    let total_rows = popup_lines + usize::from(footer);
    let popup_start = visible_lines.saturating_sub(total_rows + 1);
    let popup_width = editor
        .completion
        .items
        .iter()
        .map(|i| crate::completion::row_cells(editor, &i.label, &i.kind))
        .chain(std::iter::once(detail.chars().count()))
        .max()
        .unwrap_or(20)
        .min(60);
    let popup_x = surface.width.saturating_sub(popup_width as u16 + 2);
    for i in 0..popup_lines {
        let abs_row = popup_start + i;
        if abs_row >= visible_lines || i >= editor.completion.items.len() {
            break;
        }
        let item = &editor.completion.items[i];
        let is_sel = i == editor.completion.idx;
        let bg = if is_sel { (80, 80, 180) } else { (40, 40, 40) };
        let fg = if is_sel {
            (255, 255, 255)
        } else {
            (200, 200, 200)
        };
        let s = Style {
            fg,
            bg,
            ..Default::default()
        };
        let surf_row = row_offset + abs_row as u16;
        for x in 0..popup_width + 2 {
            let sx = popup_x + x as u16;
            if sx < surface.width {
                surface.set_cell(sx, surf_row, ' ', Some(s));
            }
        }
        let text = if item.kind.is_empty() {
            item.label.clone()
        } else {
            format!("{} {}", item.label, item.kind)
        };
        surface.set_text(popup_x + 1, surf_row, &text, Some(s));
    }
    if footer {
        let surf_row = row_offset + (popup_start + popup_lines) as u16;
        if surf_row < row_offset + visible_lines as u16 {
            let s = Style {
                fg: (150, 200, 150),
                bg: (25, 25, 25),
                ..Default::default()
            };
            for x in 0..popup_width + 2 {
                let sx = popup_x + x as u16;
                if sx < surface.width {
                    surface.set_cell(sx, surf_row, ' ', Some(s));
                }
            }
            let truncated: String = detail.chars().take(popup_width).collect();
            surface.set_text(popup_x + 1, surf_row, &truncated, Some(s));
        }
    }
}
