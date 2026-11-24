//! Refresh plumbing shared with `main.rs`: the model re-projection gate,
//! the editor grid sizing, and the pane revision signature.

use slint::{ComponentHandle, ModelRc, VecModel};

use kernel::state::Editor;

use crate::{AppWindow, logical_size};

/// Re-projects a model-valued property only when the row data actually
/// changed: replacing the model every refresh dirties Slint's scene graph
/// and forces a full re-layout even when nothing moved.
pub(crate) struct ModelGate<T> {
    last: Vec<T>,
}

impl<T: PartialEq + Clone + 'static> ModelGate<T> {
    pub(crate) fn new() -> Self {
        ModelGate { last: Vec::new() }
    }

    pub(crate) fn set(&mut self, next: Vec<T>, set: impl FnOnce(ModelRc<T>)) {
        if self.last == next {
            return;
        }
        self.last = next.clone();
        set(ModelRc::new(VecModel::from(next)));
    }
}

/// Editor grid from the actual laid-out pane, so the image, the surface, and
/// click hit-testing all agree with what is on screen.  Before the first
/// layout pass the element has no size yet; fall back to the window size
/// minus the fixed chrome (menu, tabs, breadcrumb, bottom bar, status).
pub(crate) fn editor_grid(app: &AppWindow, cell_w: u32, cell_h: u32) -> (u32, u32) {
    let mut w = app.get_editor_view_width() as f32;
    let mut h = app.get_editor_view_height() as f32;
    // Before the first real layout pass the view reports its minimum size.
    // Deriving the viewport scroll from that postage stamp sticks once the
    // pane reaches its real size — the cursor-follow rules only guarantee
    // the cursor stays inside, never that the view re-centers — so treat
    // unbelievable sizes the same as "no size yet".
    if w < 200.0 || h < 100.0 {
        let (mut lw, mut lh) = logical_size(app);
        // A not-yet-mapped window reports a stub size; sizing the surface
        // from it would freeze the viewport scroll at a postage stamp.
        // The window's preferred size is the honest guess until the real
        // layout lands.
        if lw < 300.0 || lh < 200.0 {
            lw = 1280.0;
            lh = 800.0;
        }
        w = (lw - 304.0).max(80.0);
        h = (lh - 132.0).max(40.0);
    }
    (
        (w / cell_w.max(1) as f32).floor().max(1.0) as u32,
        (h / cell_h.max(1) as f32).floor().max(1.0) as u32,
    )
}

// ── Hover card geometry ─────────────────────────────────────────────────────
// The Slint component takes its height from the `hover-height` property, so
// these estimates are the single source of truth for the pin hit-test, the
// edge-flip math, and the rendered size.

const CARD_W: f32 = 420.0;
const LINE_H: f32 = 24.0;
const CARD_CHROME: f32 = 64.0;
const CARD_MAX_H: f32 = 440.0;
/// View-edge margin and pointer→card gaps.
const EDGE: f32 = 8.0;
const GAP_X: f32 = 14.0;
const GAP_Y: f32 = 20.0;

/// The editor view's logical pixel size — hover coordinates are
/// editor-view-relative, so clamping against the view (not the window)
/// keeps the card inside the window whatever the pane chrome offset is.
/// Before the first layout pass the element reports a stub size; fall back
/// to the window like [`editor_grid`] does.
pub(super) fn view_size(app: &AppWindow) -> (f32, f32) {
    let w = app.get_editor_view_width() as f32;
    let h = app.get_editor_view_height() as f32;
    if w >= 200.0 && h >= 100.0 {
        return (w, h);
    }
    let size = app.window().size();
    let scale = app.window().scale_factor().max(1.0);
    (size.width as f32 / scale, size.height as f32 / scale)
}

pub(super) fn hover_card_geometry(line_count: usize) -> (f32, f32) {
    let h = (line_count as f32 * LINE_H + CARD_CHROME).min(CARD_MAX_H);
    (CARD_W, h)
}

/// Place the card below-right of the pointer, flipping over the opposite
/// edge (above at the bottom, left at the right) when that spot would run
/// past the view; clamp as the last resort for cards bigger than the view.
pub(super) fn hover_card_position(
    px: f32,
    py: f32,
    w: f32,
    h: f32,
    view_w: f32,
    view_h: f32,
) -> (f32, f32) {
    let right = px + GAP_X;
    let below = py + GAP_Y;
    let x = if right + w <= view_w - EDGE {
        right
    } else if px - GAP_X - w >= EDGE {
        px - GAP_X - w
    } else {
        right.clamp(EDGE, (view_w - w - EDGE).max(EDGE))
    };
    let y = if below + h <= view_h - EDGE {
        below
    } else if py - GAP_Y - h >= EDGE {
        py - GAP_Y - h
    } else {
        below.clamp(EDGE, (view_h - h - EDGE).max(EDGE))
    };
    (x, y)
}

/// Boundary-aware count of exact whole-word occurrences.
pub(super) fn count_word_occurrences(text: &str, word: &str) -> usize {
    if word.is_empty() {
        return 0;
    }
    let edge = |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut n = 0;
    let mut from = 0;
    while let Some(rel) = text[from..].find(word) {
        let start = from + rel;
        let end = start + word.len();
        if edge(text[..start].chars().next_back()) && edge(text[end..].chars().next()) {
            n += 1;
        }
        from = end;
    }
    n
}

pub(crate) fn revision_of(editor: &Editor) -> u64 {
    editor
        .buffers
        .iter()
        .map(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).revision())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_opens_below_right_when_it_fits() {
        let (x, y) = hover_card_position(100.0, 100.0, 420.0, 200.0, 1200.0, 800.0);
        assert_eq!((x, y), (114.0, 120.0));
    }

    #[test]
    fn card_flips_above_at_the_bottom_edge() {
        let (x, y) = hover_card_position(100.0, 700.0, 420.0, 200.0, 1200.0, 800.0);
        // below would end at 920 > 792; above ends at 480 — fully visible
        assert_eq!(y, 480.0);
        assert_eq!(x, 114.0);
    }

    #[test]
    fn card_stays_below_while_it_fits() {
        let (x, y) = hover_card_position(100.0, 320.0, 420.0, 400.0, 1200.0, 800.0);
        // below ends at 740 ≤ 792 — no flip
        assert_eq!(y, 340.0);
        let (x, y) = hover_card_position(100.0, 500.0, 420.0, 400.0, 1200.0, 800.0);
        // below ends at 940 > 792; above starts at 80 ≥ 8 — flips
        assert_eq!(y, 80.0);
        assert_eq!(x, 114.0);
    }

    #[test]
    fn card_clamps_when_neither_side_fits() {
        let (x, y) = hover_card_position(50.0, 250.0, 420.0, 300.0, 500.0, 500.0);
        // below ends at 570, above would start at -70 → clamp into the view
        assert_eq!(x, 64.0);
        assert_eq!(y, 192.0);
    }

    #[test]
    fn card_flips_left_at_the_right_edge() {
        let (x, y) = hover_card_position(1000.0, 100.0, 420.0, 200.0, 1200.0, 800.0);
        assert_eq!(x, 566.0);
        assert_eq!(y, 120.0);
    }

    #[test]
    fn geometry_grows_with_lines_and_caps() {
        assert_eq!(hover_card_geometry(1), (420.0, 88.0));
        assert_eq!(hover_card_geometry(100).1, 440.0);
    }

    #[test]
    fn occurrences_count_whole_words_only() {
        assert_eq!(count_word_occurrences("cat scatter cat. cat-", "cat"), 3);
        assert_eq!(count_word_occurrences("cats cat", "cat"), 1);
        assert_eq!(count_word_occurrences("", "cat"), 0);
        assert_eq!(count_word_occurrences("cat", ""), 0);
        assert_eq!(count_word_occurrences("cabìn cabìn", "cabìn"), 2);
    }
}

