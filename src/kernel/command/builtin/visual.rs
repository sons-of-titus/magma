//! Selection operations and multi-cursor primitives.
//!
//! There is no visual mode in the shipped editor: selections come from the
//! mouse or shift+movement, and these commands act on whatever is selected.
//! Modal selection editing is community-extension territory.

use std::collections::HashMap;

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;
use crate::kernel::state::ExtraCursor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("delete-selection", "Delete the selected text",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(crate::kernel::state::mode::Selection { anchor, .. }) = editor.selection {
                if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                    let cursor = view.cursor_offset();
                    let (start, end) = {
                        let buf = view.buffer.lock().unwrap();
                        let end_char_len = buf.char_at(anchor.max(cursor))
                            .map(|c| c.len_utf8()).unwrap_or(1);
                        (anchor.min(cursor), anchor.max(cursor) + end_char_len)
                    };
                    view.set_cursor(start);
                    view.delete(start, end);
                }
            }
            editor.selection = None;
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("yank-selection", "Copy the selected text to the clipboard and yank register",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(crate::kernel::state::mode::Selection { anchor, .. }) = editor.selection {
                if let Some(view) = editor.view_of_buffer(buf_id) {
                    let cursor = view.cursor_offset();
                    let yanked = {
                        let buf = view.buffer.lock().unwrap();
                        let end_char_len = buf.char_at(anchor.max(cursor))
                            .map(|c| c.len_utf8()).unwrap_or(1);
                        let start = anchor.min(cursor);
                        let end = anchor.max(cursor) + end_char_len;
                        buf.slice(start, end).to_string()
                    };
                    editor.yanked_text = Some(yanked.clone());
                    editor.clipboard.set_text(&yanked);
                }
            }
            editor.selection = None;
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("cut-selection", "Cut the selected text to the clipboard and yank register",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let cut = match editor.selection {
                Some(crate::kernel::state::mode::Selection { anchor, .. }) => {
                    let cursor = editor.view_of_buffer(buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
                    let out = editor.view_of_buffer(buf_id).map(|view| {
                        let buf = view.buffer.lock().unwrap();
                        let end_char_len = buf.char_at(anchor.max(cursor))
                            .map(|c| c.len_utf8()).unwrap_or(1);
                        let start = anchor.min(cursor);
                        let end = anchor.max(cursor) + end_char_len;
                        (start, end, buf.slice(start, end).to_string())
                    });
                    out
                }
                None => None,
            };
            if let Some((start, end, yanked)) = cut {
                editor.yanked_text = Some(yanked.clone());
                editor.clipboard.set_text(&yanked);
                if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                    view.set_cursor(start);
                    view.delete(start, end);
                }
            }
            editor.selection = None;
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    // ── Multi-cursor primitives (high-level logic in Janet) ───────────────

    cmds.register_fn("cursor-add",
        "Add an extra cursor at a byte position",
        vec![ArgSpec::new("pos", ArgType::Integer)],
        cursor_add_impl,
    );

    cmds.register_fn("cursor-clear",
        "Clear all extra cursors",
        vec![],
        cursor_clear_impl,
    );
}

fn cursor_add_impl(editor: &mut Editor, args: &HashMap<String, ArgValue>) -> CommandResult {
    let pos = args.get("pos").and_then(|a| a.as_integer())
        .ok_or_else(|| "pos required".to_string())? as usize;
    editor.multi_cursor.extra_cursors.push(ExtraCursor { pos, anchor: None });
    editor.multi_cursor.active = true;
    emit_cursor_moved(editor);
    Ok(())
}

fn cursor_clear_impl(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    editor.multi_cursor.extra_cursors.clear();
    editor.multi_cursor.active = false;
    emit_cursor_moved(editor);
    Ok(())
}
