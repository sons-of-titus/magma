//! Shared helpers used by builtin command submodules.

use crate::event::keys;
use crate::event::payload::*;
use crate::state::Editor;
use crate::text_engine::BufferView;

pub(super) fn get_current_buffer_id(editor: &Editor) -> usize {
    editor
        .view_tree
        .focused_window()
        .and_then(|wid| editor.view_tree.buffer(wid))
        .unwrap_or(0)
}

pub(super) fn emit_cursor_moved(editor: &mut Editor) {
    let buf_id = get_current_buffer_id(editor);
    // Navigation that lands inside a closed fold opens it — the caret must
    // never sit on an invisible row.
    if let Some(view) = editor.view_of_buffer_mut(buf_id) {
        let offset = view.cursor.offset;
        let inside: Vec<(usize, usize)> = view
            .folds
            .iter()
            .filter(|&&(s, e)| s <= offset && offset < e)
            .copied()
            .collect();
        if !inside.is_empty() {
            for (s, e) in inside {
                view.remove_fold(s, e);
            }
            editor.frame_dirty = true;
        }
    }
    let cursor = editor
        .view_of_buffer(buf_id)
        .map(|v| v.cursor.offset.to_string());
    editor.events.emit_typed(
        keys::events::CURSOR_MOVED,
        CursorMovedPayload {
            buffer_id: buf_id.to_string(),
            cursor: cursor.unwrap_or_default(),
        },
    );
}

pub(super) fn current_line(view: &BufferView) -> usize {
    view.cursor.line
}

pub(super) fn current_col(view: &BufferView) -> usize {
    view.cursor.column
}

pub(super) fn move_to_line_col(view: &mut BufferView, line: usize, col: usize) {
    let target = {
        let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
        // `col` is a char count (the cursor's logical column); the byte
        // target is the char-th boundary into the line, clamped to the
        // line's end.  Mixing the two shifted the cursor left on every
        // multibyte target line and could land mid-char.
        buf.line_start_offset(line).map(|start| {
            let line_text = buf.line(line).unwrap_or_default();
            let byte_col = line_text
                .char_indices()
                .nth(col)
                .map(|(i, _)| i)
                .unwrap_or(line_text.len());
            start + byte_col
        })
    };
    if let Some(offset) = target {
        view.set_cursor(offset);
    }
}

pub(super) fn move_cursor(editor: &mut Editor, delta: isize) {
    let buf_id = get_current_buffer_id(editor);
    if let Some(view) = editor.view_of_buffer_mut(buf_id) {
        let pos = view.cursor.offset;
        // Grapheme stepping over a bounded window: one small rope slice per
        // keypress (clusters longer than the window degrade to char steps),
        // never a document-sized copy — and the cursor never lands inside
        // a combining cluster the renderer treats as one cell.
        const GRAPHEME_WINDOW: usize = 64;
        let new_pos = {
            let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
            let len = buf.len();
            if delta < 0 {
                if pos == 0 {
                    0
                } else {
                    let from = pos.saturating_sub(GRAPHEME_WINDOW);
                    let window = buf.slice(from, pos);
                    from + crate::text_engine::text_layout::prev_grapheme(&window, window.len())
                }
            } else if pos >= len {
                len
            } else {
                let to = (pos + GRAPHEME_WINDOW).min(len);
                let window = buf.slice(pos, to);
                pos + crate::text_engine::text_layout::next_grapheme(&window, 0)
            }
        };
        view.set_cursor(new_pos);
    }
    emit_cursor_moved(editor);
}

/// The word-character class shared by motions and word search.
pub(super) fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
