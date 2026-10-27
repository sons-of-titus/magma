//! Motion commands: word/find/paragraph/sentence/mark/buffer navigation.

use crate::command::args::{ArgSpec, ArgType};
use crate::state::Editor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "move-word-forward",
        "Move cursor to start of next word",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let (new_pos, buf_len) = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    let np = match buf.find_char_from(pos, is_word_char) {
                        Some(ws) => {
                            let we = buf
                                .find_char_from(ws, |c| !is_word_char(c))
                                .unwrap_or(buf.len());
                            buf.find_char_from(we, is_word_char).unwrap_or(buf.len())
                        }
                        None => buf.len(),
                    };
                    (np, buf.len())
                };
                view.set_cursor(new_pos.min(buf_len));
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "move-word-back",
        "Move cursor to start of current/previous word",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                if pos == 0 {
                    return Ok(());
                }
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    let Some(last_word) = buf.rfind_char_before(pos, is_word_char) else {
                        return Ok(());
                    };
                    buf.rfind_char_before(last_word + 1, |c| !is_word_char(c))
                        .map(|i| i + 1)
                        .unwrap_or(0)
                };
                view.set_cursor(new_pos);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "move-word-end",
        "Move cursor to end of current/next word",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    buf.find_char_from(pos, is_word_char)
                        .and_then(|ws| buf.find_char_from(ws, |c| !is_word_char(c)))
                        .unwrap_or(pos)
                };
                view.set_cursor(new_pos);
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "find-forward",
        "Find character forward",
        vec![ArgSpec::new("char", ArgType::String)],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let ch = args
                .get("char")
                .and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    buf.find_char_from(pos, |c| c == ch)
                };
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            editor.editing.last_find = Some((ch, true, false));
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "find-backward",
        "Find character backward (F)",
        vec![ArgSpec::new("char", ArgType::String)],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let ch = args
                .get("char")
                .and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    buf.rfind_char_before(pos, |c| c == ch)
                };
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            editor.editing.last_find = Some((ch, false, false));
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "find-till-forward",
        "Find character forward, stop before (t)",
        vec![ArgSpec::new("char", ArgType::String)],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let ch = args
                .get("char")
                .and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    // Stop one char before the target: back up over the
                    // preceding char (which may be multibyte).
                    buf.find_char_from(pos.saturating_sub(0), |c| c == ch)
                        .filter(|&at| at > pos)
                        .and_then(|at| buf.rfind_char_before(at, |_| true))
                };
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            editor.editing.last_find = Some((ch, true, true));
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "find-till-backward",
        "Find character backward, stop after (T)",
        vec![ArgSpec::new("char", ArgType::String)],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let ch = args
                .get("char")
                .and_then(|a| a.as_string())
                .and_then(|s| s.chars().next())
                .ok_or_else(|| "char required".to_string())?;
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    // Stop one char after the target (multibyte-aware).
                    buf.rfind_char_before(pos, |c| c == ch)
                        .map(|at| at + ch.len_utf8())
                        .filter(|next| *next < pos)
                };
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            editor.editing.last_find = Some((ch, false, true));
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "repeat-last-find",
        "Repeat last f/t/F/T search (;)",
        vec![],
        |editor, _args| {
            if let Some((ch, forward, till)) = editor.editing.last_find {
                let buf_id = get_current_buffer_id(editor);
                if editor.views.contains_key(&buf_id) {
                    let cmd = if forward {
                        if till {
                            "find-till-forward"
                        } else {
                            "find-forward"
                        }
                    } else {
                        if till {
                            "find-till-backward"
                        } else {
                            "find-backward"
                        }
                    };
                    let mut args = std::collections::HashMap::new();
                    args.insert(
                        "char".to_string(),
                        crate::command::args::ArgValue::String(ch.to_string()),
                    );
                    let _ = crate::command::execute_command(editor, cmd, &args);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "reverse-last-find",
        "Repeat last f/t/F/T search reversed (,)",
        vec![],
        |editor, _args| {
            if let Some((ch, forward, till)) = editor.editing.last_find {
                let buf_id = get_current_buffer_id(editor);
                if editor.views.contains_key(&buf_id) {
                    let cmd = if forward {
                        if till {
                            "find-till-backward"
                        } else {
                            "find-backward"
                        }
                    } else {
                        if till {
                            "find-till-forward"
                        } else {
                            "find-forward"
                        }
                    };
                    let mut args = std::collections::HashMap::new();
                    args.insert(
                        "char".to_string(),
                        crate::command::args::ArgValue::String(ch.to_string()),
                    );
                    let _ = crate::command::execute_command(editor, cmd, &args);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "goto-matching-brace",
        "Jump to matching bracket (%)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    // The cursor can sit on a non-boundary byte after a
                    // UTF-16 → byte conversion; snap down first.
                    let mut pos = pos.min(buf.len());
                    while pos > 0 && buf.char_at(pos).is_none() {
                        pos -= 1;
                    }
                    // (byte index, char) of the bracket candidate at/behind
                    // the cursor — a chunk-fed scan, no document copy.
                    let at = buf.char_at(pos).map(|c| (pos, c)).or_else(|| {
                        pos.checked_sub(1)
                            .and_then(|p| buf.char_at(p).map(|c| (p, c)))
                    });
                    if let Some((at, c)) = at {
                        let pairs = [('(', ')'), ('[', ']'), ('{', '}')];
                        let mut found = None;
                        for &(open, close) in &pairs {
                            if c == open {
                                let mut depth = 0;
                                for (i, ch) in buf.char_span(at, buf.len()) {
                                    if ch == open {
                                        depth += 1;
                                    }
                                    if ch == close {
                                        depth -= 1;
                                    }
                                    if depth == 0 {
                                        found = Some(i);
                                        break;
                                    }
                                }
                            } else if c == close {
                                // A stack of open positions: the target's
                                // match pops when we reach the close at `at`.
                                let mut stack = Vec::new();
                                for (i, ch) in buf.char_span(0, at + c.len_utf8()) {
                                    if ch == open {
                                        stack.push(i);
                                    } else if ch == close {
                                        let popped = stack.pop();
                                        if i == at {
                                            found = popped;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        found
                    } else {
                        None
                    }
                };
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "move-paragraph-forward",
        "Move cursor to next paragraph (})",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
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
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "move-paragraph-back",
        "Move cursor to previous paragraph ({)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    // Rope walk: one line fetched at a time, no document copy.
                    let cursor_line = buf.line_col_bytes(pos).0;
                    let mut found_content = false;
                    let mut result = None;
                    for k in (0..=cursor_line).rev() {
                        let line_text = buf.line(k).unwrap_or_default();
                        if line_text.trim().is_empty() {
                            if found_content {
                                let s = if k == cursor_line {
                                    pos
                                } else {
                                    buf.line_start_offset(k + 1).unwrap_or(pos)
                                };
                                result = Some(s.max(1) - 1);
                                break;
                            }
                        } else {
                            found_content = true;
                        }
                    }
                    result
                };
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "move-sentence-forward",
        "Move cursor to next sentence ())",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    // Rope scans: find the terminator, then skip the run of
                    // terminators/whitespace after it.
                    buf.find_char_from(pos, |c| c == '.' || c == '!' || c == '?')
                        .map(|term| {
                            buf.find_char_from(term, |c| {
                                c != '.' && c != '!' && c != '?' && c != ' ' && c != '\t'
                            })
                            .unwrap_or(buf.len())
                        })
                };
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn(
        "move-sentence-back",
        "Move cursor to previous sentence (()",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                let pos = view.cursor.offset;
                if pos == 0 {
                    return Ok(());
                }
                let new_pos = {
                    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                    // Rope scans: the closest sentence terminator before the
                    // cursor, then past its trailing punctuation/space run.
                    buf.rfind_char_before(pos, |c| c == '.' || c == '!' || c == '?')
                        .map(|term| {
                            buf.find_char_from(term, |c| {
                                c != '.' && c != '!' && c != '?' && c != ' ' && c != '\t'
                            })
                            .unwrap_or(pos)
                        })
                };
                if let Some(np) = new_pos {
                    view.set_cursor(np);
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );
}
