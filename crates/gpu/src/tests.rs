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
    for ch in ['█', '▌', '▎', '▏', '▔', '▸', '▾', '●', '○', '—'] {
        assert!(
            atlas.contains(ch),
            "atlas is missing the rendered glyph {ch:?}"
        );
    }
}

/// First/last row (and column) of a marker's atlas cell carrying ink.
fn cell_ink_span(atlas: &GpuGlyphAtlas, ch: char) -> (u32, u32, u32, u32) {
    let uv = atlas.glyphs.get(&ch).expect("marker is baked");
    let alpha = |px: u32, py: u32| {
        atlas.atlas_pixels[(((uv.y + py) * atlas.atlas_width + uv.x + px) * 4 + 3) as usize]
    };
    let row_ink = |py: u32| (0..uv.w).any(|px| alpha(px, py) > 0);
    let col_ink = |px: u32| (0..uv.h).any(|py| alpha(px, py) > 0);
    let top = (0..uv.h).find(|&py| row_ink(py)).expect("marker has ink");
    let bottom = (0..uv.h).rev().find(|&py| row_ink(py)).unwrap();
    let left = (0..uv.w).find(|&px| col_ink(px)).expect("marker has ink");
    let right = (0..uv.w).rev().find(|&px| col_ink(px)).unwrap();
    (top, bottom, left, right)
}

#[test]
fn gutter_markers_are_baked_as_geometry_not_font_glyphs() {
    let font = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
    let mut atlas = GpuGlyphAtlas::new();
    atlas.rasterize(font, 15.0);
    let (cw, ch) = (atlas.cell_w, atlas.cell_h);

    // The disc fills the marker column and sits exactly in the cell middle;
    // the font's own ● is smaller and rides the text baseline.
    let (top, bottom, left, right) = cell_ink_span(&atlas, '●');
    assert!(right - left + 1 >= cw * 3 / 4, "disc must fill the column");
    let mid = (top + bottom) as f32 / 2.0;
    assert!(
        (mid - ch as f32 / 2.0).abs() <= 2.0,
        "disc must be centred, not baseline-aligned"
    );

    // The fold arrow spans more of the cell than the font's small variant.
    let (top, bottom, _, _) = cell_ink_span(&atlas, '▾');
    assert!(bottom - top + 1 >= ch / 2, "fold arrow must be legible");

    // The git sign is a left-anchored bar — the box fallback would ink the
    // right edge of the cell instead.
    let (_, _, left, right) = cell_ink_span(&atlas, '▎');
    assert!(left <= cw / 8, "bar hangs off the left edge");
    assert!(right <= cw / 2, "no box-fallback ink on the right half");
}

/// Coverage-weighted horizontal centre of a cell's ink over rows `[from, to)`.
fn ink_centre(atlas: &GpuGlyphAtlas, ch: char, italic: bool, from: u32, to: u32) -> f32 {
    let uv = atlas.uv_for(ch, italic).expect("glyph is baked");
    let (mut sum, mut mass) = (0.0f32, 0.0f32);
    for py in from..to {
        for px in 0..uv.w {
            let a = cell_alpha(atlas, &uv, px, py) as f32 / 255.0;
            sum += px as f32 * a;
            mass += a;
        }
    }
    sum / mass
}

/// Total coverage of a cell: how much ink the glyph carries.
fn ink_mass(atlas: &GpuGlyphAtlas, ch: char, italic: bool) -> f32 {
    let uv = atlas.uv_for(ch, italic).expect("glyph is baked");
    let mut mass = 0.0;
    for py in 0..uv.h {
        for px in 0..uv.w {
            mass += cell_alpha(atlas, &uv, px, py) as f32 / 255.0;
        }
    }
    mass
}

fn cell_alpha(atlas: &GpuGlyphAtlas, uv: &GlyphUv, px: u32, py: u32) -> u8 {
    atlas.atlas_pixels[(((uv.y + py) * atlas.atlas_width + uv.x + px) * 4 + 3) as usize]
}

#[test]
fn shear_slants_an_outline_about_its_pivot() {
    use ab_glyph::{Outline, OutlineCurve, Point, Rect};
    let mut outline = Outline {
        bounds: Rect {
            min: Point { x: 0.0, y: 10.0 },
            max: Point { x: 4.0, y: -2.0 },
        },
        curves: vec![OutlineCurve::Line(
            Point { x: 0.0, y: 5.0 },
            Point { x: 0.0, y: 10.0 },
        )],
    };
    crate::oblique::shear(&mut outline, 0.5, 5.0);

    let OutlineCurve::Line(foot, tip) = &outline.curves[0] else {
        panic!("a line stays a line");
    };
    assert_eq!((foot.x, foot.y), (0.0, 5.0), "the pivot itself holds still");
    assert_eq!((tip.x, tip.y), (2.5, 10.0), "height moves ink sideways");
    // The box has to cover the sheared corners — the top one right of the
    // upright box, the bottom one left of it.
    assert_eq!(outline.bounds.min.x, -3.5);
    assert_eq!(outline.bounds.max.x, 6.5);
    assert_eq!((outline.bounds.min.y, outline.bounds.max.y), (10.0, -2.0));
}

#[test]
fn every_baked_character_has_an_oblique_cell_below_the_roman_block() {
    let atlas = GpuGlyphAtlas::new();
    assert_eq!(atlas.glyphs.len(), atlas.glyphs_italic.len());
    let roman_bottom = atlas.glyphs.values().map(|uv| uv.y + uv.h).max().unwrap();
    assert_eq!(roman_bottom, atlas.atlas_height / 2, "the blocks split the texture");
    for (ch, slanted) in &atlas.glyphs_italic {
        let upright = atlas.glyphs[ch];
        assert_eq!(
            (slanted.x, slanted.w, slanted.h),
            (upright.x, upright.w, upright.h),
            "{ch:?}: an oblique cell sits in the upright cell's own column"
        );
        assert!(
            slanted.y >= roman_bottom,
            "{ch:?}: the oblique cell overlaps the roman block"
        );
    }
}

#[test]
fn every_glyph_leans_by_the_face_s_own_angle() {
    // Rows 5..8 and 9..12 straddle the shear's pivot, so the distance
    // between their ink centres is the glyph's lean and not an artefact of
    // where its own ink happens to sit.  The bug this pins capped each row's
    // slide by that row's blank margin, which left a full-width letter
    // upright while a stem leaned: letters tore apart at the seam.
    let font = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
    let mut atlas = GpuGlyphAtlas::new();
    atlas.rasterize(font, 15.0);
    let lean = |ch: char| {
        ink_centre(&atlas, ch, true, 5, 8)
            - ink_centre(&atlas, ch, false, 5, 8)
            - (ink_centre(&atlas, ch, true, 9, 12) - ink_centre(&atlas, ch, false, 9, 12))
    };
    let stem = lean('l');
    assert!(stem > 0.4, "the oblique must lean forward: {stem}");
    for ch in ['i', 'n', 'a', 'o', 'm', 'W', 'E'] {
        let slanted = lean(ch);
        assert!(slanted > 0.4, "{ch:?}: the oblique must lean forward");
        assert!(
            (slanted - stem).abs() < 0.25,
            "{ch:?} leans {slanted} where a stem leans {stem}: \
             a glyph's own ink must not set its slant"
        );
    }
}

#[test]
fn the_oblique_shear_moves_ink_without_eating_it() {
    // A shear preserves area, so the slanted cell carries the coverage the
    // upright one does.  A cell that lost a visible share would mean the
    // glyph ran out of its own cell.
    let font = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
    let mut atlas = GpuGlyphAtlas::new();
    atlas.rasterize(font, 15.0);
    for ch in ['l', 'i', 'n', 'a', 'o', 'e', 'r', 's', 'm', 'W', 'M', 'g'] {
        let (upright, slanted) = (ink_mass(&atlas, ch, false), ink_mass(&atlas, ch, true));
        assert!(
            (slanted - upright).abs() <= upright * 0.05,
            "{ch:?}: the oblique cell carries {slanted}, the upright one {upright}"
        );
    }
}

#[test]
fn a_missing_glyph_falls_back_to_the_box_of_its_own_slant() {
    let atlas = GpuGlyphAtlas::new();
    assert_eq!(atlas.uv_for('A', true), atlas.glyphs_italic.get(&'A').copied());
    assert_eq!(atlas.uv_for('A', false), atlas.glyphs.get(&'A').copied());
    assert_eq!(
        atlas.uv_for('漢', true),
        atlas.glyphs_italic.get(&'\u{25A1}').copied(),
        "a glyph the face does not draw must lean with the rest of the run"
    );
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
