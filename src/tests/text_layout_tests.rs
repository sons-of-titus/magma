//! Unicode visual layout (docs/core-redesign.md §10): grapheme boundaries,
//! cell widths, and screen positions through combining marks, ZWJ emoji,
//! CJK, tabs, and multibyte text.

use crate::kernel::text_engine::text_layout::{
    cell_column, floor_grapheme, grapheme_cell_width, grapheme_iter, line_cells, next_grapheme,
    prev_grapheme,
};

fn no_glyphs() -> std::collections::HashMap<char, u8> {
    std::collections::HashMap::new()
}

#[test]
fn combining_mark_stays_with_its_base() {
    // "e" + U+0301 is one grapheme cluster (3 bytes), one cell.
    let text = "e\u{0301}x";
    let clusters: Vec<&str> = grapheme_iter(text).collect();
    assert_eq!(clusters.len(), 2);
    assert_eq!(clusters[0], "e\u{0301}");

    // Left from after the cluster jumps over both bytes.
    assert_eq!(prev_grapheme(text, 3), 0);
    assert_eq!(next_grapheme(text, 0), 3);

    // The cluster renders one cell wide, not two.
    let glyphs = no_glyphs();
    assert_eq!(grapheme_cell_width("e\u{0301}", 4, 0, &glyphs), 1);
    assert_eq!(cell_column(text, 3, 4, &glyphs), 1, "'x' sits at cell 1");
}

#[test]
fn zwj_emoji_family_is_one_cell() {
    // man + ZWJ + woman + ZWJ + girl = one cluster.
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let text = format!("{family}x");
    let clusters: Vec<&str> = grapheme_iter(&text).collect();
    assert_eq!(clusters.len(), 2);
    assert_eq!(clusters[0], family);
    assert_eq!(prev_grapheme(&text, family.len() + 1), family.len(), "left from EOF lands on 'x'");
    assert_eq!(prev_grapheme(&text, family.len()), 0, "left from 'x' jumps the whole family");
}

#[test]
fn cjk_wide_characters_take_two_cells() {
    let glyphs = no_glyphs();
    let text = "中x";
    assert_eq!(grapheme_cell_width("中", 4, 0, &glyphs), 2);
    assert_eq!(cell_column(text, 3, 4, &glyphs), 2, "'x' at cell 2 after CJK");
    assert_eq!(line_cells("中中", 4, &glyphs), 4);
}

#[test]
fn tabs_advance_to_tab_stops() {
    let glyphs = no_glyphs();
    assert_eq!(grapheme_cell_width("\t", 4, 0, &glyphs), 4);
    assert_eq!(grapheme_cell_width("\t", 4, 1, &glyphs), 3);
    assert_eq!(grapheme_cell_width("\t", 4, 4, &glyphs), 4);
    assert_eq!(cell_column("\ta", 1, 4, &glyphs), 4, "'a' sits at cell 4");
    assert_eq!(cell_column("\ta", 2, 4, &glyphs), 5);
}

#[test]
fn offsets_snap_to_grapheme_boundaries() {
    // 😀 is 4 bytes (offsets 0..4); byte 2 is mid-grapheme.
    let text = "a😀b";
    assert_eq!(floor_grapheme(text, 2), 1, "mid-emoji snaps to its start");
    assert_eq!(floor_grapheme(text, 5), 5, "'b' stays");
    assert_eq!(floor_grapheme(text, 99), text.len());
    assert_eq!(prev_grapheme(text, 3), 1, "left from mid-emoji lands before it");
    assert_eq!(next_grapheme(text, 2), 5, "right from mid-emoji lands after it");
}

#[test]
fn layout_pipeline_end_to_end() {
    // byte offset → grapheme → cell width → screen position over a mixed
    // line: ASCII + combining + CJK + emoji.
    let glyphs = no_glyphs();
    let text = "e\u{0301}中😀b";
    // clusters: e+́ (1 cell), 中 (2), 😀 (2), b (1)
    assert_eq!(line_cells(text, 4, &glyphs), 6);
    assert_eq!(cell_column(text, text.len(), 4, &glyphs), 6);
    // '中' starts at cell 1; 😀 at cell 3; 'b' at cell 5.
    let e_mark = 1 + 'e'.len_utf8() + '\u{0301}'.len_utf8();
    assert_eq!(cell_column(text, e_mark, 4, &glyphs), 1);
    assert_eq!(cell_column(text, e_mark + 3, 4, &glyphs), 3);
}

#[test]
fn empty_and_edge_inputs() {
    let glyphs = no_glyphs();
    assert_eq!(line_cells("", 4, &glyphs), 0);
    assert_eq!(cell_column("", 0, 4, &glyphs), 0);
    assert_eq!(prev_grapheme("ab", 0), 0);
    assert_eq!(next_grapheme("ab", 2), 2);
    assert_eq!(floor_grapheme("", 0), 0);
}

#[test]
fn wide_glyphs_render_aligned_in_two_surface_cells() {
    use crate::kernel::command::builtin;
    use crate::kernel::render::frame::render_frame;
    use crate::kernel::render::surface::Surface;
    use crate::kernel::state::Editor;
    use crate::kernel::storage::disk::DiskFileSystem;

    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("cjk", "a中b\n");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    let mut surface = Surface::new(20, 4);
    render_frame(&ed, &mut surface);

    // 'a' at content col 0 (cursor may cover it); '中' at cols 1-2; 'b' at
    // col 3 — the 'b' cell proves the wide glyph consumed two columns.
    let cjk_x = (0..20u16).find(|&x| surface.cell(x, 0).map(|c| c.ch) == Some('中'))
        .expect("row renders 中");
    assert_eq!(surface.cell(cjk_x + 2, 0).map(|c| c.ch), Some('b'),
        "'b' must land after the wide glyph's two cells");
}
