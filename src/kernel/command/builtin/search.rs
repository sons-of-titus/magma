//! Search commands (/, ?, n, N, *, #).

use crate::kernel::command::CommandResult;
use crate::kernel::state::Editor;
use crate::kernel::state::mode::{EditorMode, Minibuffer};

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("enter-search-forward", "Start forward search prompt (/)",
        vec![],
        |editor, _args| {
            editor.keymaps.pop_layer("visual");
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode {
                name: "search".into(),
                accepts_text: false,
                minibuffer: Some(Minibuffer { prompt: "/".into(), input: String::new() }),
            };
            editor.plugin_state.insert("vim.search-direction".to_string(), "forward".to_string());
            editor.keymaps.push_layer("search");
            Ok(())
        },
    );

    cmds.register_fn("enter-search-backward", "Start backward search prompt (?)",
        vec![],
        |editor, _args| {
            editor.keymaps.pop_layer("visual");
            editor.keymaps.pop_layer("vim");
            editor.editor_mode = EditorMode {
                name: "search".into(),
                accepts_text: false,
                minibuffer: Some(Minibuffer { prompt: "?".into(), input: String::new() }),
            };
            editor.plugin_state.insert("vim.search-direction".to_string(), "backward".to_string());
            editor.keymaps.push_layer("search");
            Ok(())
        },
    );

    cmds.register_fn("search-backspace", "Delete last char of search input",
        vec![],
        |editor, _args| {
            let should_exit = if let Some(ref mut mb) = editor.editor_mode.minibuffer {
                let last_len = mb.input.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                let new_len = mb.input.len().saturating_sub(last_len);
                mb.input.truncate(new_len);
                mb.input.is_empty()
            } else {
                false
            };
        if should_exit {
            editor.editor_mode = EditorMode::new("normal", false);
            editor.keymaps.pop_layer("search");
            editor.keymaps.push_layer("vim");
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.clear_highlights();
            }
        }
        Ok(())
        },
    );

    cmds.register_fn("search-cancel", "Cancel search input",
        vec![],
        |editor, _args| {
            editor.editor_mode = EditorMode::new("normal", false);
            editor.keymaps.pop_layer("search");
            editor.keymaps.push_layer("vim");
            // Clear search highlights
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.clear_highlights();
            }
            Ok(())
        },
    );

    cmds.register_fn("search-execute", "Execute the buffered search",
        vec![],
        |editor, _args| {
            let (input, direction) = if let Some(ref mb) = editor.editor_mode.minibuffer {
                let dir = match editor.plugin_state.get("vim.search-direction").map(|s| s.as_str()) {
                    Some("forward") => crate::kernel::state::mode::SearchDirection::Forward,
                    _ => crate::kernel::state::mode::SearchDirection::Backward,
                };
                (mb.input.clone(), dir)
            } else {
                return Ok(());
            };
            editor.editor_mode = EditorMode::new("normal", false);
            editor.keymaps.pop_layer("search");
            editor.keymaps.push_layer("vim");
            let pattern = input.trim().to_string();
            if pattern.is_empty() { return Ok(()); }
            editor.search_pattern = Some(pattern.clone());
            editor.search_forward = direction == crate::kernel::state::mode::SearchDirection::Forward;
            execute_search(editor, &pattern, editor.search_forward)
        },
    );

    cmds.register_fn("repeat-search", "Repeat last search in same direction (n)",
        vec![],
        |editor, _args| {
            if let Some(ref pattern) = editor.search_pattern.clone() {
                execute_search(editor, pattern, editor.search_forward)
            } else {
                Ok(())
            }
        },
    );

    cmds.register_fn("reverse-search", "Repeat last search in opposite direction (N)",
        vec![],
        |editor, _args| {
            if let Some(ref pattern) = editor.search_pattern.clone() {
                execute_search(editor, pattern, !editor.search_forward)
            } else {
                Ok(())
            }
        },
    );

    cmds.register_fn("find-word-under-cursor", "Search forward for word under cursor (*)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get(buf_id) {
                let pos = buf.cursor();
                let text = buf.slice(0, buf.len());
                let before = &text[..pos];
                let word_start = before.rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|i| i + 1).unwrap_or(0);
                let word_slice = &text[word_start..];
                let word_end = word_slice.find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .unwrap_or(word_slice.len());
                let word = &text[word_start..word_start + word_end];
                if !word.is_empty() {
                    let search_from = pos + 1;
                    let search_text = &text[search_from..];
                    if let Some(idx) = search_text.find(word)
                        && let Some(buf) = editor.buffers.get_mut(buf_id) {
                            buf.set_cursor(search_from + idx);
                        }
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("find-word-under-cursor-back", "Search backward for word under cursor (#)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get(buf_id) {
                let pos = buf.cursor();
                let text = buf.slice(0, buf.len());
                let before = &text[..pos];
                let word_start = before.rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .map(|i| i + 1).unwrap_or(0);
                let word_slice = &text[word_start..];
                let word_end = word_slice.find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .unwrap_or(word_slice.len());
                let word = &text[word_start..word_start + word_end];
                if !word.is_empty() {
                    let search_text = &text[..pos.saturating_sub(1)];
                    if let Some(idx) = search_text.rfind(word)
                        && let Some(buf) = editor.buffers.get_mut(buf_id) {
                            buf.set_cursor(idx);
                        }
                }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("keyword-context", "Show all lines containing word under cursor ([I)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get(buf_id) {
                let pos = buf.cursor();
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
    if let Some(buf) = editor.buffers.get(buf_id) {
        let text = buf.slice(0, buf.len());
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
        if let Some(buf) = editor.buffers.get_mut(buf_id) {
            buf.set_highlights(ranges);
        }
    }
}

fn execute_search(editor: &mut Editor, pattern: &str, forward: bool) -> CommandResult {
    let buf_id = get_current_buffer_id(editor);
    if let Some(buf) = editor.buffers.get(buf_id) {
        let pos = buf.cursor();
        let text = buf.slice(0, buf.len());
        if text.is_empty() || pattern.is_empty() {
            // Clear highlights on empty pattern
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.clear_highlights();
            }
            return Ok(());
        }
        // Highlight all matches
        highlight_all_matches(editor, buf_id, pattern);
        if forward {
            let start = (pos + 1).min(text.len());
            let search_from = &text[start..];
            if let Some(idx) = search_from.find(pattern) {
                if let Some(buf) = editor.buffers.get_mut(buf_id) {
                    buf.set_cursor(start + idx);
                }
                emit_cursor_moved(editor);
                return Ok(());
            }
            if let Some(idx) = text[..start.saturating_sub(1)].find(pattern) {
                if let Some(buf) = editor.buffers.get_mut(buf_id) {
                    buf.set_cursor(idx);
                }
                emit_cursor_moved(editor);
                return Ok(());
            }
        } else {
            let search_text = &text[..pos];
            if let Some(idx) = search_text.rfind(pattern) {
                if let Some(buf) = editor.buffers.get_mut(buf_id) {
                    buf.set_cursor(idx);
                }
                emit_cursor_moved(editor);
                return Ok(());
            }
            if let Some(idx) = text[pos..].rfind(pattern) {
                if let Some(buf) = editor.buffers.get_mut(buf_id) {
                    buf.set_cursor(pos + idx);
                }
                emit_cursor_moved(editor);
                return Ok(());
            }
        }
    }
    Ok(())
}
