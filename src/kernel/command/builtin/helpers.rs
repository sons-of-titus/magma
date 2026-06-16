//! Shared helpers used by builtin command submodules.

use crate::kernel::state::Editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;
use crate::kernel::text_engine::BufferView;

pub(super) fn get_current_buffer_id(editor: &Editor) -> usize {
    editor.view_tree.focused_window()
        .and_then(|wid| editor.view_tree.buffer(wid))
        .unwrap_or(0)
}

pub(super) fn emit_cursor_moved(editor: &mut Editor) {
    let buf_id = get_current_buffer_id(editor);
    let cursor = editor.views.get(&buf_id).map(|v| v.cursor.offset.to_string());
    editor.events.emit_typed(keys::events::CURSOR_MOVED, CursorMovedPayload {
        buffer_id: buf_id.to_string(),
        cursor: cursor.unwrap_or_default(),
    });
}

pub(super) fn current_line(view: &BufferView) -> usize {
    view.cursor.line
}

pub(super) fn current_col(view: &BufferView) -> usize {
    view.cursor.column
}

pub(super) fn col_at_offset(view: &BufferView, pos: usize) -> usize {
    let buf = view.buffer.lock().unwrap();
    let text = buf.slice(0, pos);
    drop(buf);
    text.chars().rev().position(|c| c == '\n').unwrap_or_else(|| text.chars().count())
}

pub(super) fn move_to_line_col(view: &mut BufferView, line: usize, col: usize) {
    let (offset, line_len) = {
        let buf = view.buffer.lock().unwrap();
        let offset = buf.line_start_offset(line);
        let line_text = buf.line(line).unwrap_or_default();
        (offset, line_text.len())
    };
    if let Some(offset) = offset {
        view.set_cursor(offset + col.min(line_len));
    }
}

pub(super) fn move_cursor(editor: &mut Editor, delta: isize) {
    let buf_id = get_current_buffer_id(editor);
    if let Some(view) = editor.views.get_mut(&buf_id) {
        let pos = view.cursor.offset;
        let new_pos = {
            let buf = view.buffer.lock().unwrap();
            if delta < 0 {
                let text = buf.slice(0, pos);
                text.chars().last().map(|c| pos - c.len_utf8()).unwrap_or(0)
            } else {
                let text = buf.slice(pos, buf.len());
                text.chars().next().map(|c| pos + c.len_utf8()).unwrap_or(buf.len())
            }
        };
        view.set_cursor(new_pos);
    }
    emit_cursor_moved(editor);
}
