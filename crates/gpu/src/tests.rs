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
    assert_eq!(
        chars.len(),
        atlas.glyphs.len(),
        "one cell per baked character"
    );

    let a_uv = atlas.normalize(atlas.glyphs.get(&'A').expect("A is baked"));
    let space_uv = atlas.normalize(atlas.glyphs.get(&' ').expect("space is baked"));
    assert!(
        a_uv[0] > space_uv[0],
        "the atlas lays characters left to right"
    );
    assert!(
        a_uv.iter()
            .chain(space_uv.iter())
            .all(|v| (0.0..=1.0).contains(v))
    );
}

#[test]
fn atlas_bakes_the_cursor_and_gutter_glyphs() {
    let atlas = GpuGlyphAtlas::new();
    for ch in ['█', '▌', '▏', '▔', '▸', '▾', '●', '○', '—'] {
        assert!(
            atlas.contains(ch),
            "atlas is missing the rendered glyph {ch:?}"
        );
    }
}

#[test]
fn rasterize_at_ppx_grows_cells_with_the_device_ratio() {
    let font = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
    let mut atlas = GpuGlyphAtlas::new();
    atlas.rasterize_at_ppx(font, 15.0, 2.0);

    assert_eq!(
        (atlas.cell_w, atlas.cell_h),
        (ATLAS_CELL_W * 2, ATLAS_CELL_H * 2)
    );
    assert_eq!(atlas.atlas_width, ATLAS_COLS * ATLAS_CELL_W * 2);
    assert_eq!(
        atlas.atlas_pixels.len(),
        (atlas.atlas_width * atlas.atlas_height * 4) as usize
    );

    let uv = atlas.glyphs.get(&'A').expect("A is baked");
    assert_eq!((uv.w, uv.h), (ATLAS_CELL_W * 2, ATLAS_CELL_H * 2));
    let [x, y, w, h] = atlas.normalize(uv);
    assert!(
        x + w <= 1.001 && y + h <= 1.001,
        "scaled UV leaves the texture"
    );
}

#[test]
fn the_placeholder_atlas_is_opaque_white_and_dirty() {
    let atlas = GpuGlyphAtlas::new();
    assert!(atlas.needs_rebuild(), "a fresh atlas needs an upload");
    let px = atlas.generate_atlas_pixels();
    assert_eq!(
        px.len(),
        (atlas.atlas_width * atlas.atlas_height * 4) as usize
    );
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
fn rasterizer_clips_each_glyph_to_its_cell() {
    let font = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
    // Two 8×8 cells, but only the first one is drawn.  A 96px glyph is far
    // wider than the cell and must be clipped, not painted into cell two.
    let pixels = crate::gpu_rasterizer::rasterize_font_atlas(font, 96.0, &['W'], 8, 8, 2);
    let width = 16;
    for y in 0..8 {
        for x in 8..width {
            let alpha = pixels[((y * width + x) * 4 + 3) as usize];
            assert_eq!(
                alpha, 0,
                "glyph ink crossed the cell boundary at ({x}, {y})"
            );
        }
    }
}

#[test]
fn glyph_instances_skip_spaces_and_position_cells_on_the_grid() {
    let atlas = GpuGlyphAtlas::new();
    let surface = at("a b", 4, 2);

    let instances = glyphs(&surface, &atlas, false);
    assert_eq!(instances.len(), 2, "the space cell produces no quad");
    assert_eq!((instances[0].screen_x, instances[0].screen_y), (0.0, 0.0));
    assert_eq!(instances[1].screen_x, 16.0, "the gap still occupies a cell");
    assert_eq!(
        instances[0].uv_w,
        ATLAS_CELL_W as f32 / atlas.atlas_width as f32
    );
}

#[test]
fn foreground_colors_are_normalized_to_unit_range() {
    let atlas = GpuGlyphAtlas::new();
    let mut surface = Surface::new(1, 1);
    surface.set_cell(
        0,
        0,
        'x',
        Some(Style {
            fg: (255, 0, 51),
            ..Style::default()
        }),
    );

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
    let dark = Style {
        fg: (255, 255, 255),
        bg: (10, 20, 30),
        ..Style::default()
    };

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

#[test]
#[ignore]
fn dump_glyph_cells() {
    let home = std::env::var("HOME").unwrap();
    let font = std::fs::read(format!(
        "{home}/Library/Fonts/MonacoNerdFontMono-Regular.ttf"
    ))
    .unwrap();
    let chars = vec![' ', 's', 't', 'r', 'u', 'c'];
    let pixels = crate::gpu_rasterizer::rasterize_font_atlas(&font, 30.0, &chars, 20, 40, 6);
    for (idx, ch) in chars.iter().enumerate().skip(1) {
        println!("char {ch}");
        for y in 0..40 {
            let mut line = String::new();
            for x in 0..20 {
                let a = pixels[((y * 120 + idx * 20 + x) * 4 + 3) as usize];
                line.push(match a {
                    0 => ' ',
                    1..=85 => '.',
                    86..=170 => '+',
                    _ => '#',
                });
            }
            println!("{line}");
        }
    }
}
