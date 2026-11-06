//! Instance builders and vertex expanders for GPU glyph and rectangle batches.

use surface::Surface;

use super::gpu_atlas::{ATLAS_CELL_H, ATLAS_CELL_W, GlyphUv, GpuGlyphAtlas};

/// One glyph to draw on-screen this frame.
#[derive(Debug, Clone, Copy)]
pub struct GlyphInstance {
    /// Top-left screen position in pixels.
    pub screen_x: f32,
    pub screen_y: f32,
    /// Normalized [0, 1] UV rect in the atlas texture.
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    /// Foreground RGBA (each component in [0, 1]).
    pub fg: [f32; 4],
}

/// One background rectangle to draw on-screen this frame.
#[derive(Debug, Clone, Copy)]
pub struct RectInstance {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Fill color RGBA (each component in [0, 1]).
    pub color: [f32; 4],
}

/// Fallback UV for characters not in the atlas — maps to the box glyph (□)
/// if present, otherwise to the space cell (top-left, transparent after
/// rasterization but guaranteed white in the placeholder atlas).
const FALLBACK_UV: GlyphUv = GlyphUv {
    x: 0,
    y: 0,
    w: ATLAS_CELL_W,
    h: ATLAS_CELL_H,
};

/// Convert `Surface` cells to a list of `GlyphInstance`s ready for the GPU.
///
/// Space and null characters are skipped.  When `skip_last_row` is true
/// the bottom row (status bar area) is excluded.
pub fn build_glyph_instances(
    surface: &Surface,
    atlas: &GpuGlyphAtlas,
    char_w: f32,
    line_h: f32,
    origin_x: f32,
    origin_y: f32,
    skip_last_row: bool,
) -> Vec<GlyphInstance> {
    let rows = surface.height as usize;
    let cols = surface.width as usize;
    let render_rows = if skip_last_row {
        rows.saturating_sub(1)
    } else {
        rows
    };
    let mut out = Vec::with_capacity(render_rows * cols / 2);

    for sy in 0..render_rows {
        for sx in 0..cols {
            let Some(cell) = surface.cell(sx as u16, sy as u16) else {
                continue;
            };
            if cell.ch == ' ' || cell.ch.is_control() {
                continue;
            }

            // Try the cell's own slant, then the box fallback, then FALLBACK_UV.
            let uv = atlas
                .uv_for(cell.ch, cell.style.italic)
                .unwrap_or(FALLBACK_UV);
            let [uv_x, uv_y, uv_w, uv_h] = atlas.normalize(&uv);
            out.push(GlyphInstance {
                screen_x: origin_x + sx as f32 * char_w,
                screen_y: origin_y + sy as f32 * line_h,
                uv_x,
                uv_y,
                uv_w,
                uv_h,
                fg: [
                    cell.style.fg.0 as f32 / 255.0,
                    cell.style.fg.1 as f32 / 255.0,
                    cell.style.fg.2 as f32 / 255.0,
                    1.0,
                ],
            });
        }
    }
    out
}

/// Convert `Surface` cells to a list of `RectInstance`s for background fills.
///
/// Cells whose background matches `default_bg` are skipped (no rect needed).
pub fn build_rect_instances(
    surface: &Surface,
    char_w: f32,
    line_h: f32,
    origin_x: f32,
    origin_y: f32,
    default_bg: [f32; 4],
    skip_last_row: bool,
) -> Vec<RectInstance> {
    let rows = surface.height as usize;
    let cols = surface.width as usize;
    let render_rows = if skip_last_row {
        rows.saturating_sub(1)
    } else {
        rows
    };
    let mut out = Vec::new();

    for sy in 0..render_rows {
        for sx in 0..cols {
            let Some(cell) = surface.cell(sx as u16, sy as u16) else {
                continue;
            };
            let bg = [
                cell.style.bg.0 as f32 / 255.0,
                cell.style.bg.1 as f32 / 255.0,
                cell.style.bg.2 as f32 / 255.0,
                1.0,
            ];
            if bg == default_bg {
                continue;
            }
            out.push(RectInstance {
                x: origin_x + sx as f32 * char_w,
                y: origin_y + sy as f32 * line_h,
                w: char_w,
                h: line_h,
                color: bg,
            });
        }
    }
    out
}

/// Expand glyph instances into raw vertex data (6 verts per instance).
pub fn glyphs_to_vertices(instances: &[GlyphInstance], char_w: f32, line_h: f32) -> Vec<[f32; 8]> {
    let mut verts = Vec::with_capacity(instances.len() * 6);
    for gi in instances {
        let x0 = gi.screen_x;
        let y0 = gi.screen_y;
        let x1 = x0 + char_w;
        let y1 = y0 + line_h;
        let u0 = gi.uv_x;
        let v0 = gi.uv_y;
        let u1 = u0 + gi.uv_w;
        let v1 = v0 + gi.uv_h;
        let [r, g, b, a] = gi.fg;
        verts.push([x0, y0, u0, v0, r, g, b, a]);
        verts.push([x0, y1, u0, v1, r, g, b, a]);
        verts.push([x1, y0, u1, v0, r, g, b, a]);
        verts.push([x0, y1, u0, v1, r, g, b, a]);
        verts.push([x1, y1, u1, v1, r, g, b, a]);
        verts.push([x1, y0, u1, v0, r, g, b, a]);
    }
    verts
}

/// Expand rect instances into raw vertex data (6 verts per instance).
///
/// `atlas_width` / `atlas_height` are used to compute the UV of the solid-white
/// texel at the atlas origin.  Staying inside that first texel keeps the
/// constant white sample valid whatever cell geometry the atlas uses.
pub fn rects_to_vertices(
    instances: &[RectInstance],
    atlas_width: u32,
    atlas_height: u32,
) -> Vec<[f32; 8]> {
    let mut verts = Vec::with_capacity(instances.len() * 6);
    let u0 = 0.5f32 / atlas_width as f32;
    let v0 = 0.5f32 / atlas_height as f32;
    let u1 = u0;
    let v1 = v0;
    for ri in instances {
        let x0 = ri.x;
        let y0 = ri.y;
        let x1 = x0 + ri.w;
        let y1 = y0 + ri.h;
        let [r, g, b, a] = ri.color;
        verts.push([x0, y0, u0, v0, r, g, b, a]);
        verts.push([x0, y1, u0, v1, r, g, b, a]);
        verts.push([x1, y0, u1, v0, r, g, b, a]);
        verts.push([x0, y1, u0, v1, r, g, b, a]);
        verts.push([x1, y1, u1, v1, r, g, b, a]);
        verts.push([x1, y0, u1, v0, r, g, b, a]);
    }
    verts
}
