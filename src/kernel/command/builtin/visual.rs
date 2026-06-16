//! Visual mode and multi-cursor commands.

use std::collections::HashMap;

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;
use crate::kernel::state::ExtraCursor;
use crate::kernel::state::mode::{EditorMode, Selection};

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("enter-visual-mode", "Enter visual (character) selection mode",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let anchor = editor.buffers.get(buf_id).map(|b| b.cursor()).unwrap_or(0);
            editor.editor_mode = EditorMode::new("visual", false);
            editor.selection = Some(Selection { anchor, kind: "char".into() });
            editor.keymaps.push_layer("visual");
            Ok(())
        },
    );

    cmds.register_fn("exit-visual-mode", "Exit visual mode",
        vec![],
        |editor, _args| {
            save_last_visual(editor);
            editor.editor_mode = EditorMode::new("normal", false);
            editor.selection = None;
            editor.keymaps.pop_layer("visual");
            Ok(())
        },
    );

    cmds.register_fn("enter-visual-line-mode", "Enter visual line selection mode (V)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let line = editor.buffers.get(buf_id).map(|b| {
                let pos = b.cursor();
                let text = b.slice(0, pos);
                text.chars().filter(|&c| c == '\n').count()
            }).unwrap_or(0);
            editor.editor_mode = EditorMode::new("visual", false);
            editor.selection = Some(Selection { anchor: line, kind: "line".into() });
            editor.keymaps.push_layer("visual");
            Ok(())
        },
    );

    cmds.register_fn("enter-visual-block-mode", "Enter visual block selection mode",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let anchor = editor.buffers.get(buf_id).map(|b| b.cursor()).unwrap_or(0);
            editor.editor_mode = EditorMode::new("visual", false);
            editor.selection = Some(Selection { anchor, kind: "block".into() });
            editor.keymaps.push_layer("visual");
            Ok(())
        },
    );

    cmds.register_fn("delete-selection", "Delete visually selected text",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(crate::kernel::state::mode::Selection { anchor, .. }) = editor.selection
                && let Some(buf) = editor.buffers.get_mut(buf_id) {
                    let cursor = buf.cursor();
                    let start = anchor.min(cursor);
                    let end_char_len = buf.char_at(anchor.max(cursor))
                        .map(|c| c.len_utf8()).unwrap_or(1);
                    let end = anchor.max(cursor) + end_char_len;
                    buf.set_cursor(start);
                    buf.delete(start, end);
                }
            save_last_visual(editor);
            editor.editor_mode = EditorMode::new("normal", false);
            editor.selection = None;
            editor.keymaps.pop_layer("visual");
            Ok(())
        },
    );

    cmds.register_fn("change-selection", "Delete selection and enter insert mode",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(crate::kernel::state::mode::Selection { anchor, .. }) = editor.selection
                && let Some(buf) = editor.buffers.get_mut(buf_id) {
                    let cursor = buf.cursor();
                    let start = anchor.min(cursor);
                    let end_char_len = buf.char_at(anchor.max(cursor))
                        .map(|c| c.len_utf8()).unwrap_or(1);
                    let end = anchor.max(cursor) + end_char_len;
                    buf.set_cursor(start);
                    buf.delete(start, end);
                }
            save_last_visual(editor);
            editor.editor_mode = EditorMode::new("insert", true);
            editor.selection = None;
            editor.keymaps.pop_layer("visual");
            editor.keymaps.pop_layer("vim");   // visual was stacked on vim; both must go
            editor.keymaps.push_layer("insert");
            Ok(())
        },
    );

    cmds.register_fn("yank-selection", "Yank visual selection into register",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(crate::kernel::state::mode::Selection { anchor, .. }) = editor.selection
                && let Some(buf) = editor.buffers.get(buf_id) {
                    let cursor = buf.cursor();
                    let start = anchor.min(cursor);
                    let end_char_len = buf.char_at(anchor.max(cursor))
                        .map(|c| c.len_utf8()).unwrap_or(1);
                    let end = anchor.max(cursor) + end_char_len;
                    let yanked = buf.slice(start, end).to_string();
                    editor.yanked_text = Some(yanked.clone());
                    editor.clipboard.set_text(&yanked);
                }
            save_last_visual(editor);
            editor.editor_mode = EditorMode::new("normal", false);
            editor.selection = None;
            editor.keymaps.pop_layer("visual");
            editor.keymaps.push_layer("vim");
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("toggle-case-visual", "Toggle case of visual selection (~ in visual)",
        vec![],
        |editor, _args| {
            apply_to_selection(editor, |s| {
                s.chars().map(|c| {
                    if c.is_uppercase() { c.to_lowercase().next().unwrap_or(c) }
                    else { c.to_uppercase().next().unwrap_or(c) }
                }).collect()
            });
            exit_visual_mode_helper(editor);
            Ok(())
        },
    );

    cmds.register_fn("lowercase-visual", "Lowercase visual selection (u in visual)",
        vec![],
        |editor, _args| {
            apply_to_selection(editor, |s| s.to_lowercase());
            exit_visual_mode_helper(editor);
            Ok(())
        },
    );

    cmds.register_fn("uppercase-visual", "Uppercase visual selection (U in visual)",
        vec![],
        |editor, _args| {
            apply_to_selection(editor, |s| s.to_uppercase());
            exit_visual_mode_helper(editor);
            Ok(())
        },
    );

    cmds.register_fn("block-insert", "Visual block: enter insert mode at block start",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let (start_line, _end_line, insert_col, lines) = {
                let Some(buf) = editor.buffers.get(buf_id) else { return Ok(()); };
                if editor.selection.is_none() { return Ok(()); }
                let cursor = buf.cursor();
                let anchor = editor.selection.as_ref().map(|s| s.anchor).unwrap_or(0);
                let text = buf.slice(0, buf.len());
                let s_line = text[..anchor.min(cursor)].chars().filter(|&c| c == '\n').count();
                let e_line = text[..anchor.max(cursor)].chars().filter(|&c| c == '\n').count();
                let s_col = col_at(buf, anchor.min(cursor));
                let e_col = col_at(buf, anchor.max(cursor));
                let ins_col = s_col.min(e_col);
                let mut lns = Vec::new();
                for line in s_line..=e_line {
                    if let Some(off) = buf.line_start_offset(line) {
                        lns.push(off);
                    }
                }
                if lns.is_empty() { return Ok(()); }
                (s_line, e_line, ins_col, lns)
            };
            let pre_pos = lines[0] + insert_col;
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let line_text = buf.line(start_line).unwrap_or_default();
                let pos = lines[0] + insert_col.min(line_text.len());
                buf.set_cursor(pos);
            }
            editor.block_visual.active = true;
            editor.block_visual.lines = lines;
            editor.block_visual.col = insert_col;
            editor.block_visual.was_append = false;
            editor.block_visual.pre_pos = pre_pos;
            save_last_visual(editor);
            editor.keymaps.pop_layer("visual");
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            Ok(())
        },
    );

    cmds.register_fn("block-append", "Visual block: enter insert mode at block end",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let (start_line, _end_line, insert_col, lines) = {
                let Some(buf) = editor.buffers.get(buf_id) else { return Ok(()); };
                if editor.selection.is_none() { return Ok(()); }
                let cursor = buf.cursor();
                let anchor = editor.selection.as_ref().map(|s| s.anchor).unwrap_or(0);
                let text = buf.slice(0, buf.len());
                let s_line = text[..anchor.min(cursor)].chars().filter(|&c| c == '\n').count();
                let e_line = text[..anchor.max(cursor)].chars().filter(|&c| c == '\n').count();
                let s_col = col_at(buf, anchor.min(cursor));
                let e_col = col_at(buf, anchor.max(cursor));
                let ins_col = s_col.max(e_col) + 1;
                let mut lns = Vec::new();
                for line in s_line..=e_line {
                    if let Some(off) = buf.line_start_offset(line) {
                        lns.push(off);
                    }
                }
                if lns.is_empty() { return Ok(()); }
                (s_line, e_line, ins_col, lns)
            };
            let pre_pos = lines[0] + insert_col;
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let line_text = buf.line(start_line).unwrap_or_default();
                let pos = lines[0] + insert_col.min(line_text.len());
                buf.set_cursor(pos);
            }
            editor.block_visual.active = true;
            editor.block_visual.lines = lines;
            editor.block_visual.col = insert_col;
            editor.block_visual.was_append = true;
            editor.block_visual.pre_pos = pre_pos;
            save_last_visual(editor);
            editor.keymaps.pop_layer("visual");
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            Ok(())
        },
    );

    cmds.register_fn("reselect-last-visual", "Reselect last visual selection (gv)",
        vec![],
        |editor, _args| {
            if let Some((anchor, end, is_line)) = editor.last_visual {
                let buf_id = get_current_buffer_id(editor);
                if let Some(buf) = editor.buffers.get_mut(buf_id) {
                    buf.set_cursor(end);
                    editor.keymaps.pop_layer("vim");
                    editor.editor_mode = EditorMode::new("visual", false);
                    editor.selection = if is_line {
                        Some(Selection { anchor, kind: "line".into() })
                    } else {
                        Some(Selection { anchor, kind: "char".into() })
                    };
                    editor.keymaps.push_layer("visual");
                }
            }
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

/// Helper: Apply a transformation to the current visual selection.
pub(super) fn apply_to_selection(editor: &mut Editor, f: impl Fn(&str) -> String) {
    let buf_id = get_current_buffer_id(editor);
    if let Some(buf) = editor.buffers.get(buf_id) {
        let cursor = buf.cursor();
        if let Some((s, e)) = editor.selection_range(cursor) {
            let text = buf.slice(s, e).to_string();
            let new_text = f(&text);
            let _ = buf;
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.replace(s, e, &new_text);
                buf.set_cursor(s);
            }
        }
    }
}

pub(super) fn exit_visual_mode_helper(editor: &mut Editor) {
    save_last_visual(editor);
    editor.keymaps.pop_layer("visual");
    editor.editor_mode = EditorMode::new("normal", false);
    editor.selection = None;
    editor.keymaps.push_layer("vim");
}

pub(super) fn save_last_visual(editor: &mut Editor) {
    let buf_id = get_current_buffer_id(editor);
    if let Some(buf) = editor.buffers.get(buf_id) {
        let cursor = buf.cursor();
        let (anchor, is_line) = match &editor.selection {
            Some(crate::kernel::state::mode::Selection { anchor, kind }) if kind == "line" => (*anchor, true),
            Some(crate::kernel::state::mode::Selection { anchor, .. }) => (*anchor, false),
            _ => return,
        };
        editor.last_visual = Some((anchor, cursor, is_line));
    }
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
