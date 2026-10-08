//! Search commands — n/N repetition and word-under-cursor jumps.
//!
//! There is no modal `/` prompt in the shipped editor: search lives in the
//! GUI dialog (ctrl-shift-f), which sets `search_pattern`, making these
//! repeat commands work against its last query.  Community keymaps can bind
//! them to any chord.

use crate::kernel::command::CommandResult;
use crate::kernel::state::Editor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("repeat-search", "Jump to the next match of the last search (n)",
        vec![],
        |editor, _args| {
            if let Some(ref pattern) = editor.search_pattern.clone() {
                execute_search(editor, pattern, editor.search_forward)
            } else {
                Ok(())
            }
        },
    );

    cmds.register_fn("reverse-search", "Jump to the previous match of the last search (N)",
        vec![],
        |editor, _args| {
            if let Some(ref pattern) = editor.search_pattern.clone() {
                execute_search(editor, pattern, !editor.search_forward)
            } else {
                Ok(())
            }
        },
    );

    cmds.register_fn("find-word-under-cursor", "Jump to the next occurrence of the word under the cursor (*)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let (pos, text, word_start, word_end) = {
                let Some(view) = editor.view_of_buffer(buf_id) else { return Ok(()); };
                let pos = view.cursor_offset();
                let buf = view.buffer.lock().unwrap();
                let text = buf.slice(0, buf.len());
                let before = text[..pos].to_string();
                let word_start = before.rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|i| i + 1).unwrap_or(0);
                let word_slice = &text[word_start..];
                let word_end = word_slice.find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .unwrap_or(word_slice.len());
                (pos, text, word_start, word_start + word_end)
            };
            if word_start < word_end {
                let word = text[word_start..word_end].to_string();
                if !word.is_empty() {
                    editor.search_pattern = Some(word.clone());
                    editor.search_forward = true;
                    let search_from = pos + 1;
                    let found = if search_from < text.len() {
                        text[search_from..].find(&word[..]).map(|idx| search_from + idx)
                    } else {
                        None
                    };
                    if let Some(new_pos) = found {
                        if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                            view.set_cursor(new_pos);
                        }
                    }
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("find-word-under-cursor-back", "Jump to the previous occurrence of the word under the cursor (#)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let (pos, text, word_start, word_end) = {
                let Some(view) = editor.view_of_buffer(buf_id) else { return Ok(()); };
                let pos = view.cursor_offset();
                let buf = view.buffer.lock().unwrap();
                let text = buf.slice(0, buf.len());
                let before = text[..pos].to_string();
                let word_start = before.rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|i| i + 1).unwrap_or(0);
                let word_slice = &text[word_start..];
                let word_end = word_slice.find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .unwrap_or(word_slice.len());
                (pos, text, word_start, word_start + word_end)
            };
            if word_start < word_end {
                let word = text[word_start..word_end].to_string();
                if !word.is_empty() {
                    editor.search_pattern = Some(word.clone());
                    editor.search_forward = false;
                    let search_text = &text[..pos.saturating_sub(1)];
                    let found = search_text.rfind(&word[..]);
                    if let Some(idx) = found {
                        if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                            view.set_cursor(idx);
                        }
                    }
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("keyword-context", "Show all lines containing the word under the cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(view) = editor.view_of_buffer(buf_id) {
                let pos = view.cursor_offset();
                let buf = view.buffer.lock().unwrap();
                let text = buf.slice(0, buf.len());
                let before = &text[..pos];
                let word_start = before.rfind(|c: char| !c.is_alphanumeric() && c != '_')
                    .map(|i| i + 1).unwrap_or(0);
                let after = &text[pos..];
                let word_len = after.find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(after.len());
                if word_len == 0 { return Ok(()); }
                let word = &text[word_start..word_start + word_len];
                let mut line_num = 1;
                let mut line_start = 0;
                let mut matches = Vec::new();
                for (i, c) in text.char_indices() {
                    if c == '\n' {
                        let line_text = &text[line_start..i];
                        if line_text.contains(word) {
                            matches.push(format!("{:>4}: {}", line_num, line_text));
                        }
                        line_num += 1;
                        line_start = i + 1;
                    }
                }
                if line_start < text.len() {
                    let line_text = &text[line_start..];
                    if line_text.contains(word) {
                        matches.push(format!("{:>4}: {}", line_num, line_text));
                    }
                }
                if matches.is_empty() {
                    debug!("No match: {}", word);
                } else {
                    for m in &matches {
                        debug!("{}", m);
                    }
                }
            }
            Ok(())
        },
    );
}

fn highlight_all_matches(editor: &mut Editor, buf_id: usize, pattern: &str) {
    let text = {
        let Some(view) = editor.view_of_buffer(buf_id) else { return; };
        let buf = view.buffer.lock().unwrap();
        buf.slice(0, buf.len())
    };
    if text.is_empty() || pattern.is_empty() {
        return;
    }
    let mut ranges = Vec::new();
    let mut offset = 0usize;
    while let Some(idx) = text[offset..].find(pattern) {
        let start = offset + idx;
        let end = start + pattern.len();
        ranges.push((start, end, "search".to_string()));
        offset = end;
    }
    if let Some(view) = editor.view_of_buffer_mut(buf_id) {
        view.set_highlights(ranges);
    }
}

fn execute_search(editor: &mut Editor, pattern: &str, forward: bool) -> CommandResult {
    let buf_id = get_current_buffer_id(editor);
    let (pos, text) = {
        let Some(view) = editor.view_of_buffer(buf_id) else { return Ok(()); };
        let pos = view.cursor_offset();
        let buf = view.buffer.lock().unwrap();
        let text = buf.slice(0, buf.len());
        (pos, text)
    };
    if text.is_empty() || pattern.is_empty() {
        if let Some(view) = editor.view_of_buffer_mut(buf_id) {
            view.clear_highlights();
        }
        return Ok(());
    }
    // Highlight all matches
    highlight_all_matches(editor, buf_id, pattern);
    if forward {
        let start = (pos + 1).min(text.len());
        if let Some(idx) = text[start..].find(pattern) {
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                view.set_cursor(start + idx);
            }
            emit_cursor_moved(editor);
            return Ok(());
        }
        if let Some(idx) = text[..start.saturating_sub(1)].find(pattern) {
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                view.set_cursor(idx);
            }
            emit_cursor_moved(editor);
            return Ok(());
        }
    } else {
        if let Some(idx) = text[..pos].rfind(pattern) {
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                view.set_cursor(idx);
            }
            emit_cursor_moved(editor);
            return Ok(());
        }
        if let Some(idx) = text[pos..].rfind(pattern) {
            if let Some(view) = editor.view_of_buffer_mut(buf_id) {
                view.set_cursor(pos + idx);
            }
            emit_cursor_moved(editor);
            return Ok(());
        }
    }
    Ok(())
}
