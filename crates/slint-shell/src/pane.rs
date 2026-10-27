//! Surface → Slint image: the editor pane's presentation path.
//!
//! The kernel renders editor semantics into a character grid.  This module
//! turns that grid into RGBA pixels using the glyph atlas, then hands them to
//! Slint as an `Image`.  No widget toolkit types reach the kernel.

use kernel::render::gpu_atlas::{GlyphUv, GpuGlyphAtlas};
use kernel::render::surface::Surface;

use crate::theme::{Rgb, TerminalPalette};

pub const CHAR_W: u32 = 7;
pub const LINE_H: u32 = 17;

/// Terminal dock inner height: the dock is 200px, its header 26px.
pub const TERMINAL_DOCK_BODY_H: f32 = 174.0;
pub const TERMINAL_TABS_H: f32 = 26.0;

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

/// Coverage of one on-screen pixel from the glyph's atlas cell.  A cell
/// rasterized at the screen's exact pixel size is a straight texel copy; a
/// differently sized atlas cell is box-filtered down.
fn glyph_alpha(atlas: &GpuGlyphAtlas, uv: &GlyphUv, px: u32, py: u32, cell_w: u32, cell_h: u32) -> u8 {
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

pub fn blit_rgba(surface: &Surface, atlas: &GpuGlyphAtlas, cell_w: u32, cell_h: u32, scale: f32) -> (Vec<u8>, u32, u32) {
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
            // Control chars are row terminators, not visible glyphs; painting
            // them through the box fallback would leave a marker at every
            // line end.  Missing visible glyphs (CJK, icons) still get the box.
            let glyph = if cell.ch == ' ' || cell.ch.is_control() {
                None
            } else {
                atlas.glyphs.get(&cell.ch)
                    .or_else(|| atlas.glyphs.get(&'\u{25A1}'))
            };

            let x0 = cx as u32 * cell_w;
            for py in 0..cell_h {
                let row = (cy as u32 * cell_h + py) * w + x0;
                let out = &mut dst[(row * 4) as usize..((row + cell_w) * 4) as usize];
                let Some(uv) = glyph else {
                    // SAFETY: out has cell_w*4 bytes.
                    unsafe {
                        let mut p = out.as_mut_ptr();
                        let end = p.add(out.len());
                        while p < end {
                            *p = br; *p.add(1) = bg; *p.add(2) = bb; *p.add(3) = 255;
                            p = p.add(4);
                        }
                    }
                    continue;
                };

                if uv.w == cell_w && uv.h == cell_h {
                    // Atlas cell == screen cell: a direct texel row copy.
                    let src_row = (((uv.y + py) * aw + uv.x) * 4 + 3) as usize;
                    // SAFETY: atlas cell and screen cell dimensions match, so the
                    // src and dst ranges are in bounds.
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
                } else {
                    for (px, o) in out.chunks_exact_mut(4).enumerate() {
                        let a = glyph_alpha(atlas, uv, px as u32, py, cell_w, cell_h);
                        // SAFETY: `o` is a 4-byte chunk from chunks_exact_mut(4).
                        unsafe {
                            *o.get_unchecked_mut(0) = blend(fr, br, a);
                            *o.get_unchecked_mut(1) = blend(fg, bg, a);
                            *o.get_unchecked_mut(2) = blend(fb, bb, a);
                            *o.get_unchecked_mut(3) = 255;
                        }
                    }
                }
            }
        }
    }
    (dst, w, h)
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
            feed(u32::from_be_bytes([c.style.fg.0, c.style.fg.1, c.style.fg.2, c.style.bold as u8]) as u64);
            feed(u32::from_be_bytes([c.style.bg.0, c.style.bg.1, c.style.bg.2, 0]) as u64);
        }
    }
    h
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

pub const MINIMAP_W: u32 = 90;

/// Resolve a terminal color against the active theme's console palette.
fn term_color(c: kernel::terminal::Color, palette: &TerminalPalette, default: Rgb) -> Rgb {
    use kernel::terminal::Color;
    match c {
        Color::Default => default,
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) => palette.ansi[(i as usize).min(15)],
    }
}

pub fn blit_minimap(
    ed: &kernel::state::Editor,
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
            *p = r; *p.add(1) = g; *p.add(2) = b; *p.add(3) = 255;
            p = p.add(4);
        }
    }

    let start = lines.len().saturating_sub(rows as usize);
    for (ri, line) in lines[start..].iter().enumerate() {
        let y0 = ri as u32 * cell_h;

        // Run backgrounds first so the cursor block can sit above them.
        for span in &line.spans {
            let (br, bg, bb) = term_color(span.style.bg, palette, palette.bg);
            if (br, bg, bb) == palette.bg { continue; }
            let span_x = span.start as u32 * cell_w;
            let span_w = (span.len as u32 * cell_w).min(w.saturating_sub(span_x));
            for py in 0..cell_h {
                let yi = y0 + py;
                if yi >= h { break; }
                let row = yi * w;
                for px in 0..span_w {
                    let o = ((row + span_x + px) * 4) as usize;
                    dst[o] = br; dst[o + 1] = bg; dst[o + 2] = bb; dst[o + 3] = 255;
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
                if yi >= h { break; }
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
            if style.bold { fr = fr.saturating_add(24); fg = fg.saturating_add(24); fb = fb.saturating_add(24); }
            if style.dim { fr /= 2; fg /= 2; fb /= 2; }

            let span_x = span.start as u32 * cell_w;
            let (br, bg, bb) = term_color(style.bg, palette, palette.bg);
            for (ci, ch) in line.text.chars()
                .skip(span.start as usize)
                .take(span.len as usize)
                .enumerate()
            {
                if ch == ' ' || ch.is_control() { continue; }
                let Some(uv) = atlas.glyphs.get(&ch)
                    .or_else(|| atlas.glyphs.get(&'\u{25A1}')) else { continue };
                let col = span.start as usize + ci;
                let on_cursor = cursor == Some((ri, col));
                let (fr, fg, fb) = if on_cursor { palette.bg } else { (fr, fg, fb) };
                let (br, bg, bb) = if on_cursor { palette.cursor } else { (br, bg, bb) };
                let x0 = span_x + ci as u32 * cell_w;
                if x0 + cell_w > w { break; }
                let direct = uv.w == cell_w && uv.h == cell_h;
                for py in 0..cell_h {
                    let yi = y0 + py;
                    if yi >= h { break; }
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
                                        *dst_ptr = fr; *dst_ptr.add(1) = fg; *dst_ptr.add(2) = fb; *dst_ptr.add(3) = 255;
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
                                let alpha = glyph_alpha(atlas, uv, px, py, cell_w, cell_h);
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

    #[test]
    fn hidpi_blit_doubles_the_output_grid() {
        let surface = Surface::new(3, 2);
        let atlas = GpuGlyphAtlas::new();
        let (rgba, w, h) = blit_rgba(&surface, &atlas, CHAR_W, LINE_H, 2.0);
        assert_eq!((w, h), (3 * CHAR_W * 2, 2 * LINE_H * 2));
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }

    #[test]
    fn control_cells_paint_only_their_background() {
        let mut surface = Surface::new(1, 1);
        let bg = (12u8, 34, 56);
        surface.set_cell(0, 0, '\n', Some(Style { bg, fg: (255, 255, 255), ..Style::default() }));

        let atlas = GpuGlyphAtlas::new();
        let (rgba, w, h) = blit_rgba(&surface, &atlas, CHAR_W, LINE_H, 1.0);
        assert_eq!((w, h), (CHAR_W, LINE_H));
        for px in rgba.chunks_exact(4) {
            assert_eq!(&px[..3], &[bg.0, bg.1, bg.2], "a newline must not paint a glyph");
        }
    }

    #[test]
    fn visible_cells_paint_their_foreground() {
        let mut surface = Surface::new(1, 1);
        let fg = (255u8, 0, 0);
        surface.set_cell(0, 0, 'A', Some(Style { bg: (0, 0, 0), fg, ..Style::default() }));

        let atlas = GpuGlyphAtlas::new();
        let (rgba, _, _) = blit_rgba(&surface, &atlas, CHAR_W, LINE_H, 1.0);
        assert_eq!(&rgba[..3], &[fg.0, fg.1, fg.2]);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn terminal_grid_reserves_the_jetbrains_tab_strip() {
        assert_eq!(terminal_grid_size(800.0, 0, 7, 17), (114, 10));
        assert_eq!(terminal_grid_size(800.0, 1, 7, 17), (114, 8));
    }

    #[test]
    fn terminal_background_comes_from_the_theme() {
        let atlas = GpuGlyphAtlas::new();
        let palette = crate::theme::palette("intellij-dark").terminal;
        let lines = vec![kernel::terminal::screen::StyledLine::default()];
        let (rgba, _, _) = blit_terminal_rgba(&lines, &atlas, 1, 1, 8, 16, &palette, None);
        assert_eq!(&rgba[..3], &[palette.bg.0, palette.bg.1, palette.bg.2]);
    }

    #[test]
    fn terminal_ansi_slots_come_from_the_theme() {
        let atlas = GpuGlyphAtlas::new();
        let palette = crate::theme::palette("intellij-dark").terminal;
        let style = kernel::terminal::screen::Style {
            bg: kernel::terminal::Color::Indexed(1),
            ..Default::default()
        };
        let lines = vec![kernel::terminal::screen::StyledLine {
            text: " ".into(),
            spans: vec![kernel::terminal::screen::Span { start: 0, len: 1, style }],
        }];
        let (rgba, _, _) = blit_terminal_rgba(&lines, &atlas, 1, 1, 8, 16, &palette, None);
        let ansi = palette.ansi[1];
        assert_eq!(&rgba[..3], &[ansi.0, ansi.1, ansi.2]);
    }

    #[test]
    fn terminal_cursor_uses_the_theme_cursor_color() {
        let atlas = GpuGlyphAtlas::new();
        let palette = crate::theme::palette("intellij-dark").terminal;
        let lines = vec![kernel::terminal::screen::StyledLine::default()];
        let (rgba, _, _) = blit_terminal_rgba(&lines, &atlas, 1, 1, 8, 16, &palette, Some((0, 0)));
        assert_eq!(&rgba[..3], &[palette.cursor.0, palette.cursor.1, palette.cursor.2]);
    }
}

