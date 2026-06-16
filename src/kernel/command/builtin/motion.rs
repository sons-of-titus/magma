//! Motion commands: word/find/paragraph/sentence/mark/buffer navigation.

use crate::kernel::command::args::{ArgSpec, ArgType};
use crate::kernel::state::Editor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("move-word-forward", "Move cursor to start of next word",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let (new_pos, buf_len) = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(pos, buf.len());
                    let word_start = text.find(|c: char| c.is_alphanumeric() || c == '_');
                    let np = if let Some(ws) = word_start {
                        let after = &text[ws..];
                        let word_end = after.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(after.len());
                        let after_word = &after[word_end..];
                        let next_start = after_word.find(|c: char| c.is_alphanumeric() || c == '_').unwrap_or(after_word.len());
                        pos + ws + word_end + next_start
                    } else {
                        buf.len()
                    };
                    (np, buf.len())
                };
                view.set_cursor(new_pos.min(buf_len));
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("move-word-back", "Move cursor to start of current/previous word",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                if pos == 0 { return Ok(()); }
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(0, pos);
                    let trimmed = text.trim_end_matches(|c: char| !(c.is_alphanumeric() || c == '_'));
                    if trimmed.is_empty() { return Ok(()); }
                    trimmed.rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .map(|i| i + 1)
                        .unwrap_or(0)
                };
                view.set_cursor(new_pos);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("move-word-end", "Move cursor to end of current/next word",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(pos, buf.len());
                    let word_start = text.find(|c: char| c.is_alphanumeric() || c == '_');
                    if let Some(ws) = word_start {
                        let after = &text[ws..];
                        let word_end = after.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(after.len());
                        pos + ws + word_end
                    } else {
                        pos
                    }
                };
                view.set_cursor(new_pos);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("find-forward", "Find character forward",
        vec![
            ArgSpec::new("char", ArgType::String),
        ],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let ch = args.get("char").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(pos, buf.len());
                    text.find(ch).map(|offset| pos + offset)
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            editor.last_find = Some((ch, true, false));
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("find-backward", "Find character backward (F)",
        vec![
            ArgSpec::new("char", ArgType::String),
        ],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let ch = args.get("char").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(0, pos);
                    text.rfind(ch)
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            editor.last_find = Some((ch, false, false));
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("find-till-forward", "Find character forward, stop before (t)",
        vec![
            ArgSpec::new("char", ArgType::String),
        ],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let ch = args.get("char").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(pos, buf.len());
                    if let Some(offset) = text.find(ch) {
                        if offset > 0 { Some(pos + offset - 1) } else { None }
                    } else { None }
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            editor.last_find = Some((ch, true, true));
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("find-till-backward", "Find character backward, stop after (T)",
        vec![
            ArgSpec::new("char", ArgType::String),
        ],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let ch = args.get("char").and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(0, pos);
                    text.rfind(ch).and_then(|offset| {
                        let next_char = offset + ch.len_utf8();
                        if next_char < pos { Some(next_char) } else { None }
                    })
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            editor.last_find = Some((ch, false, true));
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("repeat-last-find", "Repeat last f/t/F/T search (;)",
        vec![],
        |editor, _args| {
            if let Some((ch, forward, till)) = editor.last_find {
                let buf_id = get_current_buffer_id(editor);
                if editor.views.contains_key(&buf_id) {
                    let cmd = if forward {
                        if till { "find-till-forward" } else { "find-forward" }
                    } else {
                        if till { "find-till-backward" } else { "find-backward" }
                    };
                    let mut args = std::collections::HashMap::new();
                    args.insert("char".to_string(), crate::kernel::command::args::ArgValue::String(ch.to_string()));
                    let _ = crate::kernel::command::execute_command(editor, cmd, &args);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("reverse-last-find", "Repeat last f/t/F/T search reversed (,)",
        vec![],
        |editor, _args| {
            if let Some((ch, forward, till)) = editor.last_find {
                let buf_id = get_current_buffer_id(editor);
                if editor.views.contains_key(&buf_id) {
                    let cmd = if forward {
                        if till { "find-till-backward" } else { "find-backward" }
                    } else {
                        if till { "find-till-forward" } else { "find-forward" }
                    };
                    let mut args = std::collections::HashMap::new();
                    args.insert("char".to_string(), crate::kernel::command::args::ArgValue::String(ch.to_string()));
                    let _ = crate::kernel::command::execute_command(editor, cmd, &args);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("goto-matching-brace", "Jump to matching bracket (%)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(0, buf.len());
                    let ch = text[pos..].chars().next()
                        .or_else(|| text[..pos].chars().next_back());
                    if let Some(c) = ch {
                        let pairs = [('(', ')'), ('[', ']'), ('{', '}')];
                        let mut found = None;
                        for &(open, close) in &pairs {
                            if c == open {
                                let remaining = &text[pos..];
                                let mut depth = 0;
                                for (i, ch) in remaining.char_indices() {
                                    if ch == open { depth += 1; }
                                    if ch == close { depth -= 1; }
                                    if depth == 0 {
                                        found = Some(pos + i);
                                        break;
                                    }
                                }
                            } else if c == close {
                                let before = &text[..=pos];
                                let mut depth = 0;
                                for (i, ch) in before.char_indices().rev() {
                                    if ch == close { depth += 1; }
                                    if ch == open { depth -= 1; }
                                    if depth == 0 {
                                        found = Some(i);
                                        break;
                                    }
                                }
                            }
                        }
                        found
                    } else { None }
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("move-paragraph-forward", "Move cursor to next paragraph (})",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(pos, buf.len());
                    let lines: Vec<&str> = text.split('\n').collect();
                    let mut found_empty = false;
                    let mut result = None;
                    for (i, line) in lines.iter().enumerate() {
                        if line.trim().is_empty() {
                            found_empty = true;
                        } else if found_empty {
                            let offset: usize = lines[..i].iter().map(|l| l.len() + 1).sum();
                            result = Some(pos + offset);
                            break;
                        }
                    }
                    result
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("move-paragraph-back", "Move cursor to previous paragraph ({)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(0, pos);
                    let lines: Vec<&str> = text.split('\n').collect();
                    let mut found_content = false;
                    let mut result = None;
                    for (i, line) in lines.iter().enumerate().rev() {
                        if line.trim().is_empty() && found_content {
                            let offset: usize = lines[..=i].iter().map(|l| l.len() + 1).sum();
                            result = Some(offset.max(1) - 1);
                            break;
                        } else if !line.trim().is_empty() {
                            found_content = true;
                        }
                    }
                    result
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("move-sentence-forward", "Move cursor to next sentence ())",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(pos, buf.len());
                    text.find(['.', '!', '?']).map(|idx| {
                        let after = &text[idx..];
                        let end = after.find(|c: char| c != '.' && c != '!' && c != '?' && c != ' ' && c != '\t')
                            .unwrap_or(after.len());
                        pos + idx + end
                    })
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("move-sentence-back", "Move cursor to previous sentence (()",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.views.get_mut(&buf_id) {
                let pos = view.cursor.offset;
                if pos == 0 { return Ok(()); }
                let new_pos = {
                    let buf = view.buffer.lock().unwrap();
                    let text = buf.slice(0, pos);
                    let mut result = None;
                    let mut i = pos;
                    while i > 0 {
                        let c = text[i.saturating_sub(1)..].chars().next().unwrap_or(' ');
                        i -= c.len_utf8();
                        let check = text[i..].chars().next().unwrap_or(' ');
                        if check == '.' || check == '!' || check == '?' {
                            let mut j = i + check.len_utf8();
                            while j < pos {
                                let c2 = text[j..].chars().next().unwrap_or(' ');
                                if c2 != ' ' && c2 != '\t' && c2 != '.' && c2 != '!' && c2 != '?' {
                                    break;
                                }
                                j += c2.len_utf8();
                            }
                            result = Some(j);
                            break;
                        }
                    }
                    result
                };
                if let Some(np) = new_pos { view.set_cursor(np); }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

}
