//! Editing commands (insert, delete, undo/redo, replace, eval, etc).

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType};
use crate::kernel::state::Editor;
use crate::kernel::state::mode::EditorMode;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("insert-char", "Insert a character at cursor",
        vec![
            ArgSpec::new("char", ArgType::String),
        ],
        |editor, args| {
            let ch = args.get("char")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "char required".to_string())?;
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                view.insert(pos, ch);
            }
            Ok(())
        },
    );

    cmds.register_fn("delete-char", "Delete character at cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let ch = view.buffer.lock().unwrap().char_at(pos);
                if let Some(ch) = ch {
                    view.delete(pos, pos + ch.len_utf8());
                }
            }
            Ok(())
        },
    );

    cmds.register_fn("newline", "Insert a newline at cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                view.insert(pos, "\n");
            }
            Ok(())
        },
    );

    cmds.register_fn("backspace", "Delete character before cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                if pos > 0 {
                    let text = view.buffer.lock().unwrap().slice(0, pos);
                    if let Some(ch) = text.chars().last() {
                        let start = pos - ch.len_utf8();
                        view.delete(start, pos);
                    }
                }
            }
            Ok(())
        },
    );

    cmds.register_fn("undo", "Undo the last change",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                view.undo();
            }
            Ok(())
        },
    );

    cmds.register_fn("redo", "Redo the last undone change",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                view.redo();
            }
            Ok(())
        },
    );

    cmds.register_fn("replace-char", "Replace character under cursor (r)",
        vec![
            ArgSpec::new("char", ArgType::String),
        ],
        |editor, args| {
            let ch = args.get("char").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let old_ch = view.buffer.lock().unwrap().char_at(pos);
                if let Some(old) = old_ch {
                    let end = pos + old.len_utf8();
                    let replacement: String = if ch == '\n' { "\n".to_string() } else { ch.to_string() };
                    view.delete(pos, end);
                    view.insert(pos, &replacement);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("join-lines", "Join current line with next line (J)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let (line_count, eol, next_start) = {
                    let buf = view.buffer.lock().unwrap();
                    let lc = buf.line_count();
                    if line + 1 < lc {
                        let eol = buf.line_start_offset(line).unwrap_or(0)
                            + buf.line(line).unwrap_or_default().len();
                        let next = buf.line_start_offset(line + 1).unwrap_or(0);
                        (lc, Some(eol), Some(next))
                    } else {
                        (lc, None, None)
                    }
                };
                if let (Some(eol), Some(next_start)) = (eol, next_start) {
                    if line + 1 < line_count {
                        view.delete(eol, next_start);
                        view.insert(eol, " ");
                        view.set_cursor(eol);
                    }
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("change-to-eol", "Delete to end of line and enter insert mode (C)",
        vec![],
        |editor, _args| {
            crate::kernel::command::execute_command(editor, "delete-to-eol", &std::collections::HashMap::new())?;
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("yank-to-eol", "Yank from cursor to end of line (Y)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get(&buf_id) {
                let line = current_line(view);
                let (line_start, line_end) = {
                    let buf = view.buffer.lock().unwrap();
                    let ls = buf.line_start_offset(line).unwrap_or(0);
                    let lt = buf.line(line).unwrap_or_default();
                    (ls, ls + lt.len())
                };
                let pos = view.cursor.offset;
                if pos < line_end {
                    let yanked = view.buffer.lock().unwrap().slice(pos, line_end);
                    editor.yanked_text = Some(yanked.to_string());
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("delete-word-back", "Delete word back in insert mode (ctrl-w)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                if pos == 0 { return Ok(()); }
                let text = view.buffer.lock().unwrap().slice(0, pos);
                let trimmed = text.trim_end_matches(|c: char| !(c.is_alphanumeric() || c == '_'));
                if trimmed.is_empty() {
                    let last = text.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                    view.delete(pos - last, pos);
                    return Ok(());
                }
                let word_start = trimmed.rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|i| i + 1).unwrap_or(0);
                view.delete(word_start, pos);
            }
            Ok(())
        },
    );

    cmds.register_fn("increment-number", "Increment number under cursor (ctrl-a)",
        vec![],
        |editor, _args| { adjust_number(editor, 1) },
    );

    cmds.register_fn("decrement-number", "Decrement number under cursor (ctrl-x)",
        vec![],
        |editor, _args| { adjust_number(editor, -1) },
    );

    cmds.register_fn("toggle-case", "Toggle case of character under cursor (g~)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let ch = view.buffer.lock().unwrap().char_at(pos);
                if let Some(ch) = ch {
                    let toggled: String = ch.to_uppercase().collect::<String>()
                        .chars().next()
                        .map(|c| if c == ch { ch.to_lowercase().collect() } else { c.to_string() })
                        .unwrap_or_else(|| ch.to_string());
                    let end = pos + ch.len_utf8();
                    view.delete(pos, end);
                    view.insert(pos, &toggled);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("lowercase-region", "Lowercase a region (gu)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let ch = view.buffer.lock().unwrap().char_at(pos);
                if let Some(ch) = ch {
                    let lower = ch.to_lowercase().to_string();
                    let end = pos + ch.len_utf8();
                    view.delete(pos, end);
                    view.insert(pos, &lower);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("uppercase-region", "Uppercase a region (gU)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let ch = view.buffer.lock().unwrap().char_at(pos);
                if let Some(ch) = ch {
                    let upper = ch.to_uppercase().to_string();
                    let end = pos + ch.len_utf8();
                    view.delete(pos, end);
                    view.insert(pos, &upper);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("eval", "Evaluate a Janet expression",
        vec![
            ArgSpec::new("expr", ArgType::String),
        ],
        |editor, args| {
            let expr = args.get("expr")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "Expression required".to_string())?;
            if let Some(ref mut rt) = editor.runtime {
                rt.eval(expr);
            }
            Ok(())
        },
    );

    cmds.register_fn("eval-region", "Evaluate the visual selection as a Janet expression",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let cursor = editor.views.get(&buf_id).map(|v| v.cursor.offset).unwrap_or(0);
            match editor.selection_range(cursor) {
                None => return Err("No selection active".to_string()),
                Some((start, end)) => {
                    let text = {
                        let arc = editor.buffers.get(buf_id).ok_or("no buffer")?;
                        let buf = arc.lock().unwrap();
                        buf.slice(start, end)
                    };
                    let (value, error) = match editor.runtime.as_mut().map(|rt| rt.eval_result(&text)) {
                        Some(Ok(v)) => (v, String::new()),
                        Some(Err(e)) => (String::new(), e),
                        None => (String::new(), "Scripting runtime not available".to_string()),
                    };
                    editor.events.emit_typed(keys::events::EVAL_RESULT, EvalResultPayload {
                        value,
                        error,
                    });
                }
            }
            Ok(())
        },
    );

    cmds.register_fn("eval-buffer", "Evaluate the entire focused buffer as Janet",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let text = {
                let arc = editor.buffers.get(buf_id).ok_or("no buffer")?;
                let buf = arc.lock().unwrap();
                buf.slice(0, buf.len())
            };
            let (value, error) = match editor.runtime.as_mut().map(|rt| rt.eval_result(&text)) {
                Some(Ok(v)) => (v, String::new()),
                Some(Err(e)) => (String::new(), e),
                None => (String::new(), "Scripting runtime not available".to_string()),
            };
            editor.events.emit_typed(keys::events::EVAL_RESULT, EvalResultPayload {
                value,
                error,
            });
            Ok(())
        },
    );

}

fn adjust_number(editor: &mut Editor, delta: isize) -> CommandResult {
    let buf_id = get_current_buffer_id(editor);
    if let Some(view) = editor.views.get(&buf_id) {
        let pos = view.cursor.offset;
        let text = { let b = view.buffer.lock().unwrap(); b.slice(0, b.len()) };
        // Find start of number at/around cursor
        let before = &text[..pos];
        let num_start = before.rfind(|c: char| !c.is_ascii_digit() && c != '-').map(|i| i + 1).unwrap_or(0);
        if num_start > before.len() { return Ok(()); }
        let num_text = &text[num_start..];
        let num_len = num_text.find(|c: char| !c.is_ascii_digit()).unwrap_or(num_text.len());
        if num_len == 0 { return Ok(()); }
        let num_str = &text[num_start..num_start + num_len];
        if let Ok(n) = num_str.parse::<isize>() {
            let new_val = n + delta;
            let new_str = new_val.to_string();
            if let Some(view) = editor.views.get_mut(&buf_id) {
                view.replace(num_start, num_start + num_len, &new_str);
                view.set_cursor(num_start);
            }
        }
    }
    emit_cursor_moved(editor);
    Ok(())
}
