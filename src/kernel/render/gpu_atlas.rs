//! CPU-side glyph atlas: UV map, instance builders, and rect batcher.
//!
//! This module contains no wgpu types — it is testable without a GPU.
//! The GPU texture is created by `gpu_paint_callback` using the UV layout here.

use std::collections::HashMap;

use crate::kernel::render::surface::Surface;

// ── Atlas geometry constants ──────────────────────────────────────────────────

/// Width of each glyph cell in the atlas texture (pixels).
pub const ATLAS_CELL_W: u32 = 16;
/// Height of each glyph cell in the atlas texture (pixels).
pub const ATLAS_CELL_H: u32 = 24;
/// Number of glyph columns in the atlas grid.
pub const ATLAS_COLS: u32 = 16;

/// Full set of characters baked into the atlas at startup.
/// Covers printable ASCII, a box-fallback glyph (□), and the Latin-1
/// Supplement (U+00A0–U+00FF) for accented-Latin support.
pub const ATLAS_CHARS: &str = concat!(
    // Printable ASCII (95 chars, U+0020–U+007E)
    " !\"#$%&'()*+,-./",
    "0123456789:;<=>?",
    "@ABCDEFGHIJKLMNO",
    "PQRSTUVWXYZ[\\]^_",
    "`abcdefghijklmno",
    "pqrstuvwxyz{|}~",
    // Fallback box character (1 char, U+25A1)
    "\u{25A1}",
    // Latin-1 Supplement (96 chars, U+00A0–U+00FF)
    "\u{00A0}\u{00A1}\u{00A2}\u{00A3}\u{00A4}\u{00A5}\u{00A6}\u{00A7}\
     \u{00A8}\u{00A9}\u{00AA}\u{00AB}\u{00AC}\u{00AD}\u{00AE}\u{00AF}",
    "\u{00B0}\u{00B1}\u{00B2}\u{00B3}\u{00B4}\u{00B5}\u{00B6}\u{00B7}\
     \u{00B8}\u{00B9}\u{00BA}\u{00BB}\u{00BC}\u{00BD}\u{00BE}\u{00BF}",
    "\u{00C0}\u{00C1}\u{00C2}\u{00C3}\u{00C4}\u{00C5}\u{00C6}\u{00C7}\
     \u{00C8}\u{00C9}\u{00CA}\u{00CB}\u{00CC}\u{00CD}\u{00CE}\u{00CF}",
    "\u{00D0}\u{00D1}\u{00D2}\u{00D3}\u{00D4}\u{00D5}\u{00D6}\u{00D7}\
     \u{00D8}\u{00D9}\u{00DA}\u{00DB}\u{00DC}\u{00DD}\u{00DE}\u{00DF}",
    "\u{00E0}\u{00E1}\u{00E2}\u{00E3}\u{00E4}\u{00E5}\u{00E6}\u{00E7}\
     \u{00E8}\u{00E9}\u{00EA}\u{00EB}\u{00EC}\u{00ED}\u{00EE}\u{00EF}",
    "\u{00F0}\u{00F1}\u{00F2}\u{00F3}\u{00F4}\u{00F5}\u{00F6}\u{00F7}\
     \u{00F8}\u{00F9}\u{00FA}\u{00FB}\u{00FC}\u{00FD}\u{00FE}\u{00FF}",
);

// 95 ASCII + 1 box + 96 Latin-1 = 192 chars → 12 rows of 16
pub const ATLAS_CHAR_COUNT: u32 = 192;
pub const ATLAS_ROWS: u32 = 12; // = ceil(192 / 16)

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

/// CPU-side glyph atlas: maps characters to their UV positions in the texture
/// and holds the RGBA pixel buffer for upload to the GPU.
pub struct GpuGlyphAtlas {
    /// Maps each character to its pixel-space UV cell.
    pub glyphs: HashMap<char, GlyphUv>,
    /// Total atlas texture width (pixels).
    pub atlas_width: u32,
    /// Total atlas texture height (pixels).
    pub atlas_height: u32,
    /// True when the atlas texture must be re-uploaded to the GPU.
    pub dirty: bool,
    /// RGBA pixel data (4 bytes per pixel, row-major).
    /// Placeholder (solid white) until `rasterize()` is called.
    pub atlas_pixels: Vec<u8>,
}

impl GpuGlyphAtlas {
    /// Build the atlas with a fixed grid layout for all `ATLAS_CHARS`.
    ///
    /// `atlas_pixels` is initialised to solid white (placeholder) so that
    /// background rects render correctly even before `rasterize()` is called.
    pub fn new() -> Self {
        let chars: Vec<char> = ATLAS_CHARS.chars().collect();
        let total = chars.len() as u32;
        let rows = (total + ATLAS_COLS - 1) / ATLAS_COLS;
        let atlas_width  = ATLAS_COLS * ATLAS_CELL_W;
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

        let atlas_pixels = vec![255u8; (atlas_width * atlas_height * 4) as usize];
        GpuGlyphAtlas { glyphs, atlas_width, atlas_height, dirty: true, atlas_pixels }
    }

    /// Rasterize `font_bytes` into `atlas_pixels` using `ab_glyph`.
    ///
    /// Sets `dirty = true` so the GPU texture is re-uploaded next frame.
    #[cfg(feature = "gui")]
    pub fn rasterize(&mut self, font_bytes: &[u8], font_size: f32) {
        let chars: Vec<char> = ATLAS_CHARS.chars().collect();
        self.atlas_pixels = crate::kernel::render::gpu_rasterizer::rasterize_font_atlas(
            font_bytes, font_size, &chars, ATLAS_CELL_W, ATLAS_CELL_H, ATLAS_COLS,
        );
        self.dirty = true;
    }

    /// Return the atlas pixel data (RGBA, row-major) for GPU texture upload.
    pub fn generate_atlas_pixels(&self) -> &[u8] { &self.atlas_pixels }

    pub fn needs_rebuild(&self) -> bool { self.dirty }
    pub fn mark_clean(&mut self)        { self.dirty = false; }
    pub fn mark_dirty(&mut self)        { self.dirty = true; }
    pub fn contains(&self, ch: char) -> bool { self.glyphs.contains_key(&ch) }

    /// Normalize a pixel UV to the `[0, 1]` range expected by the shader.
    pub fn normalize(&self, uv: &GlyphUv) -> [f32; 4] {
        [
            uv.x as f32 / self.atlas_width  as f32,
            uv.y as f32 / self.atlas_height as f32,
            uv.w as f32 / self.atlas_width  as f32,
            uv.h as f32 / self.atlas_height as f32,
        ]
    }
}

impl Default for GpuGlyphAtlas {
    fn default() -> Self { Self::new() }
}

// ── Instance builders ─────────────────────────────────────────────────────────

/// Fallback UV for characters not in the atlas — maps to the box glyph (□)
/// if present, otherwise to the space cell (top-left, transparent after
/// rasterization but guaranteed white in the placeholder atlas).
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

            // Try the exact char, then the box fallback, then FALLBACK_UV.
            let uv = atlas.glyphs.get(&cell.ch)
                .or_else(|| atlas.glyphs.get(&'\u{25A1}'))
                .copied()
                .unwrap_or(FALLBACK_UV);
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

/// Expand glyph instances into raw vertex data (6 verts per instance).
pub fn glyphs_to_vertices(instances: &[GlyphInstance], char_w: f32, line_h: f32) -> Vec<[f32; 8]> {
    let mut verts = Vec::with_capacity(instances.len() * 6);
    for gi in instances {
        let x0 = gi.screen_x;        let y0 = gi.screen_y;
        let x1 = x0 + char_w;        let y1 = y0 + line_h;
        let u0 = gi.uv_x;            let v0 = gi.uv_y;
        let u1 = u0 + gi.uv_w;       let v1 = v0 + gi.uv_h;
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
/// pixel reserved in the first atlas cell (the space character cell).
pub fn rects_to_vertices(instances: &[RectInstance], atlas_width: u32, atlas_height: u32) -> Vec<[f32; 8]> {
    let mut verts = Vec::with_capacity(instances.len() * 6);
    // Sample from the centre of the first cell (guaranteed solid white).
    let u0 = 0.0f32;
    let v0 = 0.0f32;
    let u1 = ATLAS_CELL_W as f32 / atlas_width  as f32;
    let v1 = ATLAS_CELL_H as f32 / atlas_height as f32;
    for ri in instances {
        let x0 = ri.x;       let y0 = ri.y;
        let x1 = x0 + ri.w;  let y1 = y0 + ri.h;
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
