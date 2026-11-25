//! Vertical scrollbar geometry for the editor pane, as pure functions plus
//! the sync that pushes them into the view.

use crate::{AppState, AppWindow};

/// (thumb size, thumb position), both fractions of the track.  The wheel may
/// scroll the last line up to the top, so the scrollable extent is
/// `total - 1 + visible` rows.
pub fn thumb(total_lines: usize, visible_rows: usize, top: usize) -> (f32, f32) {
    let visible = visible_rows.max(1) as f32;
    let max_top = total_lines.saturating_sub(1);
    if max_top == 0 {
        return (1.0, 0.0);
    }
    let size = (visible / (max_top as f32 + visible)).clamp(0.0, 1.0);
    (size, (top.min(max_top) as f32 / max_top as f32).clamp(0.0, 1.0))
}

/// Top line for a track fraction dragged by the user.
pub fn top_for_fraction(total_lines: usize, fraction: f32) -> usize {
    let max_top = total_lines.saturating_sub(1);
    (fraction.clamp(0.0, 1.0) * max_top as f32).round() as usize
}

/// Whether the caret's line is inside the viewport.  The saturating row math
/// in the kernel pins an off-screen-above caret to row 0, which would make it
/// look like it follows the scroll.
pub fn caret_in_view(cursor_line: usize, top: usize, visible_rows: usize) -> bool {
    cursor_line >= top && cursor_line < top + visible_rows.max(1)
}

pub(super) fn sync(app: &AppWindow, state: &AppState) {
    let ed = &state.editor;
    let Some(pane) = ed
        .view_tree
        .focused_window()
        .and_then(|wid| ed.view_tree.window(wid))
    else {
        app.set_scrollbar_visible(false);
        return;
    };
    let Some(buf_id) = pane.buffer_id else {
        app.set_scrollbar_visible(false);
        return;
    };
    let total = ed
        .buffers
        .get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
        .unwrap_or(1);
    let visible = (pane.height as usize).max(1);
    let top = kernel::render::frame::pane_scroll_top(ed, pane, buf_id, visible);
    let (size, pos) = thumb(total, visible, top);
    app.set_scrollbar_visible(total > visible);
    app.set_scrollbar_size(size);
    app.set_scrollbar_pos(pos);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_fills_track_for_single_line() {
        assert_eq!(thumb(1, 30, 0), (1.0, 0.0));
    }

    #[test]
    fn thumb_position_spans_zero_to_one() {
        assert_eq!(thumb(101, 20, 0).1, 0.0);
        assert_eq!(thumb(101, 20, 100).1, 1.0);
        assert!((thumb(101, 20, 50).1 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn thumb_shrinks_with_longer_documents() {
        assert!(thumb(1000, 20, 0).0 < thumb(100, 20, 0).0);
    }

    #[test]
    fn drag_fraction_maps_to_line() {
        assert_eq!(top_for_fraction(101, 0.0), 0);
        assert_eq!(top_for_fraction(101, 1.0), 100);
        assert_eq!(top_for_fraction(101, 2.0), 100);
        assert_eq!(top_for_fraction(101, 0.5), 50);
    }

    #[test]
    fn caret_hidden_when_scrolled_out_of_view() {
        assert!(caret_in_view(10, 5, 20));
        assert!(!caret_in_view(4, 5, 20));
        assert!(!caret_in_view(25, 5, 20));
        assert!(caret_in_view(24, 5, 20));
    }
}
