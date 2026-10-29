//! Viewport scroll commands.
//!
//! Line scrolls move the cursor (sticky view-follow does the rest); page and
//! half-page scrolls move the viewport with the cursor riding along, never
//! leaving the window.  Same semantics as the wheel path in `input/mouse.rs`:
//! an explicit scroll pins the offset until the cursor moves.

use crate::state::Editor;

use super::helpers::emit_cursor_moved;

fn window_step(editor: &Editor, page: bool) -> i64 {
    let height = editor
        .view_tree
        .focused_window()
        .and_then(|wid| editor.view_tree.window(wid))
        .map(|w| w.height as i64)
        .unwrap_or(24);
    if page {
        (height - 2).max(1)
    } else {
        (height / 2).max(1)
    }
}

fn scroll_by_lines(editor: &mut Editor, n: i64) {
    let Some(buf_id) = editor
        .view_tree
        .focused_window()
        .and_then(|wid| editor.view_tree.window(wid))
        .and_then(|w| w.buffer_id)
    else {
        return;
    };
    let (max_line, target) = {
        let Some(arc) = editor.buffers.get(buf_id) else {
            return;
        };
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let cur_off = editor
            .view_of_buffer(buf_id)
            .map(|v| v.cursor_offset())
            .unwrap_or(0);
        let cur_line = buf.slice(0, cur_off).matches('\n').count();
        let max_line = buf.line_count().saturating_sub(1) as i64;
        let target = (cur_line as i64 + n).clamp(0, max_line);
        (max_line, buf.line_start_offset(target as usize).map(|off| (target, off)))
    };
    let (height, old_top) = editor
        .view_tree
        .focused_window()
        .and_then(|wid| editor.view_tree.window(wid))
        .map(|w| (w.height as i64, w.scroll_offset as i64))
        .unwrap_or((24, 0));
    let visible = (height - 1).max(1);
    let mut top = (old_top + n).clamp(0, max_line);
    if let Some((t, _)) = target {
        let target_line = t as i64;
        if target_line < top {
            top = target_line;
        }
        if target_line >= top + visible {
            top = (target_line - visible).max(0);
        }
    }
    if let Some((_, off)) = target
        && let Some(view) = editor.view_of_buffer_mut(buf_id)
    {
        view.set_cursor(off);
    }
    let cursor = editor.view_of_buffer(buf_id).map(|v| v.cursor_offset());
    if let Some(pane) = editor.view_tree.focused_window_mut() {
        pane.scroll_offset = top.max(0) as usize;
        pane.scroll_pinned = true;
        pane.scroll_buf = Some(buf_id);
        pane.scroll_cursor = cursor;
    }
    emit_cursor_moved(editor);
}

fn scroll_by_columns(editor: &mut Editor, n: i64) {
    if let Some(pane) = editor.view_tree.focused_window_mut() {
        pane.scroll_left = (pane.scroll_left as i64 + n).max(0) as usize;
        pane.scroll_left_pinned = true;
    }
}

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "scroll-line-down",
        "Scroll the view down one line",
        vec![],
        |editor, _args| {
            scroll_by_lines(editor, 1);
            Ok(())
        },
    );
    cmds.register_fn(
        "scroll-line-up",
        "Scroll the view up one line",
        vec![],
        |editor, _args| {
            scroll_by_lines(editor, -1);
            Ok(())
        },
    );
    cmds.register_fn(
        "scroll-half-page-down",
        "Scroll the view down half a page",
        vec![],
        |editor, _args| {
            scroll_by_lines(editor, window_step(editor, false));
            Ok(())
        },
    );
    cmds.register_fn(
        "scroll-half-page-up",
        "Scroll the view up half a page",
        vec![],
        |editor, _args| {
            scroll_by_lines(editor, -window_step(editor, false));
            Ok(())
        },
    );
    cmds.register_fn(
        "scroll-page-down",
        "Scroll the view down a page",
        vec![],
        |editor, _args| {
            scroll_by_lines(editor, window_step(editor, true));
            Ok(())
        },
    );
    cmds.register_fn(
        "scroll-page-up",
        "Scroll the view up a page",
        vec![],
        |editor, _args| {
            scroll_by_lines(editor, -window_step(editor, true));
            Ok(())
        },
    );
    cmds.register_fn(
        "scroll-column-left",
        "Pan the view left by 4 columns",
        vec![],
        |editor, _args| {
            scroll_by_columns(editor, -4);
            Ok(())
        },
    );
    cmds.register_fn(
        "scroll-column-right",
        "Pan the view right by 4 columns",
        vec![],
        |editor, _args| {
            scroll_by_columns(editor, 4);
            Ok(())
        },
    );
}
