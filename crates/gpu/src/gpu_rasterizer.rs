//! Real glyph rasterization using `ab_glyph` — Phase 4.
//!
//! Produces an RGBA pixel buffer for the GPU atlas texture.  Each character
//! occupies a fixed `cell_w × cell_h` cell in a grid with `cols` columns.
//! The glyph is horizontally centred in the cell and vertically positioned
//! at the font's ascent metric so all baselines align.
//!
//! Supports a fallback font chain so scripts absent from the primary monospace
//! face (Arabic, Hebrew, extended Latin/Cyrillic, symbols, CJK) are resolved
//! from the first fallback font that carries the glyph.

use ab_glyph::{
    Font as _, FontRef, Glyph, GlyphId, OutlinedGlyph, PxScale, PxScaleFactor, ScaleFont as _,
};

use super::gpu_atlas::{ATLAS_CELL_H, ATLAS_CELL_W};

/// Rasterize `chars` into an RGBA atlas pixel buffer using a single font.
pub fn rasterize_font_atlas(
    font_bytes: &[u8],
    font_size: f32,
    chars: &[char],
    cell_w: u32,
    cell_h: u32,
    cols: u32,
) -> Vec<u8> {
    rasterize_font_atlas_with_fallbacks(&[font_bytes], font_size, chars, cell_w, cell_h, cols)
}

/// Rasterize `chars` into an RGBA atlas pixel buffer using a primary font
/// plus ordered fallback fonts.
pub fn rasterize_font_atlas_with_fallbacks(
    fonts: &[&[u8]],
    font_size: f32,
    chars: &[char],
    cell_w: u32,
    cell_h: u32,
    cols: u32,
) -> Vec<u8> {
    rasterize_grid(fonts, font_size, chars, cell_w, cell_h, cols, None)
}

/// Rasterize `chars` as an oblique face using a single font.
pub fn rasterize_oblique_atlas(
    font_bytes: &[u8],
    font_size: f32,
    chars: &[char],
    cell_w: u32,
    cell_h: u32,
    cols: u32,
) -> Vec<u8> {
    rasterize_oblique_atlas_with_fallbacks(&[font_bytes], font_size, chars, cell_w, cell_h, cols)
}

/// Rasterize `chars` as an oblique face using a primary font plus ordered
/// fallback fonts.
pub fn rasterize_oblique_atlas_with_fallbacks(
    fonts: &[&[u8]],
    font_size: f32,
    chars: &[char],
    cell_w: u32,
    cell_h: u32,
    cols: u32,
) -> Vec<u8> {
    rasterize_grid(fonts, font_size, chars, cell_w, cell_h, cols, Some(super::oblique::SLOPE))
}

/// Return `true` if any font in `fonts` provides a non-`.notdef` glyph for `ch`.
pub fn has_glyph(fonts: &[&[u8]], ch: char) -> bool {
    let parsed: Vec<FontRef<'_>> = fonts
        .iter()
        .filter_map(|b| FontRef::try_from_slice(b).ok())
        .collect();
    resolve_glyph(&parsed, ch).is_some()
}

/// Rasterize a single dynamic character `ch` into its upright cell
/// `(roman_x, cell_y)` and oblique cell `(italic_x, cell_y)` in `pixels`.
/// Returns `true` if a font in `fonts` provided a glyph for `ch`.
#[allow(clippy::too_many_arguments)]
pub fn rasterize_dynamic_pair(
    pixels: &mut [u8],
    atlas_w: u32,
    fonts: &[&[u8]],
    font_size: f32,
    ch: char,
    roman_x: u32,
    italic_x: u32,
    cell_y: u32,
    cell_w: u32,
    cell_h: u32,
) -> bool {
    if super::shapes::paint_marker(pixels, ch, roman_x, cell_y, cell_w, cell_h, atlas_w) {
        let _ = super::shapes::paint_marker(pixels, ch, italic_x, cell_y, cell_w, cell_h, atlas_w);
        return true;
    }
    let parsed: Vec<FontRef<'_>> = fonts
        .iter()
        .filter_map(|b| FontRef::try_from_slice(b).ok())
        .collect();
    let Some(primary) = parsed.first() else {
        return false;
    };
    let primary_ascent = primary.as_scaled(PxScale::from(font_size)).ascent();
    let Some((font_idx, glyph_id)) = resolve_glyph(&parsed, ch) else {
        return false;
    };
    let font = &parsed[font_idx];
    for (cell_x, oblique) in [(roman_x, None), (italic_x, Some(super::oblique::SLOPE))] {
        paint_glyph_cell(
            pixels,
            atlas_w,
            font,
            glyph_id,
            font_size,
            primary_ascent,
            cell_x,
            cell_y,
            cell_w,
            cell_h,
            oblique,
            true,
        );
    }
    true
}

fn rasterize_grid(
    fonts: &[&[u8]],
    font_size: f32,
    chars: &[char],
    cell_w: u32,
    cell_h: u32,
    cols: u32,
    oblique: Option<f32>,
) -> Vec<u8> {
    let rows = (chars.len() as u32).div_ceil(cols);
    let width = cols * cell_w;
    let height = rows * cell_h;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    fill_cell_white(&mut pixels, 0, 0, cell_w, cell_h, width);

    let parsed: Vec<FontRef<'_>> = fonts
        .iter()
        .filter_map(|b| FontRef::try_from_slice(b).ok())
        .collect();
    let Some(primary) = parsed.first() else {
        return vec![255u8; (width * height * 4) as usize];
    };
    let primary_ascent = primary.as_scaled(PxScale::from(font_size)).ascent();

    for (i, &ch) in chars.iter().enumerate() {
        let col = (i as u32) % cols;
        let row = (i as u32) / cols;
        let cell_x = col * cell_w;
        let cell_y = row * cell_h;

        if super::shapes::paint_marker(&mut pixels, ch, cell_x, cell_y, cell_w, cell_h, width) {
            continue;
        }

        let (font_idx, glyph_id) =
            resolve_glyph(&parsed, ch).unwrap_or((0, primary.glyph_id(ch)));
        let fit_to_cell = font_idx > 0;
        paint_glyph_cell(
            &mut pixels,
            width,
            &parsed[font_idx],
            glyph_id,
            font_size,
            primary_ascent,
            cell_x,
            cell_y,
            cell_w,
            cell_h,
            oblique,
            fit_to_cell,
        );
    }
    pixels
}

fn resolve_glyph(parsed: &[FontRef<'_>], ch: char) -> Option<(usize, GlyphId)> {
    for (idx, font) in parsed.iter().enumerate() {
        let gid = font.glyph_id(ch);
        if gid.0 != 0 {
            return Some((idx, gid));
        }
    }
    if let Some(base) = arabic_presentation_base(ch) {
        for (idx, font) in parsed.iter().enumerate() {
            let gid = font.glyph_id(base);
            if gid.0 != 0 {
                return Some((idx, gid));
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn paint_glyph_cell(
    pixels: &mut [u8],
    width: u32,
    font: &FontRef<'_>,
    glyph_id: GlyphId,
    font_size: f32,
    primary_ascent: f32,
    cell_x: u32,
    cell_y: u32,
    cell_w: u32,
    cell_h: u32,
    oblique: Option<f32>,
    fit_to_cell: bool,
) {
    let pivot = (font.ascent_unscaled() + font.descent_unscaled()) / 2.0;
    let mut scale = PxScale::from(font_size);

    if fit_to_cell {
        let scaled0 = font.as_scaled(scale);
        let probe_glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
        if let Some(probe) = outline(
            font,
            glyph_id,
            probe_glyph,
            oblique,
            pivot,
            scaled0.scale_factor(),
        ) {
            let b = probe.px_bounds();
            let bw = b.width();
            let bh = b.height();
            let max_w = (cell_w as f32 - 1.0).max(1.0);
            let max_h = (cell_h as f32 - 1.0).max(1.0);
            if bw > max_w || bh > max_h {
                let ratio = (max_w / bw.max(1.0)).min(max_h / bh.max(1.0)).min(1.0);
                scale = PxScale::from((font_size * ratio).max(1.0));
            }
        }
    }

    let scaled = font.as_scaled(scale);
    let scale_factor = scaled.scale_factor();
    let h_advance = scaled.h_advance(glyph_id);
    let mut pen_x = ((cell_w as f32 - h_advance) / 2.0).max(0.0);
    let mut pen_y = if fit_to_cell {
        primary_ascent
    } else {
        scaled.ascent()
    };

    if fit_to_cell {
        let probe_glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
        if let Some(probe) = outline(font, glyph_id, probe_glyph, oblique, pivot, scale_factor) {
            let b = probe.px_bounds();
            if pen_x + b.min.x < 0.0 || pen_x + b.max.x > cell_w as f32 {
                pen_x = (cell_w as f32 - (b.max.x - b.min.x)) / 2.0 - b.min.x;
            }
            if pen_y + b.max.y > cell_h as f32 {
                pen_y -= (pen_y + b.max.y) - cell_h as f32;
            }
            if pen_y + b.min.y < 0.0 {
                pen_y -= pen_y + b.min.y;
            }
        }
    }

    let glyph = glyph_id.with_scale_and_position(
        scale,
        ab_glyph::point(cell_x as f32 + pen_x, cell_y as f32 + pen_y),
    );
    let Some(outlined) = outline(font, glyph_id, glyph, oblique, pivot, scale_factor) else {
        return;
    };

    let bounds = outlined.px_bounds();
    let min_x = bounds.min.x as i64;
    let min_y = bounds.min.y as i64;
    let cell_x0 = cell_x as i64;
    let cell_y0 = cell_y as i64;
    let cell_x1 = cell_x0 + cell_w as i64;
    let cell_y1 = cell_y0 + cell_h as i64;
    outlined.draw(|px, py, cov| {
        if cov < 0.004 {
            return;
        }
        let x = min_x + px as i64;
        let y = min_y + py as i64;
        if x < cell_x0 || y < cell_y0 || x >= cell_x1 || y >= cell_y1 {
            return;
        }
        let idx = ((y * width as i64 + x) * 4) as usize;
        pixels[idx] = 255;
        pixels[idx + 1] = 255;
        pixels[idx + 2] = 255;
        pixels[idx + 3] = (cov * 255.0).min(255.0) as u8;
    });
}

fn outline(
    font: &FontRef,
    id: GlyphId,
    glyph: Glyph,
    oblique: Option<f32>,
    pivot: f32,
    scale_factor: PxScaleFactor,
) -> Option<OutlinedGlyph> {
    let Some(slope) = oblique else {
        return font.outline_glyph(glyph);
    };
    let mut raw = font.outline(id)?;
    super::oblique::shear(&mut raw, slope, pivot);
    Some(OutlinedGlyph::new(glyph, raw, scale_factor))
}

fn fill_cell_white(
    pixels: &mut [u8],
    cell_x: u32,
    cell_y: u32,
    cell_w: u32,
    cell_h: u32,
    atlas_w: u32,
) {
    for y in cell_y..cell_y + cell_h {
        for x in cell_x..cell_x + cell_w {
            let idx = ((y * atlas_w + x) * 4) as usize;
            if idx + 3 < pixels.len() {
                pixels[idx] = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 255;
                pixels[idx + 3] = 255;
            }
        }
    }
}

/// Map an Arabic Presentation Form character (`U+FB50..=U+FEFC`) back to
/// its standard `U+0621..=U+06CC` base letter if a font only indexes the
/// base codepoints in `cmap`.
fn arabic_presentation_base(ch: char) -> Option<char> {
    let base = match ch {
        '\u{FB56}'..='\u{FB59}' => '\u{067E}',
        '\u{FB6A}'..='\u{FB6D}' => '\u{06A4}',
        '\u{FB7A}'..='\u{FB7D}' => '\u{0686}',
        '\u{FB8A}'..='\u{FB8B}' => '\u{0698}',
        '\u{FB8E}'..='\u{FB91}' => '\u{06A9}',
        '\u{FB92}'..='\u{FB95}' => '\u{06AF}',
        '\u{FBFC}'..='\u{FBFF}' => '\u{06CC}',
        '\u{FE80}' => '\u{0621}',
        '\u{FE81}'..='\u{FE82}' => '\u{0622}',
        '\u{FE83}'..='\u{FE84}' => '\u{0623}',
        '\u{FE85}'..='\u{FE86}' => '\u{0624}',
        '\u{FE87}'..='\u{FE88}' => '\u{0625}',
        '\u{FE89}'..='\u{FE8C}' => '\u{0626}',
        '\u{FE8D}'..='\u{FE8E}' => '\u{0627}',
        '\u{FE8F}'..='\u{FE92}' => '\u{0628}',
        '\u{FE93}'..='\u{FE94}' => '\u{0629}',
        '\u{FE95}'..='\u{FE98}' => '\u{062A}',
        '\u{FE99}'..='\u{FE9C}' => '\u{062B}',
        '\u{FE9D}'..='\u{FEA0}' => '\u{062C}',
        '\u{FEA1}'..='\u{FEA4}' => '\u{062D}',
        '\u{FEA5}'..='\u{FEA8}' => '\u{062E}',
        '\u{FEA9}'..='\u{FEAA}' => '\u{062F}',
        '\u{FEAB}'..='\u{FEAC}' => '\u{0630}',
        '\u{FEAD}'..='\u{FEAE}' => '\u{0631}',
        '\u{FEAF}'..='\u{FEB0}' => '\u{0632}',
        '\u{FEB1}'..='\u{FEB4}' => '\u{0633}',
        '\u{FEB5}'..='\u{FEB8}' => '\u{0634}',
        '\u{FEB9}'..='\u{FEBC}' => '\u{0635}',
        '\u{FEBD}'..='\u{FEC0}' => '\u{0636}',
        '\u{FEC1}'..='\u{FEC4}' => '\u{0637}',
        '\u{FEC5}'..='\u{FEC8}' => '\u{0638}',
        '\u{FEC9}'..='\u{FECC}' => '\u{0639}',
        '\u{FECD}'..='\u{FED0}' => '\u{063A}',
        '\u{FED1}'..='\u{FED4}' => '\u{0641}',
        '\u{FED5}'..='\u{FED8}' => '\u{0642}',
        '\u{FED9}'..='\u{FEDC}' => '\u{0643}',
        '\u{FEDD}'..='\u{FEE0}' => '\u{0644}',
        '\u{FEE1}'..='\u{FEE4}' => '\u{0645}',
        '\u{FEE5}'..='\u{FEE8}' => '\u{0646}',
        '\u{FEE9}'..='\u{FEEC}' => '\u{0647}',
        '\u{FEED}'..='\u{FEEE}' => '\u{0648}',
        '\u{FEEF}'..='\u{FEF0}' => '\u{0649}',
        '\u{FEF1}'..='\u{FEF4}' => '\u{064A}',
        _ => return None,
    };
    Some(base)
}

const _: u32 = ATLAS_CELL_W;
const _: u32 = ATLAS_CELL_H;
