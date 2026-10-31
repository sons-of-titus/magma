//! The insert-mode completion popup — item list plus an info pane for the
//! selected candidate (signature line and documentation, as much of the
//! server's payload as the pane can hold).

use crate::render::surface::{Style, Surface};
use crate::state::Editor;

const MAX_WIDTH: usize = 90;
const DOC_LINES: usize = 4;

/// Wrap `text` to `width` display cells on whitespace; over-long words are
/// hard-truncated.  At most `max` lines.
fn wrap(text: &str, width: usize, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut line_cells = 0usize;
    for word in text.split_whitespace() {
        let w = word.chars().count();
        if w > width {
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
                line_cells = 0;
            }
            if out.len() < max {
                out.push(word.chars().take(width).collect());
            }
            continue;
        }
        if line_cells + w + usize::from(!line.is_empty()) > width {
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
                line_cells = 0;
            }
            if out.len() >= max {
                break;
            }
        }
        if !line.is_empty() {
            line.push(' ');
            line_cells += 1;
        }
        line.push_str(word);
        line_cells += w;
    }
    if !line.is_empty() && out.len() < max {
        out.push(line);
    }
    out
}

/// The selected item's signature line and its documentation text.  When the
/// server sent no `detail`, the signature falls back to the docs' first
/// line — and the docs pane skips it, so it is never shown twice.
fn selected_info(editor: &Editor) -> (String, String) {
    let Some(item) = editor.completion.items.get(editor.completion.idx) else {
        return (String::new(), String::new());
    };
    let signature = if item.detail.is_empty() {
        item.docs.lines().next().unwrap_or_default().to_string()
    } else {
        item.detail.clone()
    };
    let docs = if item.detail.is_empty() {
        item.docs.lines().skip(1).collect::<Vec<_>>().join(" ")
    } else {
        item.docs.replace('\n', " ")
    };
    (signature, docs)
}

/// Draw the completion popup (top-right of buffer area).  Each row shows
/// the label and kind; under the list an info pane shows the selected
/// item's signature and up to `DOC_LINES` wrapped documentation lines.
pub fn render(editor: &Editor, surface: &mut Surface, row_offset: u16, visible_lines: usize) {
    if !editor.completion.visible || editor.completion.items.is_empty() {
        return;
    }
    let (signature, docs) = selected_info(editor);
    let doc_rows = wrap(&docs, MAX_WIDTH, DOC_LINES);
    let footer_rows = usize::from(!signature.is_empty()) + doc_rows.len();
    let popup_lines = editor.completion.items.len().min(10);

    let popup_width = editor
        .completion
        .items
        .iter()
        .map(|i| crate::completion::row_cells(editor, &i.label, &i.kind))
        .chain(std::iter::once(signature.chars().count()))
        .chain(doc_rows.iter().map(|l| l.chars().count()))
        .max()
        .unwrap_or(20)
        .min(MAX_WIDTH);
    let footer = footer_rows > 0;
    let total_rows = popup_lines + footer_rows;
    let popup_start = visible_lines.saturating_sub(total_rows + 1);
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
        // The selected row carries its signature after the label, dimmed —
        // the info the bare label leaves out.
        if is_sel && !signature.is_empty() {
            let label_cells = text.chars().count();
            if popup_width > label_cells + 2 {
                let room = popup_width - label_cells - 2;
                let dim = Style {
                    fg: (150, 170, 220),
                    bg,
                    ..Default::default()
                };
                let tail: String = signature.chars().take(room).collect();
                surface.set_text(
                    popup_x + 1 + label_cells as u16 + 1,
                    surf_row,
                    &tail,
                    Some(dim),
                );
            }
        }
    }
    if footer {
        let s = Style {
            fg: (150, 200, 150),
            bg: (25, 25, 25),
            ..Default::default()
        };
        let mut info: Vec<String> = Vec::new();
        if !signature.is_empty() {
            info.push(signature.chars().take(popup_width).collect());
        }
        info.extend(doc_rows);
        for (n, line) in info.iter().enumerate() {
            let surf_row = row_offset + (popup_start + popup_lines + n) as u16;
            if surf_row >= row_offset + visible_lines as u16 {
                break;
            }
            for x in 0..popup_width + 2 {
                let sx = popup_x + x as u16;
                if sx < surface.width {
                    surface.set_cell(sx, surf_row, ' ', Some(s));
                }
            }
            surface.set_text(popup_x + 1, surf_row, line, Some(s));
        }
    }
}
