//! Editing commands (insert, delete, undo/redo, replace, eval, etc).

use crate::command::CommandResult;
use crate::command::args::{ArgSpec, ArgType};
use crate::state::Editor;
use crate::state::mode::EditorMode;
use crate::event::payload::*;
use crate::event::keys;

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
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.insert(buf.cursor(), ch);
            }
            Ok(())
        },
    );

    cmds.register_fn("delete-char", "Delete character at cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let pos = buf.cursor();
                if let Some(ch) = buf.char_at(pos) {
                    buf.delete(pos, pos + ch.len_utf8());
                }
            }
            Ok(())
        },
    );

    cmds.register_fn("newline", "Insert a newline at cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.insert(buf.cursor(), "\n");
            }
            Ok(())
        },
    );

    cmds.register_fn("backspace", "Delete character before cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let pos = buf.cursor();
                if pos > 0 {
                    let text = buf.slice(0, pos);
                    if let Some(ch) = text.chars().last() {
                        let start = pos - ch.len_utf8();
                        buf.delete(start, pos);
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
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.undo();
            }
            Ok(())
        },
    );

    cmds.register_fn("redo", "Redo the last undone change",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.redo();
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
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let pos = buf.cursor();
                let old_ch = buf.char_at(pos);
                if let Some(old) = old_ch {
                    let end = pos + old.len_utf8();
                    let replacement: String = if ch == '\n' { "\n".to_string() } else { ch.to_string() };
                    buf.delete(pos, end);
                    buf.insert(pos, &replacement);
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
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let line = current_line(buf);
                if line + 1 < buf.line_count() {
                    let eol = buf.line_start_offset(line).unwrap_or(0)
                        + buf.line(line).unwrap_or_default().len();
                    let next_start = buf.line_start_offset(line + 1).unwrap_or(0);
                    // Replace newline with space
                    buf.delete(eol, next_start);
                    buf.insert(eol, " ");
                    buf.set_cursor(eol);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("change-to-eol", "Delete to end of line and enter insert mode (C)",
        vec![],
        |editor, _args| {
            crate::command::execute_command(editor, "delete-to-eol", &std::collections::HashMap::new())?;
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
            if let Some(buf) = editor.buffers.get(buf_id) {
                let line = current_line(buf);
                let line_start = buf.line_start_offset(line).unwrap_or(0);
                let line_text = buf.line(line).unwrap_or_default();
                let line_end = line_start + line_text.len();
                let pos = buf.cursor();
                if pos < line_end {
                    let yanked = buf.slice(pos, line_end);
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
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let pos = buf.cursor();
                if pos == 0 { return Ok(()); }
                let text = buf.slice(0, pos);
                let trimmed = text.trim_end_matches(|c: char| !(c.is_alphanumeric() || c == '_'));
                if trimmed.is_empty() {
                    // Just delete one char
                    let last = text.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                    buf.delete(pos - last, pos);
                    return Ok(());
                }
                let word_start = trimmed.rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|i| i + 1).unwrap_or(0);
                buf.delete(word_start, pos);
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
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let pos = buf.cursor();
                if let Some(ch) = buf.char_at(pos) {
                    let toggled: String = ch.to_uppercase().collect::<String>()
                        .chars().next()
                        .map(|c| if c == ch { ch.to_lowercase().collect() } else { c.to_string() })
                        .unwrap_or_else(|| ch.to_string());
                    let end = pos + ch.len_utf8();
                    buf.delete(pos, end);
                    buf.insert(pos, &toggled);
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
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let pos = buf.cursor();
                if let Some(ch) = buf.char_at(pos) {
                    let lower = ch.to_lowercase().to_string();
                    let end = pos + ch.len_utf8();
                    buf.delete(pos, end);
                    buf.insert(pos, &lower);
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
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                let pos = buf.cursor();
                if let Some(ch) = buf.char_at(pos) {
                    let upper = ch.to_uppercase().to_string();
                    let end = pos + ch.len_utf8();
                    buf.delete(pos, end);
                    buf.insert(pos, &upper);
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
            let cursor = editor.buffers.get(buf_id).map(|b| b.cursor()).unwrap_or(0);
            match editor.selection_range(cursor) {
                None => return Err("No selection active".to_string()),
                Some((start, end)) => {
                    let text = {
                        let buf = editor.buffers.get(buf_id).ok_or("no buffer")?;
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
                let buf = editor.buffers.get(buf_id).ok_or("no buffer")?;
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
    if let Some(buf) = editor.buffers.get_mut(buf_id) {
        let pos = buf.cursor();
        let text = buf.slice(0, buf.len());
        // Find start of number at/around cursor
        let before = &text[..pos];
        // Scan backwards to find start of number
        let num_start = before.rfind(|c: char| !c.is_ascii_digit() && c != '-').map(|i| i + 1).unwrap_or(0);
        if num_start > before.len() { return Ok(()); }
        // Scan forwards to find end of number
        let num_text = &text[num_start..];
        let num_len = num_text.find(|c: char| !c.is_ascii_digit()).unwrap_or(num_text.len());
        if num_len == 0 { return Ok(()); }
        let num_str = &text[num_start..num_start + num_len];
        if let Ok(n) = num_str.parse::<isize>() {
            let new_val = n + delta;
            let new_str = new_val.to_string();
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.replace(num_start, num_start + num_len, &new_str);
                buf.set_cursor(num_start);
            }
        }
    }
    emit_cursor_moved(editor);
    Ok(())
}
