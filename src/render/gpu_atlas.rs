//! CPU-side glyph atlas: UV map, instance builders, and rect batcher.
//!
//! This module contains no wgpu types — it is testable without a GPU.
//! The GPU texture is created by `gpu_paint_callback` using the UV layout here.

use std::collections::HashMap;

use crate::render::surface::Surface;

// ── Atlas geometry constants ──────────────────────────────────────────────────

/// Width of each glyph cell in the atlas texture (pixels).
pub const ATLAS_CELL_W: u32 = 16;
/// Height of each glyph cell in the atlas texture (pixels).
pub const ATLAS_CELL_H: u32 = 24;
/// Number of glyph columns in the atlas grid.
pub const ATLAS_COLS: u32 = 16;

/// The set of characters baked into the atlas at startup.
/// Covers all printable ASCII.  Extended Unicode ranges are added by Sprint 10.
pub const ATLAS_CHARS: &str =
    " !\"#$%&'()*+,-./\
     0123456789:;<=>?\
     @ABCDEFGHIJKLMNO\
     PQRSTUVWXYZ[\\]^_\
     `abcdefghijklmno\
     pqrstuvwxyz{|}~";

// ── Data structures ───────────────────────────────────────────────────────────

/// Pixel-space UV coordinates of one glyph cell in the atlas texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphUv {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

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

// ── Atlas ─────────────────────────────────────────────────────────────────────

/// CPU-side glyph atlas: maps characters to their UV positions in the texture.
///
/// The atlas texture itself is managed by `gpu_paint_callback`.  This struct
/// holds only the coordinate map and the dirty flag.
pub struct GpuGlyphAtlas {
    /// Maps each character to its pixel-space UV cell.
    pub glyphs: HashMap<char, GlyphUv>,
    /// Total atlas texture width (pixels).
    pub atlas_width: u32,
    /// Total atlas texture height (pixels).
    pub atlas_height: u32,
    /// True when the atlas texture must be re-uploaded to the GPU.
    pub dirty: bool,
}

impl GpuGlyphAtlas {
    /// Build the atlas with a fixed grid layout for all `ATLAS_CHARS`.
    ///
    /// Each character gets an `ATLAS_CELL_W × ATLAS_CELL_H` cell.
    /// Actual glyph pixels are uploaded by the GPU path; this only computes positions.
    pub fn new() -> Self {
        let chars: Vec<char> = ATLAS_CHARS.chars().collect();
        let total = chars.len() as u32;
        let rows = (total + ATLAS_COLS - 1) / ATLAS_COLS;
        let atlas_width = ATLAS_COLS * ATLAS_CELL_W;
        let atlas_height = rows * ATLAS_CELL_H;

        let mut glyphs = HashMap::with_capacity(chars.len());
        for (i, ch) in chars.iter().enumerate() {
            let col = (i as u32) % ATLAS_COLS;
            let row = (i as u32) / ATLAS_COLS;
            glyphs.insert(*ch, GlyphUv {
                x: col * ATLAS_CELL_W,
                y: row * ATLAS_CELL_H,
                w: ATLAS_CELL_W,
                h: ATLAS_CELL_H,
            });
        }

        GpuGlyphAtlas { glyphs, atlas_width, atlas_height, dirty: true }
    }

    /// Return `true` if the atlas texture needs re-uploading to the GPU.
    pub fn needs_rebuild(&self) -> bool { self.dirty }

    /// Call after the atlas texture is uploaded; clears the dirty flag.
    pub fn mark_clean(&mut self) { self.dirty = false; }

    /// Force a rebuild on the next frame (e.g. after a `font-changed` event).
    pub fn mark_dirty(&mut self) { self.dirty = true; }

    /// Return `true` if `ch` has an entry in the atlas UV map.
    pub fn contains(&self, ch: char) -> bool { self.glyphs.contains_key(&ch) }

    /// Normalize a pixel UV to the `[0, 1]` range expected by the shader.
    pub fn normalize(&self, uv: &GlyphUv) -> [f32; 4] {
        [
            uv.x as f32 / self.atlas_width as f32,
            uv.y as f32 / self.atlas_height as f32,
            uv.w as f32 / self.atlas_width as f32,
            uv.h as f32 / self.atlas_height as f32,
        ]
    }

    /// Generate raw RGBA pixel data for the atlas texture (placeholder: solid white).
    /// Sprint 10 replaces this with real rasterized glyph bitmaps.
    pub fn generate_placeholder_pixels(&self) -> Vec<u8> {
        let pixel_count = (self.atlas_width * self.atlas_height) as usize;
        vec![255u8; pixel_count * 4]
    }
}

impl Default for GpuGlyphAtlas {
    fn default() -> Self { Self::new() }
}

// ── Instance builders ─────────────────────────────────────────────────────────

/// Fallback UV for characters not in the atlas (maps to top-left cell).
const FALLBACK_UV: GlyphUv = GlyphUv { x: 0, y: 0, w: ATLAS_CELL_W, h: ATLAS_CELL_H };

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
    let render_rows = if skip_last_row { rows.saturating_sub(1) } else { rows };
    let mut out = Vec::with_capacity(render_rows * cols / 2);

    for sy in 0..render_rows {
        for sx in 0..cols {
            let Some(cell) = surface.cell(sx as u16, sy as u16) else { continue };
            if cell.ch == ' ' || cell.ch == '\0' { continue }

            let uv = atlas.glyphs.get(&cell.ch).copied().unwrap_or(FALLBACK_UV);
            let [uv_x, uv_y, uv_w, uv_h] = atlas.normalize(&uv);
            out.push(GlyphInstance {
                screen_x: origin_x + sx as f32 * char_w,
                screen_y: origin_y + sy as f32 * line_h,
                uv_x, uv_y, uv_w, uv_h,
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
    let render_rows = if skip_last_row { rows.saturating_sub(1) } else { rows };
    let mut out = Vec::new();

    for sy in 0..render_rows {
        for sx in 0..cols {
            let Some(cell) = surface.cell(sx as u16, sy as u16) else { continue };
            let bg = [
                cell.style.bg.0 as f32 / 255.0,
                cell.style.bg.1 as f32 / 255.0,
                cell.style.bg.2 as f32 / 255.0,
                1.0,
            ];
            if bg == default_bg { continue }
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

/// Expand a slice of instances into raw vertex data (6 vertices per instance,
/// laid out as `[pos.x, pos.y, uv.x, uv.y, r, g, b, a]`).
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
        // Two triangles: (TL, BL, TR) + (BL, BR, TR)
        verts.push([x0, y0, u0, v0, r, g, b, a]);
        verts.push([x0, y1, u0, v1, r, g, b, a]);
        verts.push([x1, y0, u1, v0, r, g, b, a]);
        verts.push([x0, y1, u0, v1, r, g, b, a]);
        verts.push([x1, y1, u1, v1, r, g, b, a]);
        verts.push([x1, y0, u1, v0, r, g, b, a]);
    }
    verts
}

/// Expand a slice of rect instances into raw vertex data.
pub fn rects_to_vertices(instances: &[RectInstance]) -> Vec<[f32; 8]> {
    let mut verts = Vec::with_capacity(instances.len() * 6);
    for ri in instances {
        let x0 = ri.x;
        let y0 = ri.y;
        let x1 = x0 + ri.w;
        let y1 = y0 + ri.h;
        let [r, g, b, a] = ri.color;
        // UV (0,0)→(1,1) maps to the solid-white 1×1 area we reserve in the atlas.
        let (u0, v0, u1, v1) = (0.0f32, 0.0, 1.0f32 / ATLAS_COLS as f32, 1.0f32 / ((ATLAS_CHARS.len() as u32 + ATLAS_COLS - 1) / ATLAS_COLS) as f32);
        verts.push([x0, y0, u0, v0, r, g, b, a]);
        verts.push([x0, y1, u0, v1, r, g, b, a]);
        verts.push([x1, y0, u1, v0, r, g, b, a]);
        verts.push([x0, y1, u0, v1, r, g, b, a]);
        verts.push([x1, y1, u1, v1, r, g, b, a]);
        verts.push([x1, y0, u1, v0, r, g, b, a]);
    }
    verts
}
