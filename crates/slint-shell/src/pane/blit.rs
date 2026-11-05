//! Grid → RGBA rasterization and the content signatures that gate it.
//!
//! The editor pane blits one atlas cell per grid cell; the terminal blit
//! paints styled column runs.  `surface_signature` fingerprints the grid so
//! callers can skip the blit when nothing visible changed — it must hash
//! every style bit the blit renders, or attribute-only updates never
//! repaint.

use kernel::render::gpu_atlas::{GlyphUv, GpuGlyphAtlas};
use kernel::render::surface::Surface;

use super::{cell_size, MINIMAP_W};
use crate::theme::{Rgb, TerminalPalette};

/// Coverage of one on-screen pixel from the glyph's atlas cell.  A cell
/// rasterized at the screen's exact pixel size is a straight texel copy; a
/// differently sized atlas cell is box-filtered down.
fn glyph_alpha(
    atlas: &GpuGlyphAtlas,
    uv: &GlyphUv,
    px: u32,
    py: u32,
    cell_w: u32,
    cell_h: u32,
) -> u8 {
    let aw = atlas.atlas_width;
    if uv.w == cell_w && uv.h == cell_h {
        let src = (((uv.y + py) * aw + uv.x + px) * 4 + 3) as usize;
        if src < atlas.atlas_pixels.len() {
            // SAFETY: src is within bounds (checked above).
            return unsafe { *atlas.atlas_pixels.get_unchecked(src) };
        }
        return 0;
    }
    let sx0 = uv.x as f32 + px as f32 * uv.w as f32 / cell_w as f32;
    let sx1 = uv.x as f32 + (px + 1) as f32 * uv.w as f32 / cell_w as f32;
    let sy0 = uv.y as f32 + py as f32 * uv.h as f32 / cell_h as f32;
    let sy1 = uv.y as f32 + (py + 1) as f32 * uv.h as f32 / cell_h as f32;
    let mut sum: u32 = 0;
    let mut count: u32 = 0;
    let x_start = sx0.floor().max(0.0) as u32;
    let x_end = sx1.ceil().min(atlas.atlas_width as f32) as u32;
    let y_start = sy0.floor().max(0.0) as u32;
    let y_end = sy1.ceil().min(atlas.atlas_height as f32) as u32;
    for sy in y_start..y_end {
        for sx in x_start..x_end {
            let src = ((sy * aw + sx) * 4 + 3) as usize;
            if src < atlas.atlas_pixels.len() {
                // SAFETY: src is within bounds (checked above).
                sum += unsafe { *atlas.atlas_pixels.get_unchecked(src) } as u32;
                count += 1;
            }
        }
    }
    if count > 0 { (sum / count) as u8 } else { 0 }
}

pub fn blit_rgba(
    surface: &Surface,
    atlas: &GpuGlyphAtlas,
    cell_w: u32,
    cell_h: u32,
    scale: f32,
) -> (Vec<u8>, u32, u32) {
    let (cell_w, cell_h) = cell_size(cell_w, cell_h, scale);
    let w = surface.width as u32 * cell_w;
    let h = surface.height as u32 * cell_h;
    let mut dst = vec![0u8; (w * h * 4) as usize];
    let aw = atlas.atlas_width;

    let cells = surface.cells();
    let surf_w = surface.width as usize;
    for cy in 0..surface.height {
        for cx in 0..surface.width {
            // SAFETY: (cx, cy) is within surface dimensions.
            let cell = unsafe { cells.get_unchecked((cy as usize) * surf_w + (cx as usize)) };
            let (br, bg, bb) = cell.style.bg;
            let (mut fr, mut fg, mut fb) = cell.style.fg;
            if cell.style.bold {
                fr = fr.saturating_add(24);
                fg = fg.saturating_add(24);
                fb = fb.saturating_add(24);
            }
            if cell.style.dim {
                fr /= 2;
                fg /= 2;
                fb /= 2;
            }
            // Control chars are row terminators, not visible glyphs; painting
            // them through the box fallback would leave a marker at every
            // line end.  Missing visible glyphs (CJK) still get the box.
            let glyph = if cell.ch == ' ' || cell.ch.is_control() {
                None
            } else {
                atlas.uv_for(cell.ch, cell.style.italic)
            };

            let x0 = cx as u32 * cell_w;
            for py in 0..cell_h {
                let row = (cy as u32 * cell_h + py) * w + x0;
                let out = &mut dst[(row * 4) as usize..((row + cell_w) * 4) as usize];
                match glyph {
                    None => {
                        for o in out.chunks_exact_mut(4) {
                            o[0] = br;
                            o[1] = bg;
                            o[2] = bb;
                            o[3] = 255;
                        }
                    }
                    Some(uv) if uv.w == cell_w && uv.h == cell_h => {
                        // Atlas cell == screen cell: a direct texel row copy.
                        let src_row = (((uv.y + py) * aw + uv.x) * 4 + 3) as usize;
                        // SAFETY: out has cell_w*4 bytes and the atlas range is in bounds.
                        unsafe {
                            let mut src_ptr = atlas.atlas_pixels.as_ptr().add(src_row);
                            let mut dst_ptr = out.as_mut_ptr();
                            for _ in 0..cell_w {
                                let a = *src_ptr;
                                if a == 0 {
                                    *dst_ptr = br;
                                    *dst_ptr.add(1) = bg;
                                    *dst_ptr.add(2) = bb;
                                    *dst_ptr.add(3) = 255;
                                } else if a == 255 {
                                    *dst_ptr = fr;
                                    *dst_ptr.add(1) = fg;
                                    *dst_ptr.add(2) = fb;
                                    *dst_ptr.add(3) = 255;
                                } else {
                                    *dst_ptr = blend(fr, br, a);
                                    *dst_ptr.add(1) = blend(fg, bg, a);
                                    *dst_ptr.add(2) = blend(fb, bb, a);
                                    *dst_ptr.add(3) = 255;
                                }
                                src_ptr = src_ptr.add(4);
                                dst_ptr = dst_ptr.add(4);
                            }
                        }
                    }
                    Some(uv) => {
                        // A cell rasterized at a different size than the
                        // screen's is box-filtered down to the screen grid.
                        // Slant is not applied here: an italic cell is
                        // already sheared in the atlas.
                        for (px, o) in out.chunks_exact_mut(4).enumerate() {
                            let alpha = glyph_alpha(atlas, &uv, px as u32, py, cell_w, cell_h);
                            o[0] = blend(fr, br, alpha);
                            o[1] = blend(fg, bg, alpha);
                            o[2] = blend(fb, bb, alpha);
                            o[3] = 255;
                        }
                    }
                }
                // Line attributes draw over the glyph ink: underline on the
                // cell's bottom row, strike through its middle.
                if (cell.style.underline && py + 1 == cell_h)
                    || (cell.style.strikethrough && py == cell_h / 2)
                {
                    for o in out.chunks_exact_mut(4) {
                        o[0] = fr;
                        o[1] = fg;
                        o[2] = fb;
                        o[3] = 255;
                    }
                }
            }
        }
    }
    (dst, w, h)
}

/// Scale an image buffer's pixels into a target RGBA buffer of size
/// (target_w, target_h), preserving aspect ratio and centering on `bg`.
pub fn blit_image(
    img: &kernel::text_engine::ImageData,
    target_w: u32,
    target_h: u32,
    bg: (u8, u8, u8),
) -> (Vec<u8>, u32, u32) {
    let tw = target_w.max(1);
    let th = target_h.max(1);
    let mut dst = vec![0u8; (tw * th * 4) as usize];
    // fill bg
    for i in (0..dst.len()).step_by(4) {
        dst[i] = bg.0;
        dst[i + 1] = bg.1;
        dst[i + 2] = bg.2;
        dst[i + 3] = 255;
    }
    if img.width == 0 || img.height == 0 {
        return (dst, tw, th);
    }
    // fit
    let scale = (tw as f32 / img.width as f32).min(th as f32 / img.height as f32);
    let dw = ((img.width as f32 * scale).round().max(1.0)) as u32;
    let dh = ((img.height as f32 * scale).round().max(1.0)) as u32;
    let ox = (tw.saturating_sub(dw)) / 2;
    let oy = (th.saturating_sub(dh)) / 2;
    for dy in 0..dh {
        for dx in 0..dw {
            let sx = (dx as f32 / scale).floor().min((img.width - 1) as f32) as u32;
            let sy = (dy as f32 / scale).floor().min((img.height - 1) as f32) as u32;
            let si = ((sy * img.width + sx) * 4) as usize;
            if si + 3 < img.pixels.len() {
                let di = (((oy + dy) * tw + (ox + dx)) * 4) as usize;
                dst[di] = img.pixels[si];
                dst[di + 1] = img.pixels[si + 1];
                dst[di + 2] = img.pixels[si + 2];
                dst[di + 3] = img.pixels[si + 3];
            }
        }
    }
    (dst, tw, th)
}

/// Cheap content fingerprint for the cell grid.  Frontends use it to skip the
/// pixel blit and texture upload when only chrome changed.
pub fn surface_signature(surface: &Surface) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut feed = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    let cells = surface.cells();
    let surf_w = surface.width as usize;
    for y in 0..surface.height {
        for x in 0..surface.width {
            // SAFETY: (x, y) is within surface dimensions.
            let c = unsafe { cells.get_unchecked((y as usize) * surf_w + (x as usize)) };
            feed(c.ch as u64);
            // Every style bit the blit renders must reach the hash, or an
            // attribute-only change (dim/italic/underline/strike) reuses a
            // stale frame.
            feed(
                u32::from_be_bytes([
                    c.style.fg.0,
                    c.style.fg.1,
                    c.style.fg.2,
                    c.style.bold as u8
                        | (c.style.italic as u8) << 1
                        | (c.style.underline as u8) << 2
                        | (c.style.strikethrough as u8) << 3
                        | (c.style.dim as u8) << 4,
                ]) as u64,
            );
            feed(u32::from_be_bytes([c.style.bg.0, c.style.bg.1, c.style.bg.2, 0]) as u64);
        }
    }
    h
}

pub fn blit_minimap(
    ed: &kernel::state::Editor,
    buf_id: usize,
    width: u32,
    height: u32,
    scroll_top: usize,
    visible_rows: usize,
) -> slint::Image {
    let mut buffer =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width.max(1), height.max(1));
    let dst = buffer.make_mut_bytes();
    let w = width.max(1);
    let h = height.max(1);

    // background crust
    let (cr, cg, cb) = ed.theme_color("bg");
    for i in (0..dst.len()).step_by(4) {
        dst[i] = cr;
        dst[i + 1] = cg;
        dst[i + 2] = cb;
        dst[i + 3] = 255;
    }

    let arc = match ed.buffers.get(buf_id) {
        Some(a) => a,
        None => return slint::Image::from_rgba8(buffer),
    };
    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let total = buf.line_count();
    if total == 0 {
        return slint::Image::from_rgba8(buffer);
    }

    let max_rows = ((h as f32 / 1.5).floor() as usize).max(1);
    let step = if total <= max_rows {
        1
    } else {
        total.div_ceil(max_rows)
    };
    let row_h = (h as f32 / total as f32).clamp(1.5, 3.0);

    // collect highlights
    let mut hl: Vec<(usize, usize, String)> = Vec::new();
    if let Some(v) = ed.view_of_buffer(buf_id) {
        for (s, e, f) in &v.highlights {
            hl.push((*s, *e, f.clone()));
        }
        for layer in v.highlight_layers.values() {
            for (s, e, f) in layer {
                hl.push((*s, *e, f.clone()));
            }
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
                let bar_px =
                    (((chars.min(64) as f32 / 64.0) * (MINIMAP_W as f32 - 8.0)).max(1.0)) as u32;
                let y = (row as f32 * row_h) as u32;
                let bar_h = (row_h * 0.66).max(1.0) as u32;

                // runs
                let mut runs: Vec<(usize, (u8, u8, u8))> = Vec::new();
                for (i, _) in trimmed.char_indices() {
                    let byte = span.start + i;
                    let face = hl
                        .iter()
                        .rev()
                        .find(|(s, e, _)| *s <= byte && byte < *e)
                        .map(|(_, _, f)| f.as_str());
                    let color = face
                        .and_then(|f| ed.faces.get(f))
                        .map(|st| st.fg)
                        .unwrap_or(base_fg);
                    let col = trimmed[..i].chars().count();
                    match runs.last_mut() {
                        Some((_, c)) if *c == color => {}
                        _ => runs.push((col, color)),
                    }
                }
                let mut prev_col = 0usize;
                for (i, (col, color)) in runs.iter().enumerate() {
                    let next_col = runs.get(i + 1).map(|(c, _)| *c).unwrap_or(chars);
                    let run_w = (((next_col - prev_col) as f32 / chars as f32) * bar_px as f32)
                        .max(1.0) as u32;
                    if run_w >= 1 {
                        let x0 = 4
                            + indent_px as u32
                            + ((prev_col as f32 / chars as f32) * bar_px as f32) as u32;
                        for py in 0..bar_h {
                            for px in 0..run_w {
                                let xi = x0 + px;
                                let yi = y + py;
                                if xi < w && yi < h {
                                    let o = ((yi * w + xi) * 4) as usize;
                                    dst[o] = color.0;
                                    dst[o + 1] = color.1;
                                    dst[o + 2] = color.2;
                                    dst[o + 3] = 255;
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
                    let (r, g, b) = ed.theme_color("fg");
                    dst[o] = r;
                    dst[o + 1] = g;
                    dst[o + 2] = b;
                    dst[o + 3] = 255;
                }
            }
        }
    }

    slint::Image::from_rgba8(buffer)
}

pub fn blit_terminal_rgba(
    lines: &[kernel::terminal::screen::StyledLine],
    atlas: &GpuGlyphAtlas,
    cols: u32,
    rows: u32,
    cell_w: u32,
    cell_h: u32,
    palette: &TerminalPalette,
    cursor: Option<(usize, usize)>,
) -> (Vec<u8>, u32, u32) {
    let (cell_w, cell_h) = (cell_w.max(1), cell_h.max(1));
    let w = cols.max(1) * cell_w;
    let h = rows.max(1) * cell_h;
    let mut dst = vec![0u8; (w * h * 4) as usize];

    // SAFETY: dst has (w*h*4) bytes.
    unsafe {
        let mut p = dst.as_mut_ptr();
        let end = p.add(dst.len());
        let (r, g, b) = (palette.bg.0, palette.bg.1, palette.bg.2);
        while p < end {
            *p = r;
            *p.add(1) = g;
            *p.add(2) = b;
            *p.add(3) = 255;
            p = p.add(4);
        }
    }

    let start = lines.len().saturating_sub(rows as usize);
    for (ri, line) in lines[start..].iter().enumerate() {
        let y0 = ri as u32 * cell_h;

        // Run backgrounds first so the cursor block can sit above them.
        for span in &line.spans {
            let (br, bg, bb) = term_color(span.style.bg, palette, palette.bg);
            if (br, bg, bb) == palette.bg {
                continue;
            }
            let span_x = span.start as u32 * cell_w;
            let span_w = (span.len as u32 * cell_w).min(w.saturating_sub(span_x));
            for py in 0..cell_h {
                let yi = y0 + py;
                if yi >= h {
                    break;
                }
                let row = yi * w;
                for px in 0..span_w {
                    let o = ((row + span_x + px) * 4) as usize;
                    dst[o] = br;
                    dst[o + 1] = bg;
                    dst[o + 2] = bb;
                    dst[o + 3] = 255;
                }
            }
        }

        if let Some((cr, cc)) = cursor
            && cr == ri
            && cc < cols as usize
        {
            let x0 = cc as u32 * cell_w;
            for py in 0..cell_h {
                let yi = y0 + py;
                if yi >= h {
                    break;
                }
                let row = yi * w;
                for px in 0..cell_w {
                    let o = ((row + x0 + px) * 4) as usize;
                    dst[o] = palette.cursor.0;
                    dst[o + 1] = palette.cursor.1;
                    dst[o + 2] = palette.cursor.2;
                    dst[o + 3] = 255;
                }
            }
        }

        // Styled spans are column runs; the text has one char per cell.
        for span in &line.spans {
            let style = span.style;
            let (mut fr, mut fg, mut fb) = term_color(style.fg, palette, palette.fg);
            if style.bold {
                fr = fr.saturating_add(24);
                fg = fg.saturating_add(24);
                fb = fb.saturating_add(24);
            }
            if style.dim {
                fr /= 2;
                fg /= 2;
                fb /= 2;
            }

            let span_x = span.start as u32 * cell_w;
            let (br, bg, bb) = term_color(style.bg, palette, palette.bg);
            for (ci, ch) in line
                .text
                .chars()
                .skip(span.start as usize)
                .take(span.len as usize)
                .enumerate()
            {
                if ch == ' ' || ch.is_control() {
                    continue;
                }
                let Some(uv) = atlas.uv_for(ch, style.italic) else {
                    continue;
                };
                let col = span.start as usize + ci;
                let on_cursor = cursor == Some((ri, col));
                let (fr, fg, fb) = if on_cursor { palette.bg } else { (fr, fg, fb) };
                let (br, bg, bb) = if on_cursor {
                    palette.cursor
                } else {
                    (br, bg, bb)
                };
                let x0 = span_x + ci as u32 * cell_w;
                if x0 + cell_w > w {
                    break;
                }
                let direct = uv.w == cell_w && uv.h == cell_h;
                for py in 0..cell_h {
                    let yi = y0 + py;
                    if yi >= h {
                        break;
                    }
                    let row = yi * w + x0;
                    let out = &mut dst[(row * 4) as usize..((row + cell_w) * 4) as usize];
                    if direct {
                        let src_row = (((uv.y + py) * atlas.atlas_width + uv.x) * 4 + 3) as usize;
                        // SAFETY: out has cell_w*4 bytes and atlas range is in bounds.
                        unsafe {
                            let mut src_ptr = atlas.atlas_pixels.as_ptr().add(src_row);
                            let mut dst_ptr = out.as_mut_ptr();
                            for _ in 0..cell_w {
                                let alpha = *src_ptr;
                                if alpha != 0 {
                                    if alpha == 255 {
                                        *dst_ptr = fr;
                                        *dst_ptr.add(1) = fg;
                                        *dst_ptr.add(2) = fb;
                                        *dst_ptr.add(3) = 255;
                                    } else {
                                        *dst_ptr = blend(fr, br, alpha);
                                        *dst_ptr.add(1) = blend(fg, bg, alpha);
                                        *dst_ptr.add(2) = blend(fb, bb, alpha);
                                        *dst_ptr.add(3) = 255;
                                    }
                                }
                                src_ptr = src_ptr.add(4);
                                dst_ptr = dst_ptr.add(4);
                            }
                        }
                    } else {
                        // SAFETY: out has cell_w*4 bytes.
                        unsafe {
                            let mut dst_ptr = out.as_mut_ptr();
                            for px in 0..cell_w {
                                let alpha = glyph_alpha(atlas, &uv, px, py, cell_w, cell_h);
                                if alpha != 0 {
                                    *dst_ptr = blend(fr, br, alpha);
                                    *dst_ptr.add(1) = blend(fg, bg, alpha);
                                    *dst_ptr.add(2) = blend(fb, bb, alpha);
                                    *dst_ptr.add(3) = 255;
                                }
                                dst_ptr = dst_ptr.add(4);
                            }
                        }
                    }
                }
            }
        }
    }
    (dst, w, h)
}

/// Resolve a terminal color against the active theme's console palette.
fn term_color(c: kernel::terminal::Color, palette: &TerminalPalette, default: Rgb) -> Rgb {
    use kernel::terminal::Color;
    match c {
        Color::Default => default,
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) => palette.ansi[(i as usize).min(15)],
    }
}

/// `fg * a/255 + bg * (1 - a/255)` without an integer division in the
/// per-pixel loop.  `x <= 255 * 255`, so the 16.16 multiply-and-round is
/// exact for this range.
#[inline(always)]
fn blend(fg: u8, bg: u8, alpha: u8) -> u8 {
    let a = alpha as u32;
    let x = fg as u32 * a + bg as u32 * (255 - a);
    ((x * 257 + 32768) >> 16) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::render::surface::Style;

    const FONT: &[u8] = include_bytes!("../../../../assets/fonts/JetBrainsMono-Regular.ttf");

    fn meas() -> (u32, u32) {
        super::super::measure_cell(FONT, 15.0)
    }

    fn atlas_with_font() -> GpuGlyphAtlas {
        let mut atlas = GpuGlyphAtlas::new();
        let (cw, ch) = meas();
        atlas.rasterize_for_grid(FONT, 15.0, 1.0, cw, ch);
        atlas
    }

    #[test]
    fn hidpi_blit_doubles_the_output_grid() {
        let surface = Surface::new(3, 2);
        let atlas = GpuGlyphAtlas::new();
        let (rgba, w, h) = blit_rgba(&surface, &atlas, 7, 17, 2.0);
        assert_eq!((w, h), (3 * 7 * 2, 2 * 17 * 2));
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }

    #[test]
    fn control_cells_paint_only_their_background() {
        let mut surface = Surface::new(1, 1);
        let bg = (12u8, 34, 56);
        surface.set_cell(
            0,
            0,
            '\n',
            Some(Style {
                bg,
                fg: (255, 255, 255),
                ..Style::default()
            }),
        );

        let atlas = GpuGlyphAtlas::new();
        let (rgba, w, h) = blit_rgba(&surface, &atlas, 7, 17, 1.0);
        assert_eq!((w, h), (7, 17));
        for px in rgba.chunks_exact(4) {
            assert_eq!(
                &px[..3],
                &[bg.0, bg.1, bg.2],
                "a newline must not paint a glyph"
            );
        }
    }

    #[test]
    fn visible_cells_paint_their_foreground() {
        let mut surface = Surface::new(1, 1);
        let fg = (255u8, 0, 0);
        surface.set_cell(
            0,
            0,
            'A',
            Some(Style {
                bg: (0, 0, 0),
                fg,
                ..Style::default()
            }),
        );

        let atlas = GpuGlyphAtlas::new();
        let (rgba, _, _) = blit_rgba(&surface, &atlas, 7, 17, 1.0);
        assert_eq!(&rgba[..3], &[fg.0, fg.1, fg.2]);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn signature_distinguishes_every_style_bit() {
        // An attribute-only change (comment goes dim, code gets underlined,
        // dead code is struck) must invalidate the signature even when glyph
        // and colors are identical.
        let style = |flags: fn(&mut Style)| {
            let mut s = Style {
                fg: (200, 200, 200),
                bg: (0, 0, 0),
                ..Style::default()
            };
            flags(&mut s);
            s
        };
        let sig = |s: Style| {
            let mut surface = Surface::new(2, 1);
            surface.set_cell(0, 0, 'x', Some(s.clone()));
            surface.set_cell(1, 0, 'y', Some(s));
            surface_signature(&surface)
        };
        let base = sig(style(|_| {}));
        assert_ne!(base, sig(style(|s| s.dim = true)));
        assert_ne!(base, sig(style(|s| s.italic = true)));
        assert_ne!(base, sig(style(|s| s.underline = true)));
        assert_ne!(base, sig(style(|s| s.strikethrough = true)));
        assert_ne!(base, sig(style(|s| s.bold = true)));
    }

    #[test]
    fn dim_halves_the_foreground_on_glyph_ink() {
        let (cw, ch) = meas();
        let fg = (200u8, 210, 220);
        let mut surface = Surface::new(1, 1);
        surface.set_cell(
            0,
            0,
            'M',
            Some(Style {
                bg: (0, 0, 0),
                fg,
                dim: true,
                ..Style::default()
            }),
        );
        let (plain_rgba, _, _) = {
            let mut surface = Surface::new(1, 1);
            surface.set_cell(
                0,
                0,
                'M',
                Some(Style {
                    bg: (0, 0, 0),
                    fg,
                    ..Style::default()
                }),
            );
            blit_rgba(&surface, &atlas_with_font(), cw, ch, 1.0)
        };
        let (rgba, _, _) = blit_rgba(&surface, &atlas_with_font(), cw, ch, 1.0);
        // At this size every glyph pixel is antialiased, so the contract is
        // strict darkening: dim ink never brightens any pixel and is
        // strictly darker somewhere.
        let mut darker = false;
        for (dim_px, plain_px) in rgba.chunks_exact(4).zip(plain_rgba.chunks_exact(4)) {
            for c in 0..3 {
                assert!(
                    dim_px[c] <= plain_px[c],
                    "dim must not brighten ink: {dim_px:?} vs {plain_px:?}"
                );
            }
            darker |= dim_px[..3] < plain_px[..3];
        }
        assert!(darker, "dim must darken the glyph somewhere");
    }

    #[test]
    fn italic_cells_reach_the_pane_texel_for_texel() {
        // Italic reads the atlas's oblique cell; the blit does not slide a
        // rasterized upright cell sideways.
        let (_, ch) = meas();
        let atlas = atlas_with_font();
        for glyph in ['m', 'W', 'l'] {
            let (rgba, w) = cell_pixels(glyph, true);
            let uv = atlas.uv_for(glyph, true).expect("oblique cell is baked");
            let baked = (0..ch)
                .flat_map(|py| (0..w).map(move |px| (px, py)))
                .filter(|(px, py)| {
                    atlas.atlas_pixels
                        [(((uv.y + py) * atlas.atlas_width + uv.x + px) * 4 + 3) as usize]
                        > 0
                })
                .count();
            let painted = (0..ch * w).filter(|i| rgba[(*i * 4) as usize] > 0).count();
            assert_eq!(
                painted, baked,
                "{glyph:?}: every texel of the oblique cell must land in the pane"
            );
        }
    }

    #[test]
    fn italic_leans_a_wide_glyph_as_far_as_a_stem() {
        // The bug this pins: the shear was capped by each row's own blank
        // margin, so a stem leaned fully while the rows carrying a letter's
        // wide parts stayed put — letters tore apart at the seam between
        // strokes, and a full-width glyph did not lean at all.
        let lean = |glyph: char| {
            let (roman, w) = cell_pixels(glyph, false);
            let (italic, _) = cell_pixels(glyph, true);
            (ink_centre(&italic, w, 3, 6) - ink_centre(&roman, w, 3, 6))
                - (ink_centre(&italic, w, 9, 12) - ink_centre(&roman, w, 9, 12))
        };
        let stem = lean('l');
        assert!(stem > 0.5, "italic must lean forward: {stem}");
        for glyph in ['i', 'n', 'a', 'o', 'm', 'W', 'E'] {
            let slanted = lean(glyph);
            assert!(slanted > 0.5, "{glyph:?}: italic must lean forward");
            assert!(
                (slanted - stem).abs() < 0.45,
                "{glyph:?} leans {slanted} where a stem leans {stem}: \
                 a glyph's own ink must not set its slant"
            );
        }
    }

    #[test]
    fn terminal_italic_cells_use_the_oblique_block() {
        use kernel::terminal::screen::{Span, Style as TermStyle, StyledLine};
        let (cw, ch) = meas();
        let atlas = atlas_with_font();
        let palette = crate::theme::palette("magma-dark").terminal;
        let line = |italic: bool| {
            vec![StyledLine {
                text: "l".into(),
                spans: vec![Span {
                    start: 0,
                    len: 1,
                    style: TermStyle {
                        italic,
                        ..Default::default()
                    },
                }],
            }]
        };
        let ink = |rgba: &[u8]| {
            rgba.chunks_exact(4)
                .filter(|px| px[..3] != [palette.bg.0, palette.bg.1, palette.bg.2])
                .count()
        };
        let (roman, _, _) = blit_terminal_rgba(&line(false), &atlas, 1, 1, cw, ch, &palette, None);
        let (italic, _, _) = blit_terminal_rgba(&line(true), &atlas, 1, 1, cw, ch, &palette, None);

        let uv = atlas.uv_for('l', true).expect("oblique cell is baked");
        let baked = (0..ch)
            .flat_map(|py| (0..cw).map(move |px| (px, py)))
            .filter(|(px, py)| {
                atlas.atlas_pixels[(((uv.y + py) * atlas.atlas_width + uv.x + px) * 4 + 3) as usize]
                    > 0
            })
            .count();
        assert_eq!(ink(&italic), baked, "the oblique cell's texels are the ink");
        assert_ne!(italic, roman, "SGR italic must move the ink");
    }

    #[test]
    #[ignore]
    fn zz_probe_running_app_geometry() {
        let bytes = std::env::home_dir()
            .map(|h| h.join("Library/Fonts/MonacoNerdFontMono-Regular.ttf"))
            .and_then(|p| std::fs::read(p).ok())
            .or_else(|| std::fs::read("/Library/Fonts/Monaco.ttf").ok())
            .or_else(|| std::fs::read("/System/Library/Fonts/Monaco.ttf").ok())
            .unwrap_or_else(|| FONT.to_vec());
        let size = 20.0_f32;
        let ppx = 2.0_f32;
        let (cw, ch) = crate::pane::measure_cell(&bytes, size);
        println!("cell logical: {cw}x{ch}  device: {:?}", super::super::cell_size(cw, ch, ppx));
        let mut atlas = GpuGlyphAtlas::new();
        atlas.rasterize_for_grid(&bytes, size, ppx, cw, ch);
        println!("atlas cell: {}x{}", atlas.cell_w, atlas.cell_h);
        for glyph in ['!', 'E', 'l', 'a', 'M', ';'] {
            for italic in [false, true] {
                let mut surface = Surface::new(1, 1);
                surface.set_cell(
                    0,
                    0,
                    glyph,
                    Some(Style {
                        fg: (255, 255, 255),
                        bg: (0, 0, 0),
                        italic,
                        dim: italic,
                        ..Default::default()
                    }),
                );
                let (rgba, w, h) = blit_rgba(&surface, &atlas, cw, ch, ppx);
                println!("--- {glyph:?} italic={italic} (w={w} h={h}) ---");
                for py in 0..h {
                    let mut row = String::new();
                    for px in 0..w {
                        let o = ((py * w + px) * 4) as usize;
                        if rgba[o] > 120 {
                            row.push('#');
                        } else if rgba[o] > 40 {
                            row.push('+');
                        } else {
                            row.push(' ');
                        }
                    }
                    println!("{row}");
                }
            }
        }
    }

    /// Blit one cell holding `glyph`, returning its pixels and width.
    fn cell_pixels(glyph: char, italic: bool) -> (Vec<u8>, u32) {
        let (cw, ch) = meas();
        let mut surface = Surface::new(1, 1);
        surface.set_cell(
            0,
            0,
            glyph,
            Some(Style {
                fg: (255, 255, 255),
                bg: (0, 0, 0),
                italic,
                ..Default::default()
            }),
        );
        let (rgba, w, _) = blit_rgba(&surface, &atlas_with_font(), cw, ch, 1.0);
        (rgba, w)
    }

    /// Coverage-weighted horizontal centre of a cell's ink over rows
    /// `[from, to)`, sub-pixel so two glyphs can be compared.
    fn ink_centre(rgba: &[u8], w: u32, from: u32, to: u32) -> f32 {
        let (mut sum, mut mass) = (0.0f32, 0.0f32);
        for py in from..to {
            for px in 0..w {
                let a = rgba[((py * w + px) * 4) as usize] as f32 / 255.0;
                sum += px as f32 * a;
                mass += a;
            }
        }
        sum / mass
    }





    #[test]
    fn underline_paints_the_bottom_row_in_fg() {
        let fg = (255u8, 255, 0);
        let mut surface = Surface::new(1, 1);
        surface.set_cell(
            0,
            0,
            ' ',
            Some(Style {
                bg: (0, 0, 0),
                fg,
                underline: true,
                ..Style::default()
            }),
        );
        let atlas = atlas_with_font();
        let (rgba, w, h) = blit_rgba(&surface, &atlas, 7, 17, 1.0);
        let bottom = ((h - 1) * w) as usize * 4;
        assert_eq!(
            &rgba[bottom..bottom + 3],
            &[fg.0, fg.1, fg.2],
            "the underline row is full fg"
        );
        let mid = ((h / 2) * w) as usize * 4;
        assert_eq!(&rgba[mid..mid + 3], &[0, 0, 0], "no strike row was asked for");
    }

    #[test]
    fn strikethrough_paints_the_middle_row_in_fg() {
        let fg = (0u8, 255, 255);
        let mut surface = Surface::new(1, 1);
        surface.set_cell(
            0,
            0,
            ' ',
            Some(Style {
                bg: (0, 0, 0),
                fg,
                strikethrough: true,
                ..Style::default()
            }),
        );
        let atlas = atlas_with_font();
        let (rgba, w, h) = blit_rgba(&surface, &atlas, 7, 17, 1.0);
        let mid = ((h / 2) * w) as usize * 4;
        assert_eq!(
            &rgba[mid..mid + 3],
            &[fg.0, fg.1, fg.2],
            "the strike row is full fg"
        );
        let bottom = ((h - 1) * w) as usize * 4;
        assert_eq!(&rgba[bottom..bottom + 3], &[0, 0, 0], "no underline was asked for");
    }

    #[test]
    fn terminal_grid_reserves_the_jetbrains_tab_strip() {
        assert_eq!(super::super::terminal_grid_size(800.0, 0, 7, 17), (114, 10));
        assert_eq!(super::super::terminal_grid_size(800.0, 1, 7, 17), (114, 8));
    }

    #[test]
    fn terminal_background_comes_from_the_theme() {
        let atlas = GpuGlyphAtlas::new();
        let palette = crate::theme::palette("magma-dark").terminal;
        let lines = vec![kernel::terminal::screen::StyledLine::default()];
        let (rgba, _, _) = blit_terminal_rgba(&lines, &atlas, 1, 1, 8, 16, &palette, None);
        assert_eq!(&rgba[..3], &[palette.bg.0, palette.bg.1, palette.bg.2]);
    }

    #[test]
    fn terminal_ansi_slots_come_from_the_theme() {
        let atlas = GpuGlyphAtlas::new();
        let palette = crate::theme::palette("magma-dark").terminal;
        let style = kernel::terminal::screen::Style {
            bg: kernel::terminal::Color::Indexed(1),
            ..Default::default()
        };
        let lines = vec![kernel::terminal::screen::StyledLine {
            text: " ".into(),
            spans: vec![kernel::terminal::screen::Span {
                start: 0,
                len: 1,
                style,
            }],
        }];
        let (rgba, _, _) = blit_terminal_rgba(&lines, &atlas, 1, 1, 8, 16, &palette, None);
        let ansi = palette.ansi[1];
        assert_eq!(&rgba[..3], &[ansi.0, ansi.1, ansi.2]);
    }

    #[test]
    fn terminal_cursor_uses_the_theme_cursor_color() {
        let atlas = GpuGlyphAtlas::new();
        let palette = crate::theme::palette("magma-dark").terminal;
        let lines = vec![kernel::terminal::screen::StyledLine::default()];
        let (rgba, _, _) = blit_terminal_rgba(&lines, &atlas, 1, 1, 8, 16, &palette, Some((0, 0)));
        assert_eq!(
            &rgba[..3],
            &[palette.cursor.0, palette.cursor.1, palette.cursor.2]
        );
    }
}

