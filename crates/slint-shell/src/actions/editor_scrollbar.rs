//! Scrollbar drag: moves the pane's viewport, never the cursor.

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

use crate::sync::scrollbar::top_for_fraction;
use crate::{AppState, AppWindow, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    let weak = app.as_weak();
    app.on_scrollbar_dragged(move |fraction| {
        let mut s = state.borrow_mut();
        let ed = &mut s.editor;
        let Some(pane_id) = ed.view_tree.focused_window() else {
            return;
        };
        let Some(buf_id) = ed.view_tree.window(pane_id).and_then(|p| p.buffer_id) else {
            return;
        };
        let total = ed
            .buffers
            .get(buf_id)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).line_count())
            .unwrap_or(1);
        let cursor = ed.view_of_buffer(buf_id).map(|v| v.cursor.offset);
        if let Some(pane) = ed.view_tree.pane_mut(pane_id) {
            pane.scroll_offset = top_for_fraction(total, fraction);
            pane.scroll_pinned = true;
            pane.scroll_buf = Some(buf_id);
            pane.scroll_cursor = cursor;
        }
        ed.frame_dirty = true;
        refresh(&weak, &mut s);
    });
}
