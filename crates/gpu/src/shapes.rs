//! Procedural shape coverage — gutter markers drawn by geometry instead of
//! font glyphs, so they look identical at every editor font and DPI.

/// The glyphs the kernel paints as gutter markers.  Several of these
/// codepoints are missing from common mono faces (which renders the box
/// fallback in their place) and the font's own symbol designs vary in size;
/// baking geometry into the atlas cells removes the font from the equation.
pub const MARKER_GLYPHS: &[char] = &['●', '○', '▎', '▸', '▾'];

pub fn is_marker(ch: char) -> bool {
    MARKER_GLYPHS.contains(&ch)
}

/// Paint `ch` as geometry into the RGBA atlas buffer at the given cell.
/// Returns false for non-marker characters so the caller falls back to the
/// font outline.
pub fn paint_marker(
    pixels: &mut [u8],
    ch: char,
    cell_x: u32,
    cell_y: u32,
    cell_w: u32,
    cell_h: u32,
    atlas_w: u32,
) -> bool {
    if !is_marker(ch) {
        return false;
    }
    for py in 0..cell_h {
        for px in 0..cell_w {
            let a = marker_alpha(ch, px, py, cell_w, cell_h);
            if a == 0 {
                continue;
            }
            let idx = (((cell_y + py) * atlas_w + cell_x + px) * 4) as usize;
            if idx + 3 < pixels.len() {
                pixels[idx] = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 255;
                pixels[idx + 3] = a;
            }
        }
    }
    true
}

fn marker_alpha(ch: char, px: u32, py: u32, cell_w: u32, cell_h: u32) -> u8 {
    match ch {
        '●' => disc_alpha(px, py, cell_w, cell_h, true),
        '○' => disc_alpha(px, py, cell_w, cell_h, false),
        '▎' => bar_alpha(px, py, cell_w, cell_h),
        '▸' => triangle_alpha(px, py, cell_w, cell_h, false),
        '▾' => triangle_alpha(px, py, cell_w, cell_h, true),
        _ => 0,
    }
}

/// Anti-aliased coverage of a signed distance (inside ≤ 0): one pixel of
/// feather across the boundary.
fn antialias(d: f32) -> u8 {
    ((0.5 - d).clamp(0.0, 1.0) * 255.0) as u8
}

/// The largest disc radius that keeps a clear pixel between the disc and the
/// cell boundary — the dot reads as fat as the marker column allows without
/// touching its neighbours.
fn disc_radius(cell_w: u32, cell_h: u32) -> f32 {
    cell_w.min(cell_h).max(2) as f32 / 2.0 - 1.0
}

/// Anti-aliased coverage (0–255) of a breakpoint disc at one pixel of a
/// `cell_w × cell_h` cell.  `filled` renders a solid disc (●), otherwise a
/// ring (○).  The disc is centred in the cell.
pub fn disc_alpha(px: u32, py: u32, cell_w: u32, cell_h: u32, filled: bool) -> u8 {
    let cw = cell_w.max(1) as f32;
    let ch = cell_h.max(1) as f32;
    let cx = cw / 2.0;
    let cy = ch / 2.0;
    let radius = disc_radius(cell_w, cell_h);
    let dx = px as f32 + 0.5 - cx;
    let dy = py as f32 + 0.5 - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    if filled {
        antialias(dist - radius)
    } else {
        let half_thickness = (radius * 0.30).max(1.0) / 2.0;
        antialias((dist - radius).abs() - half_thickness)
    }
}

/// The git-sign bar (`▎`): a full-height slab hanging off the cell's left
/// edge.
pub fn bar_alpha(px: u32, py: u32, cell_w: u32, cell_h: u32) -> u8 {
    let cw = cell_w.max(1) as f32;
    let ch = cell_h.max(1) as f32;
    let x = px as f32 + 0.5;
    let y = py as f32 + 0.5;
    let dx = (cw * 0.10 - x).max(x - cw * 0.52);
    let dy = (ch * 0.04 - y).max(y - ch * 0.96);
    antialias(dx.max(dy))
}

/// Fold arrows (`▸`/`▾`), spanning the middle 60% of the cell.  Faces ship
/// only the small codepoint variants, which read as pinpricks at body size.
pub fn triangle_alpha(px: u32, py: u32, cell_w: u32, cell_h: u32, down: bool) -> u8 {
    let cw = cell_w.max(1) as f32;
    let ch = cell_h.max(1) as f32;
    let x = px as f32 + 0.5;
    let y = py as f32 + 0.5;
    // Wound so the interior is on the positive side of every edge cross.
    let v = if down {
        [
            (cw * 0.12, ch * 0.20),
            (cw * 0.88, ch * 0.20),
            (cw * 0.50, ch * 0.80),
        ]
    } else {
        [
            (cw * 0.20, ch * 0.50),
            (cw * 0.80, ch * 0.20),
            (cw * 0.80, ch * 0.80),
        ]
    };
    let mut inside = true;
    let mut d_in = f32::NEG_INFINITY;
    let mut d_out = f32::INFINITY;
    for i in 0..3 {
        let (x0, y0) = v[i];
        let (x1, y1) = v[(i + 1) % 3];
        let ex = x1 - x0;
        let ey = y1 - y0;
        let len = (ex * ex + ey * ey).sqrt().max(1.0);
        let edge = (ex * (y - y0) - ey * (x - x0)) / len;
        inside &= edge >= 0.0;
        d_in = d_in.max(edge);
        // True distance to the edge *segment*: the half-plane distance alone
        // overestimates outside points past a slanted edge's ends.
        let t = (((x - x0) * ex + (y - y0) * ey) / (len * len)).clamp(0.0, 1.0);
        let qx = x - (x0 + t * ex);
        let qy = y - (y0 + t * ey);
        d_out = d_out.min((qx * qx + qy * qy).sqrt());
    }
    // Vertices are wound inside-positive; coverage wants inside-negative.
    antialias(if inside { -d_in } else { d_out })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CW: u32 = 18;
    const CH: u32 = 34;

    #[test]
    fn filled_disc_is_solid_at_center_and_clear_at_corners() {
        assert_eq!(disc_alpha(CW / 2, CH / 2, CW, CH, true), 255);
        assert_eq!(disc_alpha(0, 0, CW, CH, true), 0);
        assert_eq!(disc_alpha(CW - 1, CH - 1, CW, CH, true), 0);
    }

    #[test]
    fn ring_is_hollow_at_center_and_painted_on_the_rim() {
        assert_eq!(disc_alpha(CW / 2, CH / 2, CW, CH, false), 0);
        let rim_x = (CW as f32 / 2.0 + disc_radius(CW, CH) - 0.5) as u32;
        assert!(
            disc_alpha(rim_x, CH / 2, CW, CH, false) > 0,
            "the rim must carry coverage"
        );
    }

    #[test]
    fn disc_fills_the_marker_column_but_never_touches_the_cell_edge() {
        let mut ink_cols = 0;
        for px in 0..CW {
            if disc_alpha(px, CH / 2, CW, CH, true) > 0 {
                ink_cols += 1;
            }
        }
        assert!(ink_cols >= CW * 3 / 4, "the disc must read as a fat dot");
        let edge = disc_alpha(0, CH / 2, CW, CH, true);
        assert_eq!(edge, 0, "cell edge must stay clear");
    }

    #[test]
    fn markers_are_geometry_only_for_the_gutter_set() {
        for ch in MARKER_GLYPHS.iter().copied() {
            assert!(is_marker(ch));
            assert!(
                (0..CW).any(|px| marker_alpha(ch, px, CH / 2, CW, CH) > 0),
                "{ch} must carry ink on the cell's middle row"
            );
        }
        assert!(!is_marker('a') && !is_marker('█') && !is_marker('□'));
        assert_eq!(marker_alpha('a', CW / 2, CH / 2, CW, CH), 0);
    }

    #[test]
    fn fold_triangles_fill_their_cell_without_touching_the_base_edge() {
        for down in [false, true] {
            assert_eq!(triangle_alpha(CW / 2, CH / 2, CW, CH, down), 255);
            assert_eq!(triangle_alpha(1, 1, CW, CH, down), 0);
            assert_eq!(triangle_alpha(1, CH - 2, CW, CH, down), 0);
            assert_eq!(
                triangle_alpha(CW - 1, CH / 2, CW, CH, down),
                0,
                "the base must keep a margin from the cell edge"
            );
            let mut ink_rows = 0;
            for py in 0..CH {
                if (0..CW).any(|px| triangle_alpha(px, py, CW, CH, down) > 0) {
                    ink_rows += 1;
                }
            }
            assert!(
                ink_rows >= CH / 2,
                "the arrow must span half the cell height, got {ink_rows}/{CH}"
            );
        }
    }

    #[test]
    fn git_sign_bar_hangs_off_the_left_edge_full_height() {
        assert_eq!(bar_alpha(2, CH / 2, CW, CH), 255);
        assert_eq!(
            bar_alpha(CW - 1, CH / 2, CW, CH),
            0,
            "right half stays clear"
        );
        assert!(bar_alpha(2, 4, CW, CH) > 0 && bar_alpha(2, CH - 5, CW, CH) > 0);
    }
}
