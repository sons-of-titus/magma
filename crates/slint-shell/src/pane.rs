//! Surface → Slint image: the editor pane's presentation path.
//!
//! The kernel renders editor semantics into a character grid.  This module
//! turns that grid into RGBA pixels using the glyph atlas, then hands them to
//! Slint as an `Image`.  No widget toolkit types reach the kernel.

use magma::kernel::render::gpu_atlas::GpuGlyphAtlas;
use magma::kernel::render::surface::Surface;
use slint::SharedPixelBuffer;

pub const CHAR_W: u32 = 10;
pub const LINE_H: u32 = 20;
pub const RAIL_W: f32 = 32.0;
pub const SIDEBAR_W: f32 = 240.0;
const CHROME_H: f32 = 30.0 + 30.0 + 22.0 + 24.0;

pub fn grid_size(logical_w: f32, logical_h: f32) -> (u32, u32) {
    let pane_w = (logical_w - RAIL_W - SIDEBAR_W).max(80.0);
    let pane_h = (logical_h - CHROME_H).max(40.0);
    (
        (pane_w / CHAR_W as f32).floor().max(1.0) as u32,
        (pane_h / LINE_H as f32).floor().max(1.0) as u32,
    )
}

pub fn blit_rgba(surface: &Surface, atlas: &GpuGlyphAtlas) -> (Vec<u8>, u32, u32) {
    let w = surface.width as u32 * CHAR_W;
    let h = surface.height as u32 * LINE_H;
    let mut dst = vec![0u8; (w * h * 4) as usize];
    let aw = atlas.atlas_width;

    for cy in 0..surface.height {
        for cx in 0..surface.width {
            let Some(cell) = surface.cell(cx, cy) else { continue };
            let (br, bg, bb) = cell.style.bg;
            let (mut fr, mut fg, mut fb) = cell.style.fg;
            if cell.style.bold {
                fr = fr.saturating_add(24);
                fg = fg.saturating_add(24);
                fb = fb.saturating_add(24);
            }
            let glyph = if cell.ch == ' ' { None } else { atlas.glyphs.get(&cell.ch) };

            for py in 0..LINE_H {
                for px in 0..CHAR_W {
                    let mut alpha = 0u8;
                    if let Some(uv) = glyph {
                        let sx = uv.x + px * uv.w / CHAR_W;
                        let sy = uv.y + py * uv.h / LINE_H;
                        let src = ((sy * aw + sx) * 4 + 3) as usize;
                        alpha = atlas.atlas_pixels.get(src).copied().unwrap_or(0);
                    }
                    let o = (((cy as u32 * LINE_H + py) * w + cx as u32 * CHAR_W + px) * 4) as usize;
                    dst[o]     = blend(fr, br, alpha);
                    dst[o + 1] = blend(fg, bg, alpha);
                    dst[o + 2] = blend(fb, bb, alpha);
                    dst[o + 3] = 255;
                }
            }
        }
    }
    (dst, w, h)
}

pub fn blit(surface: &Surface, atlas: &GpuGlyphAtlas) -> slint::Image {
    let (rgba, w, h) = blit_rgba(surface, atlas);
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
    buffer.make_mut_bytes().copy_from_slice(&rgba);
    slint::Image::from_rgba8(buffer)
}

pub const MINIMAP_W: u32 = 90;

/// Resolve a terminal Color to RGB using a 16-color palette.
fn term_color(c: magma::kernel::terminal::Color, palette: &[(u8,u8,u8); 16], default_fg: (u8,u8,u8)) -> (u8,u8,u8) {
    use magma::kernel::terminal::Color;
    match c {
        Color::Default => default_fg,
        Color::Rgb(r,g,b) => (r,g,b),
        Color::Indexed(i) => palette[(i as usize).min(15)],
    }
}

pub fn blit_minimap(
    ed: &magma::kernel::state::Editor,
    buf_id: usize,
    width: u32,
    height: u32,
    scroll_top: usize,
    visible_rows: usize,
) -> slint::Image {
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width.max(1), height.max(1));
    let dst = buffer.make_mut_bytes();
    let w = width.max(1);
    let h = height.max(1);

    // background crust
    let (cr, cg, cb) = ed.theme_color("bg");
    for i in (0..dst.len()).step_by(4) {
        dst[i] = cr; dst[i+1] = cg; dst[i+2] = cb; dst[i+3] = 255;
    }

    let arc = match ed.buffers.get(buf_id) { Some(a) => a, None => return slint::Image::from_rgba8(buffer) };
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let total = buf.line_count();
    if total == 0 { return slint::Image::from_rgba8(buffer); }

    let max_rows = ((h as f32 / 1.5).floor() as usize).max(1);
    let step = if total <= max_rows { 1 } else { total.div_ceil(max_rows) };
    let row_h = (h as f32 / total as f32).clamp(1.5, 3.0);

    // collect highlights
    let mut hl: Vec<(usize, usize, String)> = Vec::new();
    if let Some(v) = ed.view_of_buffer(buf_id) {
        for (s, e, f) in &v.highlights { hl.push((*s, *e, f.clone())); }
        for layer in v.highlight_layers.values() {
            for (s, e, f) in layer { hl.push((*s, *e, f.clone())); }
        }
    }

    let base_fg = ed.theme_color("fg");

    let mut line_idx = 0usize;
    let mut row = 0usize;
    while line_idx < total {
        if line_idx.is_multiple_of(step) {
            let span = buf.line_span(line_idx).unwrap_or(0..0);
            let text = buf.slice(span.start, span.end);
            let trimmed = text.trim_end();
            let chars = trimmed.chars().count();
            if chars > 0 {
                let indent = text.len() - text.trim_start_matches([' ', '\t']).len();
                let indent_px = (indent.min(32) as f32 / 2.0 * 1.8).min(32.0) as i32;
                let bar_px = (((chars.min(64) as f32 / 64.0) * (MINIMAP_W as f32 - 8.0)).max(1.0)) as u32;
                let y = (row as f32 * row_h) as u32;
                let bar_h = (row_h * 0.66).max(1.0) as u32;

                // runs
                let mut runs: Vec<(usize, (u8,u8,u8))> = Vec::new();
                for (i, _) in trimmed.char_indices() {
                    let byte = span.start + i;
                    let face = hl.iter().rev().find(|(s, e, _)| *s <= byte && byte < *e).map(|(_, _, f)| f.as_str());
                    let color = face.and_then(|f| ed.faces.get(f)).map(|st| st.fg).unwrap_or(base_fg);
                    let col = trimmed[..i].chars().count();
                    match runs.last_mut() {
                        Some((_, c)) if *c == color => {}
                        _ => runs.push((col, color)),
                    }
                }
                let mut prev_col = 0usize;
                for (i, (col, color)) in runs.iter().enumerate() {
                    let next_col = runs.get(i + 1).map(|(c, _)| *c).unwrap_or(chars);
                    let run_w = (((next_col - prev_col) as f32 / chars as f32) * bar_px as f32).max(1.0) as u32;
                    if run_w >= 1 {
                        let x0 = 4 + indent_px as u32 + ((prev_col as f32 / chars as f32) * bar_px as f32) as u32;
                        for py in 0..bar_h {
                            for px in 0..run_w {
                                let xi = x0 + px;
                                let yi = y + py;
                                if xi < w && yi < h {
                                    let o = ((yi * w + xi) * 4) as usize;
                                    dst[o] = color.0; dst[o+1] = color.1; dst[o+2] = color.2; dst[o+3] = 255;
                                }
                            }
                        }
                    }
                    prev_col = *col;
                }
            }
            row += 1;
        }
        line_idx += 1;
    }

    // viewport rect
    let vp_top = ((scroll_top / step) as f32 * row_h) as u32;
    let vp_rows = (((visible_rows / step).max(1) as f32) * row_h).max(row_h) as u32;
    let vp_h = vp_rows.min(h.saturating_sub(vp_top));
    for py in 0..vp_h {
        for px in 0..(w.saturating_sub(2)) {
            let xi = 1 + px;
            let yi = vp_top + py;
            if xi < w && yi < h {
                let o = ((yi * w + xi) * 4) as usize;
                // stroke: only border
                if px == 0 || px == w.saturating_sub(3) || py == 0 || py == vp_h.saturating_sub(1) {
                    let (r,g,b) = ed.theme_color("fg");
                    dst[o] = r; dst[o+1] = g; dst[o+2] = b; dst[o+3] = 255;
                }
            }
        }
    }

    slint::Image::from_rgba8(buffer)
}

pub fn blit_terminal(
    lines: &[magma::kernel::terminal::screen::StyledLine],
    atlas: &magma::kernel::render::gpu_atlas::GpuGlyphAtlas,
    width: u32,
    height: u32,
) -> slint::Image {
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width.max(1), height.max(1));
    let dst = buffer.make_mut_bytes();
    let w = width.max(1);
    let h = height.max(1);

    let palette: [(u8,u8,u8); 16] = [
        (30,31,34), (243,139,168), (166,227,161), (249,226,175),
        (137,180,250), (203,166,247), (148,226,213), (157,160,168),
        (67,69,74), (243,139,168), (166,227,161), (249,226,175),
        (137,180,250), (203,166,247), (148,226,213), (223,225,229),
    ];
    let default_fg = (223u8, 225u8, 229u8);
    let default_bg = (30u8, 31u8, 34u8);

    for i in (0..dst.len()).step_by(4) {
        dst[i] = default_bg.0; dst[i+1] = default_bg.1; dst[i+2] = default_bg.2; dst[i+3] = 255;
    }

    let row_h: u32 = LINE_H;
    let max_lines = (h / row_h).max(1) as usize;
    let lines: Vec<&magma::kernel::terminal::screen::StyledLine> = lines.iter().rev().take(max_lines).rev().collect();

    for (ri, line) in lines.iter().enumerate() {
        let y0 = ri as u32 * row_h;
        for span in &line.spans {
            let style = span.style;
            let (mut fr, mut fg, mut fb) = term_color(style.fg, &palette, default_fg);
            let (br, bg, bb) = term_color(style.bg, &palette, default_bg);
            if style.bold { fr = fr.saturating_add(24); fg = fg.saturating_add(24); fb = fb.saturating_add(24); }
            if style.dim { fr /= 2; fg /= 2; fb /= 2; }
            let start = span.start as usize;
            let len = span.len as usize;
            let end = (start + len).min(line.text.len());
            // render each char in span
            for (ci, ch) in line.text[start..end].chars().enumerate() {
                if ch == ' ' { continue; }
                let glyph = atlas.glyphs.get(&ch);
                let aw = atlas.atlas_width;
                let x0 = (start + ci) as u32 * CHAR_W;
                if let Some(uv) = glyph {
                    for py in 0..row_h {
                        for px in 0..10u32 {
                            let sx = (uv.x as u32 + px * uv.w as u32 / 10).min(aw.saturating_sub(1));
                            let sy = (uv.y as u32 + py * uv.h as u32 / row_h).min(atlas.atlas_height.saturating_sub(1));
                            let src = ((sy * aw + sx) * 4 + 3) as usize;
                            let alpha = atlas.atlas_pixels.get(src).copied().unwrap_or(0);
                            let xi = x0 + px;
                            let yi = y0 + py;
                            if xi < w && yi < h {
                                let o = ((yi * w + xi) * 4) as usize;
                                dst[o] = blend(fr, br, alpha);
                                dst[o+1] = blend(fg, bg, alpha);
                                dst[o+2] = blend(fb, bb, alpha);
                                dst[o+3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }
    slint::Image::from_rgba8(buffer)
}

fn blend(fg: u8, bg: u8, alpha: u8) -> u8 {
    let a = alpha as u16;
    (((fg as u16 * a) + (bg as u16 * (255 - a))) / 255) as u8
}
