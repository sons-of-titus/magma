//! Simple cursor movement / navigation commands.

use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("cursor-left", "Move cursor left",
        vec![],
        |editor, _args| {
            move_cursor(editor, -1);
            Ok(())
        },
    );

    cmds.register_fn("cursor-right", "Move cursor right",
        vec![],
        |editor, _args| {
            move_cursor(editor, 1);
            Ok(())
        },
    );

    cmds.register_fn("cursor-up", "Move cursor up",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                if line > 0 {
                    let col = current_col(view);
                    move_to_line_col(view, line - 1, col);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("cursor-down", "Move cursor down",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let count = view.buffer.lock().unwrap().line_count();
                if line + 1 < count {
                    let col = current_col(view);
                    move_to_line_col(view, line + 1, col);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("line-start", "Move cursor to start of current line",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let offset = view.buffer.lock().unwrap().line_start_offset(line);
                if let Some(offset) = offset {
                    view.set_cursor(offset);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("line-end", "Move cursor to end of current line",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let target = {
                    let buf = view.buffer.lock().unwrap();
                    let start = buf.line_start_offset(line).unwrap_or(0);
                    let text_len = buf.line(line).unwrap_or_default().len();
                    start + text_len
                };
                view.set_cursor(target);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("move-to-first-char", "Move to first non-whitespace on line",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let target = {
                    let buf = view.buffer.lock().unwrap();
                    let start = buf.line_start_offset(line).unwrap_or(0);
                    let text = buf.line(line).unwrap_or_default();
                    let indent = text.len() - text.trim_start().len();
                    start + indent
                };
                view.set_cursor(target);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-buffer-start", "Move cursor to first line",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                view.set_cursor(0);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-buffer-end", "Move cursor to last line",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let len = view.buffer.lock().unwrap().len();
                view.set_cursor(len);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-line", "Go to a specific line (1-indexed). Defaults to last line.",
        vec![
            ArgSpec::optional("line", ArgType::Integer, ArgValue::Integer(-1)),
        ],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let target_offset = {
                    let buf = view.buffer.lock().unwrap();
                    let line_count = buf.line_count();
                    let line = args.get("line")
                        .and_then(|a| a.as_integer())
                        .map(|n| n as usize)
                        .filter(|&n| n > 0)
                        .map(|n| n - 1)
                        .unwrap_or(line_count.saturating_sub(1));
                    let target = line.min(line_count.saturating_sub(1));
                    buf.line_start_offset(target)
                };
                if let Some(offset) = target_offset {
                    view.set_cursor(offset);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-screen-top", "Move cursor to top of screen (H)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let target = line.saturating_sub(20);
                let offset = view.buffer.lock().unwrap().line_start_offset(target);
                if let Some(offset) = offset {
                    view.set_cursor(offset);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-screen-middle", "Move cursor to middle of screen (M)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let target = line.saturating_sub(10);
                let offset = view.buffer.lock().unwrap().line_start_offset(target);
                if let Some(offset) = offset {
                    view.set_cursor(offset);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-screen-bottom", "Move cursor to bottom of screen (L)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let len = view.buffer.lock().unwrap().len();
                view.set_cursor(len);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("scroll-to-top", "Move cursor near screen top (zt)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let target = line.saturating_sub(20);
                let offset = view.buffer.lock().unwrap().line_start_offset(target);
                if let Some(offset) = offset {
                    view.set_cursor(offset);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("scroll-to-middle", "Move cursor near screen middle (zz)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let target = line.saturating_sub(10);
                let offset = view.buffer.lock().unwrap().line_start_offset(target);
                if let Some(offset) = offset {
                    view.set_cursor(offset);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("scroll-to-bottom", "Move cursor near screen bottom (zb)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let offset = view.buffer.lock().unwrap().line_start_offset(line);
                if let Some(offset) = offset {
                    view.set_cursor(offset);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-older-change", "Jump to older change in change list (g;)",
        vec![],
        |editor, _args| {
            if editor.change_list.is_empty() { return Ok(()); }
            if editor.change_list_idx > 0 {
                editor.change_list_idx -= 1;
            }
            let pos = editor.change_list[editor.change_list_idx];
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let len = view.buffer.lock().unwrap().len();
                view.set_cursor(pos.min(len));
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-newer-change", "Jump to newer change in change list (g,)",
        vec![],
        |editor, _args| {
            if editor.change_list.is_empty() { return Ok(()); }
            if editor.change_list_idx < editor.change_list.len().saturating_sub(1) {
                editor.change_list_idx += 1;
            }
            let pos = editor.change_list[editor.change_list_idx];
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let len = view.buffer.lock().unwrap().len();
                view.set_cursor(pos.min(len));
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("go-to-last-insert-pos", "Go to last insert position (gi)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let last_insert_pos = editor.last_insert_pos;
            if let Some(view) = editor.views.get_mut(&buf_id) {
                view.set_cursor(last_insert_pos);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );
}
