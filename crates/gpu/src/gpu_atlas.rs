//! CPU-side glyph atlas: UV map, instance builders, and rect batcher.
//!
//! This module contains no wgpu types — it is testable without a GPU.
//! The GPU texture is created by `gpu_paint_callback` using the UV layout here.

use std::collections::HashMap;

use surface::Surface;

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
    // Arrows and typographic symbols (U+2013, U+2018-2019, U+201C-201D,
    // U+2022, U+2026, U+2190-2193)
    "\u{2013}\u{2018}\u{2019}\u{201C}\u{201D}\u{2022}\u{2026}\u{2190}\u{2191}\u{2192}\u{2193}",
    // Box drawing (U+2500-2517) — PTY tool window borders
    "\u{2500}\u{2501}\u{2502}\u{2503}\u{2504}\u{2505}\u{2506}\u{2507}\
     \u{2508}\u{2509}\u{250A}\u{250B}\u{250C}\u{250D}\u{250E}\u{250F}\
     \u{2510}\u{2511}\u{2512}\u{2513}\u{2514}\u{2515}\u{2516}\u{2517}",
    // Box drawing continued (U+2518-252F)
    "\u{2518}\u{2519}\u{251A}\u{251B}\u{251C}\u{251D}\u{251E}\u{251F}\
     \u{2520}\u{2521}\u{2522}\u{2523}\u{2524}\u{2525}\u{2526}\u{2527}\
     \u{2528}\u{2529}\u{252A}\u{252B}\u{252C}\u{252D}\u{252E}\u{252F}",
    // Box drawing continued (U+2530-2547)
    "\u{2530}\u{2531}\u{2532}\u{2533}\u{2534}\u{2535}\u{2536}\u{2537}\
     \u{2538}\u{2539}\u{253A}\u{253B}\u{253C}\u{253D}\u{253E}\u{253F}\
     \u{2540}\u{2541}\u{2542}\u{2543}\u{2544}\u{2545}\u{2546}\u{2547}",
    // Box drawing tail (U+2548-257F)
    "\u{2548}\u{2549}\u{254A}\u{254B}\u{2550}\u{2551}\u{2552}\u{2553}\
     \u{2554}\u{2555}\u{2556}\u{2557}\u{2558}\u{2559}\u{255A}\u{255B}\
     \u{255C}\u{255D}\u{255E}\u{255F}\u{2560}\u{2561}\u{2562}\u{2563}\
     \u{2564}\u{2565}\u{2566}\u{2567}\u{2568}\u{2569}\u{256A}\u{256B}\
     \u{256C}\u{256D}\u{256E}\u{256F}\u{2570}\u{2571}\u{2572}\u{2573}\
     \u{2574}\u{2575}\u{2576}\u{2577}\u{2578}\u{2579}\u{257A}\u{257B}\
     \u{257C}\u{257D}\u{257E}\u{257F}",
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
    // Cursor shapes and gutter markers — the kernel paints these into the
    // Surface (`█`, `▏`, `▔`, `▌` for cursors, `▾`/`▸` for folds, `●`/`○`
    // for breakpoints, `▎` for git signs).  Without their own cells they
    // would fall back to the box glyph.  The gutter markers are painted as
    // geometry, not font outlines (see shapes), so they render the same
    // whatever font the editor uses.
    "\u{2588}\u{258C}\u{258E}\u{258F}\u{2594}\u{25B8}\u{25BE}\u{25CF}\u{25CB}",
    // Typographic dash used throughout the editor's own docs and comments.
    "\u{2014}",
    // Greek and Cyrillic (F-35: the GPU path is not ASCII-only) — the
    // CJK ranges stay off the atlas; they exceed a fixed-grid texture and
    // fall back to the CPU paint path per glyph.
    "\u{0391}\u{0392}\u{0393}\u{0394}\u{0395}\u{0396}\u{0397}\u{0398}\
     \u{0399}\u{039A}\u{039B}\u{039C}\u{039D}\u{039E}\u{039F}\u{03A0}",
    "\u{03A1}\u{03A3}\u{03A4}\u{03A5}\u{03A6}\u{03A7}\u{03A8}\u{03A9}\
     \u{03B1}\u{03B2}\u{03B3}\u{03B4}\u{03B5}\u{03B6}\u{03B7}\u{03B8}",
    "\u{03B9}\u{03BA}\u{03BB}\u{03BC}\u{03BD}\u{03BE}\u{03BF}\u{03C0}\
     \u{03C1}\u{03C2}\u{03C3}\u{03C4}\u{03C5}\u{03C6}\u{03C7}\u{03C8}",
    "\u{03C9}\u{0401}\u{0410}\u{0411}\u{0412}\u{0413}\u{0414}\u{0415}\
     \u{0416}\u{0417}\u{0418}\u{0419}\u{041A}\u{041B}\u{041C}\u{041D}",
    "\u{041E}\u{041F}\u{0420}\u{0421}\u{0422}\u{0423}\u{0424}\u{0425}\
     \u{0426}\u{0427}\u{0428}\u{0429}\u{042A}\u{042B}\u{042C}\u{042D}",
    "\u{042E}\u{042F}\u{0430}\u{0431}\u{0432}\u{0433}\u{0434}\u{0435}\
     \u{0436}\u{0437}\u{0438}\u{0439}\u{043A}\u{043B}\u{043C}\u{043D}",
    "\u{043E}\u{043F}\u{0440}\u{0441}\u{0442}\u{0443}\u{0444}\u{0445}\
     \u{0446}\u{0447}\u{0448}\u{0449}\u{044A}\u{044B}\u{044C}\u{044D}",
    "\u{044E}\u{044F}\u{0451}",
);

// 95 ASCII + 1 box + 96 Latin-1 = 192 chars → 12 rows of 16
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
    /// The same characters as their synthetic oblique twins, in a second
    /// block of the texture below `glyphs`.
    pub glyphs_italic: HashMap<char, GlyphUv>,
    /// Total atlas texture width (pixels).
    pub atlas_width: u32,
    /// Total atlas texture height (pixels).
    pub atlas_height: u32,
    /// Width of one glyph cell (pixels).  Grows with `pixels_per_point` so a
    /// device-resolution glyph never spills into its neighbour.
    pub cell_w: u32,
    /// Height of one glyph cell (pixels).
    pub cell_h: u32,
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
        Self::with_cell_size(ATLAS_CELL_W, ATLAS_CELL_H)
    }

    /// Build the atlas with an explicit glyph-cell size.
    ///
    /// The cell must be large enough for the rasterized font: at a device
    /// pixel ratio of 2 the glyphs are drawn at twice the point size, so the
    /// cells double too.  `rasterize_at_ppx` uses this to keep glyphs from
    /// bleeding into their neighbours.
    pub fn with_cell_size(cell_w: u32, cell_h: u32) -> Self {
        let chars: Vec<char> = ATLAS_CHARS.chars().collect();
        let total = chars.len() as u32;
        // The grid derives from the live character set — no hand-maintained
        // row constant to drift (the old 12-row constant silently clipped
        // the 331-char set the GPU path had grown to).
        let rows = total.div_ceil(ATLAS_COLS);
        let atlas_width = ATLAS_COLS * cell_w;
        // Two blocks: the roman glyphs, then their oblique twins.
        let atlas_height = rows * cell_h * 2;

        let cell = |row: u32, col: u32, block: u32| GlyphUv {
            x: col * cell_w,
            y: (block * rows + row) * cell_h,
            w: cell_w,
            h: cell_h,
        };
        let mut glyphs = HashMap::with_capacity(chars.len());
        let mut glyphs_italic = HashMap::with_capacity(chars.len());
        for (i, ch) in chars.iter().enumerate() {
            let col = (i as u32) % ATLAS_COLS;
            let row = (i as u32) / ATLAS_COLS;
            glyphs.insert(*ch, cell(row, col, 0));
            glyphs_italic.insert(*ch, cell(row, col, 1));
        }

        let atlas_pixels = vec![255u8; (atlas_width * atlas_height * 4) as usize];
        GpuGlyphAtlas {
            glyphs,
            glyphs_italic,
            atlas_width,
            atlas_height,
            cell_w,
            cell_h,
            dirty: true,
            atlas_pixels,
        }
    }

    /// Rasterize `font_bytes` into `atlas_pixels` using `ab_glyph`.
    ///
    /// Sets `dirty = true` so the GPU texture is re-uploaded next frame.
    pub fn rasterize(&mut self, font_bytes: &[u8], font_size: f32) {
        let chars: Vec<char> = ATLAS_CHARS.chars().collect();
        let roman = crate::gpu_rasterizer::rasterize_font_atlas(
            font_bytes,
            font_size,
            &chars,
            self.cell_w,
            self.cell_h,
            ATLAS_COLS,
        );
        let oblique = crate::gpu_rasterizer::rasterize_oblique_atlas(
            font_bytes,
            font_size,
            &chars,
            self.cell_w,
            self.cell_h,
            ATLAS_COLS,
        );
        self.atlas_pixels = roman;
        self.atlas_pixels.extend_from_slice(&oblique);
        self.dirty = true;
    }

    /// Pixel UV of `ch`, in the oblique block when `italic`.  Characters the
    /// font does not draw fall back to the box glyph of the same block, so
    /// missing glyphs stay upright rather than leaning as a box.
    pub fn uv_for(&self, ch: char, italic: bool) -> Option<GlyphUv> {
        let block = if italic {
            &self.glyphs_italic
        } else {
            &self.glyphs
        };
        block
            .get(&ch)
            .or_else(|| block.get(&'\u{25A1}'))
            .copied()
    }

    /// Rasterize at device resolution (F-35): `pixels_per_point` scales the
    /// font *and* the glyph cells so HiDPI displays sample denser texels
    /// instead of stretching the 1× atlas.
    ///
    /// `cell_w`/`cell_h` are the frontend's logical grid cell in points.  The
    /// atlas cell is rounded exactly like the frontend rounds its screen cell
    /// (`cell * ppx`), so the blit is a straight 1:1 texel copy and a glyph
    /// fills its cell instead of floating in padded atlas space.
    pub fn rasterize_for_grid(
        &mut self,
        font_bytes: &[u8],
        font_size: f32,
        ppx: f32,
        cell_w: u32,
        cell_h: u32,
    ) {
        let ppx = ppx.max(1.0);
        let cell_w = (cell_w as f32 * ppx).round().max(1.0) as u32;
        let cell_h = (cell_h as f32 * ppx).round().max(1.0) as u32;
        if cell_w != self.cell_w || cell_h != self.cell_h {
            *self = Self::with_cell_size(cell_w, cell_h);
        }
        self.rasterize(font_bytes, font_size * ppx);
    }

    /// Device-resolution rasterization with the default atlas cell geometry.
    pub fn rasterize_at_ppx(&mut self, font_bytes: &[u8], font_size: f32, ppx: f32) {
        self.rasterize_for_grid(font_bytes, font_size, ppx, ATLAS_CELL_W, ATLAS_CELL_H);
    }

    /// Return the atlas pixel data (RGBA, row-major) for GPU texture upload.
    pub fn generate_atlas_pixels(&self) -> &[u8] {
        &self.atlas_pixels
    }

    pub fn needs_rebuild(&self) -> bool {
        self.dirty
    }
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    pub fn contains(&self, ch: char) -> bool {
        self.glyphs.contains_key(&ch)
    }

    /// Normalize a pixel UV to the `[0, 1]` range expected by the shader.
    pub fn normalize(&self, uv: &GlyphUv) -> [f32; 4] {
        [
            uv.x as f32 / self.atlas_width as f32,
            uv.y as f32 / self.atlas_height as f32,
            uv.w as f32 / self.atlas_width as f32,
            uv.h as f32 / self.atlas_height as f32,
        ]
    }
}

impl Default for GpuGlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}

// ── Instance builders ─────────────────────────────────────────────────────────

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
