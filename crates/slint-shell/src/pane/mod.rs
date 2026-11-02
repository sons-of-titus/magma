//! Surface → Slint image: the editor pane's presentation path.
//!
//! The kernel renders editor semantics into a character grid.  The `blit`
//! submodule turns that grid into RGBA pixels using the glyph atlas; this
//! module owns the grid geometry and font measurement those pixels sit on.
//! No widget toolkit types reach the kernel.

pub mod blit;

pub use blit::{blit_image, blit_minimap, blit_rgba, blit_terminal_rgba, surface_signature};

use kernel::render::gpu_atlas::GpuGlyphAtlas;

pub const CHAR_W: u32 = 7;
pub const LINE_H: u32 = 17;

/// Terminal dock inner height: the dock is 200px, its header 26px.
pub const TERMINAL_DOCK_BODY_H: f32 = 174.0;
pub const TERMINAL_TABS_H: f32 = 26.0;

pub const MINIMAP_W: u32 = 90;

/// Grid for the terminal body.  The dock spans the window width; the session
/// tab strip is carved out of its height, and the body is rounded down to
/// whole rows so the rendered texture is never scaled.
pub fn terminal_grid_size(logical_w: f32, sessions: usize, cell_w: u32, cell_h: u32) -> (u32, u32) {
    let tabs = if sessions > 0 { TERMINAL_TABS_H } else { 0.0 };
    let cell_w = cell_w.max(1) as f32;
    let cell_h = cell_h.max(1) as f32;
    let body = (TERMINAL_DOCK_BODY_H - tabs).max(cell_h);
    (
        (logical_w / cell_w).floor().max(1.0) as u32,
        (body / cell_h).floor().max(1.0) as u32,
    )
}

/// Device-pixel size of one grid cell at `scale`.
pub fn cell_size(cell_w: u32, cell_h: u32, scale: f32) -> (u32, u32) {
    let scale = if scale < 1.0 { 1.0 } else { scale };
    (
        (cell_w.max(1) as f32 * scale).round().max(1.0) as u32,
        (cell_h.max(1) as f32 * scale).round().max(1.0) as u32,
    )
}

/// Measure the tight cell size (logical pixels) for a font at `font_size`.
pub fn measure_cell(font_bytes: &[u8], font_size: f32) -> (u32, u32) {
    use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};
    let Ok(font) = FontRef::try_from_slice(font_bytes) else {
        return (CHAR_W, LINE_H);
    };
    let scaled = font.as_scaled(PxScale::from(font_size));
    let gid = scaled.glyph_id('M');
    let adv = scaled.h_advance(gid);
    let cell_w = adv.ceil().max(1.0) as u32;
    let glyph_h = (scaled.ascent() - scaled.descent()).ceil().max(1.0) as u32;
    let cell_h = ((font_size * 1.15).ceil() as u32).max(glyph_h);
    (cell_w.max(4), cell_h)
}

/// Caret geometry from font metrics only: `(cap-line inset, beam height)`
/// in logical pixels.  The beam runs from the cap line down to the baseline
/// (a capital's ink span).  Serves as the pre-first-frame fallback; once
/// the atlas exists, [`atlas_ink_span_px`] measures the real ink and is
/// exact.
pub fn caret_metrics_fallback(font_bytes: &[u8], font_size: f32) -> (f32, f32) {
    use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};
    let Ok(font) = FontRef::try_from_slice(font_bytes) else {
        return (0.0, font_size);
    };
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let cap = scaled
        .glyph_id('M')
        .with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
    let Some(outline) = font.outline_glyph(cap) else {
        return (0.0, font_size);
    };
    let inset = (scaled.ascent() + outline.px_bounds().min.y).max(0.0);
    (inset, -outline.px_bounds().min.y)
}

/// The vertical ink span of a capital letter inside its atlas cell:
/// `(first ink row, last ink row)` in device pixels.  Measured from the
/// same rasterized pixels the text is blitted from, so it *is* the glyph
/// geometry on screen — font-metric math can miss it by a pixel because
/// the rasterizer quantizes outlines at device scale.
pub fn atlas_ink_span_px(atlas: &GpuGlyphAtlas) -> Option<(u32, u32)> {
    let uv = atlas.glyphs.get(&'M')?;
    let covered = |py: u32| {
        (0..uv.w).any(|px| {
            atlas.atlas_pixels[(((uv.y + py) * atlas.atlas_width + uv.x + px) * 4) as usize] > 0
        })
    };
    let top = (0..uv.h).find(|&py| covered(py))?;
    let bottom = (0..uv.h).rev().find(|&py| covered(py))?;
    Some((top, bottom))
}

/// The logical size one grid cell actually occupies on screen: the atlas and
/// the pane blit work in whole device pixels (`round(cell * ppx)`), so a
/// non-integral `cell × ppx` leaves sub-pixel slack that accumulates down
/// the screen.  Overlays that must land on a glyph row compute their
/// geometry from these units, never from the raw integer grid.
pub fn logical_cell(cell_w: u32, cell_h: u32, ppx: f32) -> (f32, f32) {
    let (dw, dh) = cell_size(cell_w, cell_h, ppx);
    let ppx = ppx.max(1.0);
    (dw as f32 / ppx, dh as f32 / ppx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::render::surface::Surface;

    const FONT: &[u8] = include_bytes!("../../../../assets/fonts/JetBrainsMono-Regular.ttf");

    #[test]
    fn gutter_digits_and_text_share_surface_rows() {
        use kernel::render::frame::render_frame_with;

        let mut ed =
            kernel::state::Editor::new(Box::new(kernel::storage::disk::DiskFileSystem::new()));
        kernel::command::builtin::register_builtin_commands(&mut ed);
        let key = ed.create_buffer_from_str(
            "lib.rs",
            "fn main() {\n    let alpha = 1;\n    let beta = 2;\n}",
        );
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        ed.view_tree.resize(60, 8);
        let mut surface = Surface::new(60, 8);
        render_frame_with(&ed, &mut surface, kernel::render::CursorPaint::Skip);

        let row_of = |want: &str, x_range: std::ops::Range<u16>| {
            (0..surface.height).find(|&y| {
                (x_range.clone())
                    .any(|x| surface.cell(x, y).map(|c| c.ch.to_string()) == Some(want.to_string()))
            })
        };
        let digit_row = row_of("1", 0..8);
        let f_row = row_of("f", 8..30);
        assert_eq!(
            digit_row, f_row,
            "gutter digit and its line's text must share a row"
        );
    }

    #[test]
    fn measured_ink_span_covers_exactly_the_capital_ink() {
        // atlas_ink_span_px reads the same pixels the text blits from: the
        // beam built on it must start on the first ink row, end on the last,
        // and stay on the device grid, at every size and scale.
        for size in [12.0_f32, 15.0, 17.5] {
            for ppx in [1.0_f32, 2.0] {
                let (cell_w, cell_h) = measure_cell(FONT, size);
                let mut atlas = GpuGlyphAtlas::new();
                atlas.rasterize_for_grid(FONT, size, ppx, cell_w, cell_h);
                let (top, bottom) = atlas_ink_span_px(&atlas).expect("'M' has ink");
                let uv = atlas.glyphs.get(&'M').expect("glyph in atlas charset");
                let atlas_top = (0..uv.h)
                    .find(|&py| {
                        (0..uv.w).any(|px| {
                            atlas.atlas_pixels
                                [(((uv.y + py) * atlas.atlas_width + uv.x + px) * 4) as usize]
                                > 0
                        })
                    })
                    .expect("glyph has visible pixels");
                assert_eq!(top, atlas_top);
                let (inset, beam) = (top as f32 / ppx, (bottom - top + 1) as f32 / ppx);
                assert_eq!(inset * ppx, top as f32, "inset stays on the device grid");
                assert_eq!(beam * ppx, (bottom - top + 1) as f32);
                assert!(
                    inset + beam <= cell_h as f32,
                    "size {size} ppx {ppx}: beam must fit inside the cell"
                );
            }
        }
    }

    #[test]
    fn font_metric_fallback_stays_within_a_pixel_of_the_real_ink() {
        // Before the first atlas exists the caret uses font-math; the cap
        // line must never be off by more than a device pixel or the beam
        // visibly floats until the first frame.
        for size in [12.0_f32, 15.0, 17.5] {
            let (cell_w, cell_h) = measure_cell(FONT, size);
            let mut atlas = GpuGlyphAtlas::new();
            atlas.rasterize_for_grid(FONT, size, 1.0, cell_w, cell_h);
            let (ink_top, ink_bottom) = atlas_ink_span_px(&atlas).expect("'M' has ink");
            let (inset, beam) = caret_metrics_fallback(FONT, size);
            assert!(
                (inset - ink_top as f32).abs() <= 1.0,
                "size {size}: fallback inset {inset} vs ink top {ink_top}"
            );
            assert!(
                ((inset + beam) - (ink_bottom as f32 + 1.0)).abs() <= 1.0,
                "size {size}: fallback beam bottom {} vs ink bottom {ink_bottom}",
                inset + beam
            );
        }
    }

    #[test]
    fn logical_cell_round_trips_through_device_pixels() {
        for ppx in [1.0_f32, 2.0, 1.5, 2.25] {
            let (lw, lh) = logical_cell(9, 17, ppx);
            let (dw, dh) = cell_size(9, 17, ppx);
            assert_eq!(lw * ppx, dw as f32, "width at ppx {ppx}");
            assert_eq!(lh * ppx, dh as f32, "height at ppx {ppx}");
        }
    }
}
