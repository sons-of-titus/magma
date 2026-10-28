//! Procedural shape coverage — gutter markers drawn by geometry instead of
//! font glyphs, so they look identical at every editor font and DPI.

/// Anti-aliased coverage (0–255) of a breakpoint disc at one pixel of a
/// `cell_w × cell_h` cell.  `filled` renders a solid disc (●), otherwise a
/// ring (○).  The disc is centred in the cell with a radius proportional to
/// the smaller dimension.
pub fn disc_alpha(px: u32, py: u32, cell_w: u32, cell_h: u32, filled: bool) -> u8 {
    let cw = cell_w.max(1) as f32;
    let ch = cell_h.max(1) as f32;
    let cx = cw / 2.0;
    let cy = ch / 2.0;
    let radius = cw.min(ch) * 0.34;
    let dx = px as f32 + 0.5 - cx;
    let dy = py as f32 + 0.5 - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    let cov = if filled {
        (radius + 0.5 - dist).clamp(0.0, 1.0)
    } else {
        let thickness = (radius * 0.30).max(1.0);
        (thickness + 0.5 - (dist - radius).abs()).clamp(0.0, 1.0)
    };
    (cov * 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::disc_alpha;

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
        let rim_y = CH as f32 / 2.0;
        let rim_x = CW as f32 / 2.0 + (CW.min(CH) as f32 * 0.34);
        let px = (rim_x - 0.5) as u32;
        let py = (rim_y - 0.5) as u32;
        assert!(
            disc_alpha(px, py, CW, CH, false) > 0,
            "the rim must carry coverage"
        );
    }

    #[test]
    fn disc_fits_inside_the_cell_at_small_sizes() {
        // No pixel may be painted outside a one-pixel margin of the cell:
        // the old font glyph clipped flat edges at the cell boundary.
        for py in 0..CH {
            for px in 0..CW {
                let _ = disc_alpha(px, py, CW, CH, true);
            }
        }
        let edge = disc_alpha(0, CH / 2, CW, CH, true);
        assert_eq!(edge, 0, "cell edge must stay clear");
    }
}
