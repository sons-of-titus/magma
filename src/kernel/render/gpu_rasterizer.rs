//! Real glyph rasterization using `ab_glyph` — Phase 4.
//!
//! Produces an RGBA pixel buffer for the GPU atlas texture.  Each character
//! occupies a fixed `cell_w × cell_h` cell in a grid with `cols` columns.
//! The glyph is horizontally centred in the cell and vertically positioned
//! at the font's ascent metric so all baselines align.
//!
//! Falls back to a solid-white atlas if the font bytes fail to parse, which
//! preserves the existing placeholder behaviour.

use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _};

use super::gpu_atlas::{ATLAS_CELL_H, ATLAS_CELL_W};

/// Rasterize `chars` into an RGBA atlas pixel buffer.
///
/// Returns flat RGBA bytes (4 bytes per pixel, row-major).
pub fn rasterize_font_atlas(
    font_bytes: &[u8],
    font_size: f32,
    chars: &[char],
    cell_w: u32,
    cell_h: u32,
    cols: u32,
) -> Vec<u8> {
    let rows   = (chars.len() as u32 + cols - 1) / cols;
    let width  = cols * cell_w;
    let height = rows * cell_h;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    // First cell (space, index 0) is kept solid white so background rects
    // can sample it and get the expected opaque white fill.
    fill_cell_white(&mut pixels, 0, 0, cell_w, cell_h, width);

    let Ok(font) = FontRef::try_from_slice(font_bytes) else {
        // Font load failed — return all-white placeholder.
        return vec![255u8; (width * height * 4) as usize];
    };
    let scale  = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let ascent = scaled.ascent();

    for (i, &ch) in chars.iter().enumerate() {
        let col    = (i as u32) % cols;
        let row    = (i as u32) / cols;
        let cell_x = col * cell_w;
        let cell_y = row * cell_h;

        let glyph_id  = scaled.glyph_id(ch);
        let h_advance = scaled.h_advance(glyph_id);
        // Centre glyph horizontally; clamp so it never starts left of the cell.
        let h_offset  = ((cell_w as f32 - h_advance) / 2.0).max(0.0);

        let glyph = glyph_id.with_scale_and_position(
            scale,
            ab_glyph::point(cell_x as f32 + h_offset, cell_y as f32 + ascent),
        );
        let Some(outlined) = font.outline_glyph(glyph) else { continue };

        // draw() gives absolute pixel coords (screen-space origin is the atlas).
        outlined.draw(|px, py, cov| {
            if cov < 0.004 || px >= width || py >= height { return }
            let idx = ((py * width + px) * 4) as usize;
            // White glyph with coverage alpha; colour is applied by the shader.
            pixels[idx]     = 255;
            pixels[idx + 1] = 255;
            pixels[idx + 2] = 255;
            pixels[idx + 3] = (cov * 255.0).min(255.0) as u8;
        });
    }
    pixels
}

/// Fill one atlas cell with solid white (used to guarantee a white pixel for
/// background rect sampling regardless of what the font provides for space).
fn fill_cell_white(pixels: &mut [u8], cell_x: u32, cell_y: u32, cell_w: u32, cell_h: u32, atlas_w: u32) {
    for y in cell_y..cell_y + cell_h {
        for x in cell_x..cell_x + cell_w {
            let idx = ((y * atlas_w + x) * 4) as usize;
            if idx + 3 < pixels.len() {
                pixels[idx]     = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 255;
                pixels[idx + 3] = 255;
            }
        }
    }
}

// Suppress unused-import warning when the constants are only referenced here.
const _: u32 = ATLAS_CELL_W;
const _: u32 = ATLAS_CELL_H;
