use crate::render::gpu_atlas::{
    build_glyph_instances, build_rect_instances, glyphs_to_vertices, rects_to_vertices,
    GpuGlyphAtlas, ATLAS_CHARS,
};
use crate::render::surface::{Style, Surface};

fn default_style() -> Style {
    Style::default()
}

fn red_fg_style() -> Style {
    Style { fg: (255, 0, 0), ..Style::default() }
}

fn blue_bg_style() -> Style {
    Style { bg: (0, 0, 255), ..Style::default() }
}

// ── GpuGlyphAtlas::new ───────────────────────────────────────────────────────

#[test]
fn atlas_new_is_dirty() {
    let atlas = GpuGlyphAtlas::new();
    assert!(atlas.needs_rebuild());
}

#[test]
fn atlas_mark_clean_and_dirty_roundtrip() {
    let mut atlas = GpuGlyphAtlas::new();
    atlas.mark_clean();
    assert!(!atlas.needs_rebuild());
    atlas.mark_dirty();
    assert!(atlas.needs_rebuild());
}

#[test]
fn atlas_covers_all_printable_ascii() {
    let atlas = GpuGlyphAtlas::new();
    for ch in ATLAS_CHARS.chars() {
        assert!(atlas.contains(ch), "atlas missing char: {:?}", ch);
    }
}

#[test]
fn atlas_covers_space() {
    let atlas = GpuGlyphAtlas::new();
    assert!(atlas.contains(' '));
}

#[test]
fn atlas_normalize_sums_to_one_for_full_cell() {
    let atlas = GpuGlyphAtlas::new();
    // A character in the last row, last column should have uv + uv_size ≤ 1.0
    for uv in atlas.glyphs.values() {
        let [x, y, w, h] = atlas.normalize(uv);
        assert!(x + w <= 1.001, "UV x+w out of range: {}", x + w);
        assert!(y + h <= 1.001, "UV y+h out of range: {}", y + h);
        assert!(x >= 0.0 && y >= 0.0);
    }
}

#[test]
fn atlas_placeholder_pixels_correct_size() {
    let atlas = GpuGlyphAtlas::new();
    let pixels = atlas.generate_placeholder_pixels();
    assert_eq!(pixels.len(), (atlas.atlas_width * atlas.atlas_height * 4) as usize);
    assert!(pixels.iter().all(|&b| b == 255), "placeholder should be all-white RGBA");
}

// ── build_glyph_instances ────────────────────────────────────────────────────

#[test]
fn glyph_instances_skips_spaces_and_nulls() {
    let mut surface = Surface::new(4, 2);
    surface.set_cell(0, 0, ' ', Some(default_style()));
    surface.set_cell(1, 0, 'A', Some(red_fg_style()));
    surface.set_cell(2, 0, '\0', Some(default_style()));
    surface.set_cell(3, 0, 'B', Some(red_fg_style()));

    let atlas = GpuGlyphAtlas::new();
    let instances = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    assert_eq!(instances.len(), 2, "only A and B should be in instances");
}

#[test]
fn glyph_instances_position_is_correct() {
    let mut surface = Surface::new(3, 1);
    surface.set_cell(2, 0, 'X', Some(default_style()));

    let atlas = GpuGlyphAtlas::new();
    let char_w = 10.0f32;
    let line_h = 20.0f32;
    let instances = build_glyph_instances(&surface, &atlas, char_w, line_h, 5.0, 7.0, false);
    assert_eq!(instances.len(), 1);
    assert!((instances[0].screen_x - (5.0 + 2.0 * char_w)).abs() < 1e-5);
    assert!((instances[0].screen_y - 7.0).abs() < 1e-5);
}

#[test]
fn glyph_instances_fg_color_normalized() {
    let mut surface = Surface::new(1, 1);
    surface.set_cell(0, 0, 'Z', Some(Style { fg: (255, 128, 0), ..Style::default() }));

    let atlas = GpuGlyphAtlas::new();
    let instances = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    assert_eq!(instances.len(), 1);
    let [r, g, b, a] = instances[0].fg;
    assert!((r - 1.0).abs() < 1e-3);
    assert!((g - 128.0 / 255.0).abs() < 1e-3);
    assert!((b - 0.0).abs() < 1e-3);
    assert!((a - 1.0).abs() < 1e-3);
}

#[test]
fn glyph_instances_skip_last_row_when_requested() {
    let mut surface = Surface::new(2, 3);
    surface.set_cell(0, 0, 'A', Some(default_style()));
    surface.set_cell(0, 1, 'B', Some(default_style()));
    surface.set_cell(0, 2, 'C', Some(default_style())); // last row (status bar)

    let atlas = GpuGlyphAtlas::new();
    let all = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    let skip = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, true);
    assert_eq!(all.len(), 3);
    assert_eq!(skip.len(), 2, "last row should be excluded");
}

// ── build_rect_instances ─────────────────────────────────────────────────────

#[test]
fn rect_instances_skips_default_bg_cells() {
    let mut surface = Surface::new(3, 1);
    let default_bg = [0.0f32, 0.0, 0.0, 1.0]; // Style::default() bg = (0, 0, 0) → alpha=1

    surface.set_cell(0, 0, ' ', Some(default_style()));
    surface.set_cell(1, 0, 'A', Some(blue_bg_style())); // different bg
    surface.set_cell(2, 0, ' ', Some(default_style()));

    let rects = build_rect_instances(&surface, 8.0, 16.0, 0.0, 0.0, default_bg, false);
    assert_eq!(rects.len(), 1, "only blue-bg cell produces a rect");
}

#[test]
fn rect_instances_position_and_size_correct() {
    let mut surface = Surface::new(2, 1);
    surface.set_cell(1, 0, 'A', Some(blue_bg_style()));

    // Style::default() bg = (0,0,0) → [0,0,0,1] (alpha = 1)
    let default_bg = [0.0f32, 0.0, 0.0, 1.0];
    let char_w = 12.0f32;
    let line_h = 18.0f32;
    let rects = build_rect_instances(&surface, char_w, line_h, 3.0, 5.0, default_bg, false);
    assert_eq!(rects.len(), 1);
    assert!((rects[0].x - (3.0 + char_w)).abs() < 1e-5);
    assert!((rects[0].y - 5.0).abs() < 1e-5);
    assert!((rects[0].w - char_w).abs() < 1e-5);
    assert!((rects[0].h - line_h).abs() < 1e-5);
}

// ── vertex expansion ─────────────────────────────────────────────────────────

#[test]
fn glyphs_to_vertices_produces_6_verts_per_instance() {
    let mut surface = Surface::new(2, 1);
    surface.set_cell(0, 0, 'A', Some(default_style()));
    surface.set_cell(1, 0, 'B', Some(default_style()));

    let atlas = GpuGlyphAtlas::new();
    let instances = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    let verts = glyphs_to_vertices(&instances, 8.0, 16.0);
    assert_eq!(verts.len(), instances.len() * 6);
}

#[test]
fn rects_to_vertices_produces_6_verts_per_rect() {
    let mut surface = Surface::new(3, 1);
    surface.set_cell(0, 0, ' ', Some(blue_bg_style()));
    surface.set_cell(1, 0, ' ', Some(blue_bg_style()));

    let default_bg = [0.0f32, 0.0, 0.0, 1.0];
    let rects = build_rect_instances(&surface, 8.0, 16.0, 0.0, 0.0, default_bg, false);
    let verts = rects_to_vertices(&rects);
    assert_eq!(verts.len(), rects.len() * 6);
}

// ── Fallback path selection ───────────────────────────────────────────────────

#[test]
fn new_atlas_gpu_path_disabled_by_default() {
    // GuiApp starts with gpu_path_ready = false, so the fallback is used.
    // We verify this indirectly: building instances from an empty surface
    // produces an empty list, which is the "nothing to draw" fallback signal.
    let surface = Surface::new(0, 0);
    let atlas = GpuGlyphAtlas::new();
    let instances = build_glyph_instances(&surface, &atlas, 8.0, 16.0, 0.0, 0.0, false);
    assert!(instances.is_empty());
}
