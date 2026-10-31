//! Indent and insert-mode line commands.
//!
//! TAB runs per keystroke, so these operate on the rope directly.
//! `delete-to-eol` lives here natively so `change-to-eol` does not
//! round-trip through a Janet command.

use std::collections::HashMap;

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::state::Editor;

use super::helpers::{emit_cursor_moved, get_current_buffer_id};

const INDENT: &str = "    ";

fn count_arg(args: &HashMap<String, ArgValue>) -> usize {
    args.get("count")
        .and_then(|a| a.as_integer())
        .filter(|&c| c > 0)
        .map(|c| c as usize)
        .unwrap_or(1)
}

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "indent-line",
        "Indent the current line (TAB)",
        vec![ArgSpec::optional(
            "count",
            ArgType::Integer,
            ArgValue::Integer(1),
        )],
        |editor, args| {
            let count = count_arg(args);
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let line = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    buf.slice(0, view.cursor_offset()).matches('\n').count()
                };
                for _ in 0..count {
                    let Some(start) = view
                        .buffer
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .line_start_offset(line)
                    else {
                        break;
                    };
                    view.insert(start, INDENT);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "deindent-line",
        "Remove up to one indent step from the start of the current line (SHIFT-TAB)",
        vec![ArgSpec::optional(
            "count",
            ArgType::Integer,
            ArgValue::Integer(1),
        )],
        |editor, args| {
            let count = count_arg(args);
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let line = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    buf.slice(0, view.cursor_offset()).matches('\n').count()
                };
                for _ in 0..count {
                    let to_remove = {
                        let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                        let Some(start) = buf.line_start_offset(line) else {
                            break;
                        };
                        let probe = buf.slice(start, (start + 4).min(buf.len()));
                        probe
                            .chars()
                            .take_while(|&c| c == ' ' || c == '\t')
                            .map(char::len_utf8)
                            .sum::<usize>()
                    };
                    if to_remove == 0 {
                        break;
                    }
                    let start = view
                        .buffer
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .line_start_offset(line)
                        .unwrap_or(0);
                    view.delete(start, start + to_remove);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "delete-to-bol",
        "Delete from the start of the line to the cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let (pos, start) = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    let pos = view.cursor_offset();
                    let line = buf.slice(0, pos).matches('\n').count();
                    (pos, buf.line_start_offset(line))
                };
                if let Some(start) = start
                    && start < pos
                {
                    view.delete(start, pos);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "delete-to-eol",
        "Delete from the cursor to the end of the line",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let (pos, end) = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    let pos = view.cursor_offset();
                    let line = buf.slice(0, pos).matches('\n').count();
                    // Stop before the newline: deleting through it would join
                    // the next line (change-to-eol must not do that).
                    let end = match buf.line_start_offset(line + 1) {
                        Some(next) => next - 1,
                        None => buf.len(),
                    };
                    (pos, end)
                };
                if pos < end {
                    view.delete(pos, end);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );
}
