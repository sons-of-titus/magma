//! Cursor-adjacent viewport decorations: bracket matching and indent guides.
//!
//! Split from `editor_view.rs` (file-size gate); both features read the same
//! visible rows + row layouts the highlight pass uses, so they never re-scan
//! the document and share one coordinate system with the text.

use crate::render::editor_view::VisibleRow;
use crate::render::row_layout::RowLayout;
use crate::render::surface::{Style, Surface};
use crate::state::Editor;
use crate::text_engine::Buffer;

/// Bracket pairs, in scan order.  `open` marks the inward direction.
const PAIRS: [(char, char, bool); 3] = [('(', ')', true), ('[', ']', true), ('{', '}', true)];

/// The character under or directly before the cursor, if it opens or closes
/// a bracket pair, plus the offset of its partner — a bounded scan outward
/// from the cursor so pathological buffers can't stall the frame.
pub fn bracket_pair(buf: &Buffer, cursor: usize) -> Option<(usize, usize)> {
    let before = cursor.checked_sub(1);
    for at in [before, Some(cursor.min(buf.len()))].into_iter().flatten() {
        let Some(ch) = buf.char_at(at) else {
            continue;
        };
        let Some((open, close, _)) = PAIRS.iter().find(|(o, c, _)| *o == ch || *c == ch) else {
            continue;
        };
        let found = if ch == *open {
            scan_forward(buf, at, *open, *close)
        } else {
            scan_backward(buf, at, *close, *open)
        };
        if let Some(other) = found {
            return Some((at, other));
        }
    }
    None
}

fn scan_forward(buf: &Buffer, from: usize, open: char, close: char) -> Option<usize> {
    let limit = buf.len().min(from + 10_000);
    let mut depth = 0i32;
    let mut off = from;
    while off < limit {
        if let Some(c) = buf.char_at(off) {
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    return Some(off);
                }
            }
        }
        off += buf.char_at(off).map(|c| c.len_utf8()).unwrap_or(1);
    }
    None
}

fn scan_backward(buf: &Buffer, from: usize, close: char, open: char) -> Option<usize> {
    let limit = from.saturating_sub(10_000);
    let mut depth = 0i32;
    let mut off = from;
    while off >= limit {
        if let Some(c) = buf.char_at(off) {
            if c == close {
                depth += 1;
            } else if c == open {
                depth -= 1;
                if depth == 0 {
                    return Some(off);
                }
            }
        }
        if off == 0 {
            break;
        }
        // Walk back one full char: rope slices panic on non-boundaries.
        off = buf
            .char_at(off)
            .and_then(|c| off.checked_sub(c.len_utf8()))
            .filter(|o| *o >= limit)
            .unwrap_or(limit);
    }
    None
}

/// Byte offset → (visible row index, display column), via the shared rows
/// and layouts.
fn locate(
    rows: &[VisibleRow],
    row_layouts: &[std::sync::Arc<RowLayout>],
    offset: usize,
) -> Option<(usize, usize)> {
    for (i, r) in rows.iter().enumerate() {
        if r.start <= offset && offset < r.end {
            let layout = row_layouts.get(i)?;
            return Some((i, layout.visual_col_of(offset - r.start)));
        }
    }
    None
}

/// Highlight the bracket pair around the cursor: restyle both cells with the
/// `bracket-match` background without touching their glyphs.  Pairs scrolled
/// out of the viewport are skipped silently.
#[allow(clippy::too_many_arguments)]
pub fn render_bracket_match(
    editor: &Editor,
    buf: &Buffer,
    surface: &mut Surface,
    rows: &[VisibleRow],
    row_layouts: &[std::sync::Arc<RowLayout>],
    cursor: usize,
    scroll_left: usize,
    content_x: u16,
    row_offset: u16,
) {
    let Some((a, b)) = bracket_pair(buf, cursor) else {
        return;
    };
    let (br, bg, bb) = editor.theme_color("bracket-match");
    for offset in [a, b] {
        let Some((row, col)) = locate(rows, row_layouts, offset) else {
            continue;
        };
        if col < scroll_left {
            continue;
        }
        let sx = content_x + (col - scroll_left) as u16;
        let sy = row_offset + row as u16;
        if let Some(cell) = surface.cell(sx, sy) {
            let style = Style {
                bg: (br, bg, bb),
                ..cell.style
            };
            surface.set_cell(sx, sy, cell.ch, Some(style));
        }
    }
}

/// Vertical guides at each indent level of non-blank lines, drawn only in
/// whitespace so they never cover a glyph.  On by default; `set
/// indent-guides false` opts out.
#[allow(clippy::too_many_arguments)]
pub fn render_indent_guides(
    editor: &Editor,
    surface: &mut Surface,
    rows: &[VisibleRow],
    row_layouts: &[std::sync::Arc<RowLayout>],
    scroll_left: usize,
    content_x: u16,
    row_offset: u16,
    max_col: usize,
    tab_width: usize,
) {
    if tab_width == 0 {
        return;
    }
    let (gr, gg, gb) = editor.theme_color("indent-guide");
    for (i, r) in rows.iter().enumerate() {
        if r.fold {
            continue;
        }
        let Some(layout) = row_layouts.get(i) else {
            continue;
        };
        // First non-blank cell = the line's indent, in display columns.
        let Some(indent) = layout
            .leads
            .iter()
            .position(|c| !c.is_whitespace())
            .map(|idx| layout.cols[idx] as usize)
        else {
            continue;
        };
        if indent < tab_width {
            continue;
        }
        let sy = row_offset + i as u16;
        let mut guide = tab_width;
        while guide < indent {
            if guide >= scroll_left {
                let sx = content_x + (guide - scroll_left) as u16;
                if let Some(cell) = surface.cell(sx, sy)
                    && cell.ch == ' '
                {
                    let style = Style {
                        fg: (gr, gg, gb),
                        bg: cell.style.bg,
                        ..Default::default()
                    };
                    surface.set_cell(sx, sy, '│', Some(style));
                }
            }
            if guide >= max_col {
                break;
            }
            guide += tab_width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::disk::DiskFileSystem;
    use id::BufferId;

    fn buf(text: &str) -> Buffer {
        Buffer::from_string(BufferId(1), "t", text)
    }

    #[test]
    fn bracket_pair_finds_the_partner_before_and_at_the_cursor() {
        let b = buf("fn f() {\n    x\n}\n");
        // Cursor right after the ')' at byte 5 (cursor sits at byte 6).
        assert_eq!(bracket_pair(&b, 6), Some((5, 4)));
        // Cursor on the '}' at line 3; its '{' is at byte 7.
        let close = b.line_start_offset(2).unwrap();
        assert_eq!(bracket_pair(&b, close), Some((close, 7)));
    }

    #[test]
    fn bracket_pair_respects_nesting_and_gives_up_far_away() {
        let text = format!("{}{}", "(".repeat(20_001), ")".repeat(20_001));
        let b = buf(&text);
        // A cursor in the middle of the open run has its partner beyond the
        // 10 KB scan window: no highlight, no stall.
        assert_eq!(bracket_pair(&b, 10_000), None);
        let mut nested = String::from("(((");
        nested.push_str("x");
        nested.push_str(")))");
        let b = buf(&nested);
        // Cursor after the last ')': depth counting skips the inner pairs
        // and lands on the outermost '(' at byte 0.
        assert_eq!(bracket_pair(&b, 7), Some((6, 0)));
    }

    #[test]
    fn non_bracket_cursors_match_nothing() {
        let b = buf("plain text\n");
        assert_eq!(bracket_pair(&b, 2), None);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::command::builtin;
    use crate::render::editor_view::build_visible_rows;
    use crate::storage::disk::DiskFileSystem;

    #[test]
    fn matching_bracket_cells_carry_the_match_background() {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let key = ed.create_buffer_from_str("bm", "fn f() {\n    x\n}\n");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        // Cursor after the ')' at byte 5.
        ed.view_of_buffer_mut(key).unwrap().set_cursor(6);

        let arc = ed.buffers.get(key).unwrap();
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let mut surface = Surface::new(40, 6);
        let rows = build_visible_rows(&buf, &[], 0, 5, buf.line_count());
        let tab_w = 4;
        let row_layouts: Vec<std::sync::Arc<RowLayout>> = rows
            .iter()
            .map(|r| {
                std::sync::Arc::new(RowLayout::build(
                    &r.text,
                    0,
                    tab_w,
                    &ed.font_config.glyph_widths,
                ))
            })
            .collect();
        // Gutter width for a 4-line buffer: digits+3 = 2 → content starts at 2.
        let gutter = ed.gutter.total_width(buf.line_count()) + 1; // + prefix separator
        let content_x = 1 + gutter as u16;
        let (br, bg, bb) = ed.theme_color("bracket-match");
        render_bracket_match(
            &ed,
            &buf,
            &mut surface,
            &rows,
            &row_layouts,
            6,
            0,
            content_x,
            1,
        );

        let hit = (0..surface.height).any(|y| {
            (0..surface.width).any(|x| {
                surface
                    .cell(x, y)
                    .map(|c| c.style.bg == (br, bg, bb))
                    .unwrap_or(false)
            })
        });
        drop(buf);
        assert!(hit, "at least one bracket cell must carry the match style");
    }

    #[test]
    fn indent_guides_draw_only_in_whitespace() {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let key = ed.create_buffer_from_str("ig", "fn f() {\n    let x = 1;\n}\n");
        let arc = ed.buffers.get(key).unwrap();
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let rows = build_visible_rows(&buf, &[], 0, 5, buf.line_count());
        let row_layouts: Vec<std::sync::Arc<RowLayout>> = rows
            .iter()
            .map(|r| std::sync::Arc::new(RowLayout::build(&r.text, 0, 4, &Default::default())))
            .collect();
        let mut surface = Surface::new(40, 6);
        // Paint text first, as the real pass does.
        for (i, r) in rows.iter().enumerate() {
            let sy = (1 + i) as u16;
            for (ci, ch) in r.text.chars().enumerate() {
                surface.set_cell(4 + ci as u16, sy, ch, None);
            }
        }
        render_indent_guides(&ed, &mut surface, &rows, &row_layouts, 0, 4, 1, 40, 4);
        // Line 1 ("    let x = 1;") has indent 4 → exactly one guide, at
        // guide column 0 — wait: guides sit at tab_width multiples strictly
        // inside the indent, i.e. columns 0 only when indent > 4.  With
        // indent == 4 there is no interior column, so no guide draws.
        let guides: Vec<u16> = (0..surface.width)
            .filter(|x| surface.cell(*x, 2).map(|c| c.ch == '│').unwrap_or(false))
            .collect();
        assert!(
            guides.is_empty(),
            "indent == tab_width has no interior guide"
        );

        // A deeper line gets interior guides that stop before any glyph.
        let deep = "        deep();\n";
        let rows2 = vec![VisibleRow {
            start: 0,
            end: deep.len(),
            text: deep.to_string(),
            fold: false,
            line: 0,
        }];
        let layouts2 = vec![std::sync::Arc::new(RowLayout::build(
            deep,
            0,
            4,
            &Default::default(),
        ))];
        let mut surface2 = Surface::new(40, 3);
        for (ci, ch) in deep.chars().enumerate() {
            surface2.set_cell(4 + ci as u16, 1, ch, None);
        }
        render_indent_guides(&ed, &mut surface2, &rows2, &layouts2, 0, 4, 1, 40, 4);
        let guide_cols: Vec<u16> = (0..40)
            .filter(|x| surface2.cell(*x, 1).map(|c| c.ch == '│').unwrap_or(false))
            .collect();
        // Guides at tab multiples strictly inside the indent: column 4 only;
        // column 8 holds the 'd' of deep() and must stay untouched.
        assert_eq!(guide_cols, vec![4 + 4], "one interior guide at column 4");
        assert_ne!(
            surface2.cell(4 + 8, 1).unwrap().ch,
            '│',
            "the 'd' of deep() must never be covered"
        );
    }
}
