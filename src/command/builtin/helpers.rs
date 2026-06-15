//! Shared helpers used by builtin command submodules.

use crate::state::Editor;

pub(super) fn get_current_buffer_id(editor: &Editor) -> usize {
    editor.windows.focused_window()
        .and_then(|wid| editor.windows.buffer(wid))
        .unwrap_or(0)
}

pub(super) fn emit_cursor_moved(editor: &mut Editor) {
    let buf_id = get_current_buffer_id(editor);
    let mut data = std::collections::HashMap::new();
    data.insert("buffer-id".to_string(), buf_id.to_string());
    if let Some(buf) = editor.buffers.get(buf_id) {
        data.insert("cursor".to_string(), buf.cursor().to_string());
    }
    editor.events.emit("cursor-moved", data);
}

pub(super) fn current_line(buf: &crate::buffer::Buffer) -> usize {
    let pos = buf.cursor();
    let text = buf.slice(0, pos);
    text.chars().filter(|&c| c == '\n').count()
}

pub(super) fn current_col(buf: &crate::buffer::Buffer) -> usize {
    col_at(buf, buf.cursor())
}

pub(super) fn col_at(buf: &crate::buffer::Buffer, pos: usize) -> usize {
    let text = buf.slice(0, pos);
    text.chars().rev().position(|c| c == '\n').unwrap_or_else(|| text.chars().count())
}

pub(super) fn move_to_line_col(buf: &mut crate::buffer::Buffer, line: usize, col: usize) {
    if let Some(offset) = buf.line_start_offset(line) {
        let line_text = buf.line(line).unwrap_or_default();
        let max_col = line_text.len();
        buf.set_cursor(offset + col.min(max_col));
    }
}

pub(super) fn move_cursor(editor: &mut Editor, delta: isize) {
    let buf_id = get_current_buffer_id(editor);
    if let Some(buf) = editor.buffers.get_mut(buf_id) {
        let pos = buf.cursor();
        let new_pos = if delta < 0 {
            let text = buf.slice(0, pos);
            text.chars().last().map(|c| pos - c.len_utf8()).unwrap_or(0)
        } else {
            let text = buf.slice(pos, buf.len());
            text.chars().next().map(|c| pos + c.len_utf8()).unwrap_or(buf.len())
        };
        buf.set_cursor(new_pos);
    }
    emit_cursor_moved(editor);
}
