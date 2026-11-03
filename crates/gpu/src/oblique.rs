//! Synthetic oblique for faces that ship no italic — Monaco, and most
//! system monospaces, have a roman design only.
//!
//! The shear belongs to the outline, so the rasterizer computes coverage for
//! the slanted shape itself.  Shearing the pixels afterwards instead would
//! quantize each row to a whole texel and make every glyph's slant depend on
//! its own ink, which is what a synthetic italic must not do.

use ab_glyph::{Outline, OutlineCurve, Point, Rect};

/// Slant of the synthetic italic: horizontal units per unit of height,
/// matched to the ~10° a real monospace italic face leans by.
pub const SLOPE: f32 = 0.17;

/// Shear `outline` sideways in proportion to its height above `pivot` (in
/// the outline's own units, measured up from the baseline).  Height is
/// untouched, so the glyph keeps its rows and leans as one shape.
pub fn shear(outline: &mut Outline, slope: f32, pivot: f32) {
    let slant = |p: Point| Point {
        x: p.x + slope * (p.y - pivot),
        y: p.y,
    };
    for curve in &mut outline.curves {
        *curve = match *curve {
            OutlineCurve::Line(a, b) => OutlineCurve::Line(slant(a), slant(b)),
            OutlineCurve::Quad(a, b, c) => OutlineCurve::Quad(slant(a), slant(b), slant(c)),
            OutlineCurve::Cubic(a, b, c, d) => {
                OutlineCurve::Cubic(slant(a), slant(b), slant(c), slant(d))
            }
        };
    }
    // The bounding box drives `OutlinedGlyph`'s pixel bounds, and the
    // rasterizer indexes its buffer from it: a box that misses the sheared
    // shape sends the outline's own points out of range.  The shear moves
    // each corner by its own height, so the extremes come from all four —
    // under a y-up box the leftmost ink is a top corner, not the bottom one.
    let corner = |x: f32, y: f32| slant(Point { x, y }).x;
    let xs = [
        corner(outline.bounds.min.x, outline.bounds.min.y),
        corner(outline.bounds.max.x, outline.bounds.min.y),
        corner(outline.bounds.min.x, outline.bounds.max.y),
        corner(outline.bounds.max.x, outline.bounds.max.y),
    ];
    outline.bounds = Rect {
        min: Point {
            x: xs.iter().copied().fold(f32::INFINITY, f32::min),
            y: outline.bounds.min.y,
        },
        max: Point {
            x: xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
            y: outline.bounds.max.y,
        },
    };
}
