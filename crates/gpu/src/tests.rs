use super::*;
use surface::{Style, Surface};

fn at(text: &str, w: u16, h: u16) -> Surface {
    let mut s = Surface::new(w, h);
    s.set_text(0, 0, text, None);
    s
}

fn glyphs(surface: &Surface, atlas: &GpuGlyphAtlas, skip_last_row: bool) -> Vec<GlyphInstance> {
    build_glyph_instances(surface, atlas, 8.0, 16.0, 0.0, 0.0, skip_last_row)
}

#[test]
fn a_new_atlas_maps_every_baked_character_to_a_grid_cell() {
    let atlas = GpuGlyphAtlas::new();
    let chars: Vec<char> = ATLAS_CHARS.chars().collect();
    assert!(atlas.contains('A') && atlas.contains('é') && atlas.contains('□'));
    assert!(!atlas.contains('漢'), "CJK falls back, it is not baked");
    assert_eq!(chars.len(), atlas.glyphs.len(), "one cell per baked character");

    let a_uv = atlas.normalize(atlas.glyphs.get(&'A').expect("A is baked"));
    let space_uv = atlas.normalize(atlas.glyphs.get(&' ').expect("space is baked"));
    assert!(a_uv[0] > space_uv[0], "the atlas lays characters left to right");
    assert!(a_uv.iter().chain(space_uv.iter()).all(|v| (0.0..=1.0).contains(v)));
}

#[test]
fn the_placeholder_atlas_is_opaque_white_and_dirty() {
    let atlas = GpuGlyphAtlas::new();
    assert!(atlas.needs_rebuild(), "a fresh atlas needs an upload");
    let px = atlas.generate_atlas_pixels();
    assert_eq!(px.len(), (atlas.atlas_width * atlas.atlas_height * 4) as usize);
    assert!(px.iter().all(|b| *b == 255));
}

#[test]
fn marking_clean_and_dirty_toggles_the_upload_flag() {
    let mut atlas = GpuGlyphAtlas::new();
    atlas.mark_clean();
    assert!(!atlas.needs_rebuild());
    atlas.mark_dirty();
    assert!(atlas.needs_rebuild());
}

#[test]
fn glyph_instances_skip_spaces_and_position_cells_on_the_grid() {
    let atlas = GpuGlyphAtlas::new();
    let surface = at("a b", 4, 2);

    let instances = glyphs(&surface, &atlas, false);
    assert_eq!(instances.len(), 2, "the space cell produces no quad");
    assert_eq!((instances[0].screen_x, instances[0].screen_y), (0.0, 0.0));
    assert_eq!(instances[1].screen_x, 16.0, "the gap still occupies a cell");
    assert_eq!(instances[0].uv_w, ATLAS_CELL_W as f32 / atlas.atlas_width as f32);
}

#[test]
fn foreground_colors_are_normalized_to_unit_range() {
    let atlas = GpuGlyphAtlas::new();
    let mut surface = Surface::new(1, 1);
    surface.set_cell(0, 0, 'x', Some(Style { fg: (255, 0, 51), ..Style::default() }));

    let fg = glyphs(&surface, &atlas, false)[0].fg;
    assert_eq!(fg[0], 1.0);
    assert_eq!(fg[1], 0.0);
    assert!((fg[2] - 0.2).abs() < 0.01);
    assert_eq!(fg[3], 1.0);
}

#[test]
fn skipping_the_last_row_drops_the_status_line_cells() {
    let atlas = GpuGlyphAtlas::new();
    let mut surface = Surface::new(2, 2);
    surface.set_text(0, 0, "ab", None);
    surface.set_text(0, 1, "cd", None);

    assert_eq!(glyphs(&surface, &atlas, false).len(), 4);
    assert_eq!(glyphs(&surface, &atlas, true).len(), 2);
}

#[test]
fn glyph_vertices_are_six_eight_float_vertices_per_instance() {
    let atlas = GpuGlyphAtlas::new();
    let surface = at("xy", 3, 1);

    let instances = glyphs(&surface, &atlas, false);
    let vertices = glyphs_to_vertices(&instances, 8.0, 16.0);
    assert_eq!(vertices.len(), instances.len() * 6);
    assert!(vertices.iter().all(|v| v.len() == 8));
    // First vertex corner, last vertex of the first quad is the top-right.
    assert_eq!(&vertices[0][..2], &[0.0, 0.0]);
    assert_eq!(&vertices[5][..2], &[8.0, 0.0]);
}

#[test]
fn rect_instances_skip_the_default_background_and_expand_to_vertices() {
    let mut surface = Surface::new(2, 1);
    let default_bg = [0.0, 0.0, 0.0, 1.0];
    let dark = Style { fg: (255, 255, 255), bg: (10, 20, 30), ..Style::default() };

    surface.set_cell(0, 0, ' ', Some(dark));
    assert_eq!(
        build_rect_instances(&surface, 8.0, 16.0, 0.0, 0.0, default_bg, false).len(),
        1,
        "the default-background cell needs no rect"
    );

    let rects = build_rect_instances(&surface, 8.0, 16.0, 0.0, 0.0, default_bg, false);
    let vertices = rects_to_vertices(&rects, ATLAS_COLS * ATLAS_CELL_W, ATLAS_CELL_H);
    assert_eq!(vertices.len(), rects.len() * 6);
    assert!((vertices[0][4] - 10.0 / 255.0).abs() < 0.01);
}
