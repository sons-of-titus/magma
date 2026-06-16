//! Register, yank, put, and macro commands.

use crate::kernel::command::args::{ArgSpec, ArgType};
use crate::kernel::input::dispatch_key;
use crate::kernel::state::Editor;
use crate::kernel::state::mode::EditorMode;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("delete-line", "Delete the current line",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let line = current_line(view);
                let (start, eol, deleted) = {
                    let buf = view.buffer.lock().unwrap();
                    let start = buf.line_start_offset(line).unwrap_or(0);
                    let line_text = buf.line(line).unwrap_or_default();
                    let end = start + line_text.len();
                    let eol = if end < buf.len() { end + 1 } else { end };
                    let deleted = buf.slice(start, eol).to_string();
                    (start, eol, deleted)
                };
                editor.yanked_text = Some(deleted.clone());
                editor.clipboard.set_text(&deleted);
                view.set_cursor(start);
                view.delete(start, eol);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("change-line", "Delete the current line and enter insert mode",
        vec![],
        |editor, _args| {
            crate::kernel::command::execute_command(editor, "delete-line", &std::collections::HashMap::new())?;
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode::new("insert", true);
            editor.keymaps.push_layer("insert");
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("yank-line", "Copy the current line into the yank register",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get(&buf_id) {
                let line = current_line(view);
                let yanked = {
                    let buf = view.buffer.lock().unwrap();
                    let start = buf.line_start_offset(line).unwrap_or(0);
                    let line_text = buf.line(line).unwrap_or_default();
                    let end = start + line_text.len();
                    let eol = if end < buf.len() { end + 1 } else { end };
                    buf.slice(start, eol).to_string()
                };
                editor.yanked_text = Some(yanked.clone());
                editor.clipboard.set_text(&yanked);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("put", "Paste yanked/deleted text after cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let text = editor.yanked_text.clone()
                .or_else(|| editor.clipboard.get_text());
            if let Some(content) = text {
                if let Some(view) = editor.views.get_mut(&buf_id) {
                    let pos = view.cursor_offset();
                    let paste_pos = {
                        let buf = view.buffer.lock().unwrap();
                        let after = buf.slice(pos, buf.len());
                        after.chars().next().map(|c| pos + c.len_utf8()).unwrap_or(pos)
                    };
                    view.insert(paste_pos, &content);
                    view.set_cursor(paste_pos);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("put-before", "Paste yanked/deleted text before cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let text = editor.yanked_text.clone()
                .or_else(|| editor.clipboard.get_text());
            if let Some(content) = text {
                if let Some(view) = editor.views.get_mut(&buf_id) {
                    let pos = view.cursor_offset();
                    view.insert(pos, &content);
                    view.set_cursor(pos);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("put-after-cursor", "Paste, cursor after pasted text (gp)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let text = editor.yanked_text.clone()
                .or_else(|| editor.clipboard.get_text())
                .unwrap_or_default();
            if text.is_empty() { return Ok(()); }
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor_offset();
                view.insert(pos, &text);
                view.set_cursor(pos + text.len());
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("put-before-cursor", "Paste before, cursor after pasted text (gP)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let text = editor.yanked_text.clone()
                .or_else(|| editor.clipboard.get_text())
                .unwrap_or_default();
            if text.is_empty() { return Ok(()); }
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor_offset();
                view.insert(pos, &text);
                view.set_cursor(pos + text.len());
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("insert-register", "Insert register content in insert mode (ctrl-r)",
        vec![
            ArgSpec::new("register", ArgType::String),
        ],
        |editor, args| {
            let reg = args.get("register").and_then(|a| a.as_string())
                .unwrap_or("\"");
            let text = if reg == "\"" || reg == "0" {
                editor.yanked_text.clone()
            } else {
                editor.registers.get(reg).cloned()
            };
            if let Some(content) = text {
                let buf_id = get_current_buffer_id(editor);
                if let Some(view) = editor.views.get_mut(&buf_id) {
                    let pos = view.cursor_offset();
                    view.insert(pos, &content);
                }
            }
            Ok(())
        },
    );

    cmds.register_fn("start-macro-recording", "Start recording macro into register (q<char>)",
        vec![
            ArgSpec::new("register", ArgType::String),
        ],
        |editor, args| {
            let reg = args.get("register").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "register required".to_string())?;
            editor.recording_macro = Some(reg.to_string());
            editor.macros.entry(reg.to_string()).or_insert_with(Vec::new);
            Ok(())
        },
    );

    cmds.register_fn("stop-macro-recording", "Stop recording macro (q while recording)",
        vec![],
        |editor, _args| {
            editor.recording_macro = None;
            Ok(())
        },
    );

    // vim-macro-toggle: used by the char-capture callback for "q".
    // If not recording, starts recording into the char/register arg.
    // If already recording, stops.
    cmds.register_fn("vim-macro-toggle", "Toggle macro recording (char-capture callback for q)",
        vec![
            ArgSpec::new("char", ArgType::String),
        ],
        |editor, args| {
            if editor.recording_macro.is_some() {
                editor.recording_macro = None;
            } else {
                let reg = args.get("char").or_else(|| args.get("register"))
                    .and_then(|a| a.as_string())
                    .and_then(|s| s.chars().next())
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                if !reg.is_empty() {
                    editor.macros.remove(&reg);
                    editor.recording_macro = Some(reg);
                }
            }
            Ok(())
        },
    );

    cmds.register_fn("play-macro", "Play back macro from register (@<char>)",
        vec![
            ArgSpec::new("register", ArgType::String),
        ],
        |editor, args| {
            let reg = args.get("register").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "register required".to_string())?;
            let keys = editor.macros.get(&reg.to_string()).cloned()
                .unwrap_or_default();
            for key in &keys {
                dispatch_key(editor, key);
            }
            Ok(())
        },
    );

    cmds.register_fn("set-mark", "Set a mark at cursor position (m<char>)",
        vec![
            crate::kernel::command::args::ArgSpec::new("char", crate::kernel::command::args::ArgType::String),
        ],
        |editor, args| {
            let ch = args.get("char").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "mark char required".to_string())?;
            let buf_id = get_current_buffer_id(editor);
            let pos = editor.views.get(&buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
            editor.marks.insert(ch, pos);
            Ok(())
        },
    );

    cmds.register_fn("jump-to-mark", "Jump to mark line ('<char>)",
        vec![
            crate::kernel::command::args::ArgSpec::new("char", crate::kernel::command::args::ArgType::String),
        ],
        |editor, args| {
            let ch = args.get("char").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "mark char required".to_string())?;
            let buf_id = get_current_buffer_id(editor);
            if let Some(&mark_pos) = editor.marks.get(&ch) {
                if let Some(view) = editor.views.get_mut(&buf_id) {
                    let offset = {
                        let buf = view.buffer.lock().unwrap();
                        let text = buf.slice(0, mark_pos);
                        let line = text.chars().filter(|&c| c == '\n').count();
                        buf.line_start_offset(line).unwrap_or(mark_pos)
                    };
                    view.set_cursor(offset);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("jump-to-mark-char", "Jump to mark exact position (`<char>)",
        vec![
            crate::kernel::command::args::ArgSpec::new("char", crate::kernel::command::args::ArgType::String),
        ],
        |editor, args| {
            let ch = args.get("char").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "mark char required".to_string())?;
            let buf_id = get_current_buffer_id(editor);
            if let Some(&mark_pos) = editor.marks.get(&ch) {
                if let Some(view) = editor.views.get_mut(&buf_id) {
                    view.set_cursor(mark_pos);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );
}
