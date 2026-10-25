//! Search commands — n/N repetition and word-under-cursor jumps.
//!
//! There is no modal `/` prompt in the shipped editor: search lives in the
//! GUI dialog (ctrl-f), which sets `editor.search.pattern`, making these repeat
//! commands work against its last query.  Community keymaps can bind
//! them to any chord.
//!
//! All matching runs over rope chunk scans — no document-sized copies
//! per keypress.

use crate::kernel::command::CommandResult;
use crate::kernel::state::Editor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("repeat-search", "Jump to the next match of the last search (n)",
        vec![],
        |editor, _args| {
            if let Some(ref pattern) = editor.search.pattern.clone() {
                execute_search(editor, pattern, editor.search.forward)
            } else {
                Ok(())
            }
        },
    );

    cmds.register_fn("reverse-search", "Jump to the previous match of the last search (N)",
        vec![],
        |editor, _args| {
            if let Some(ref pattern) = editor.search.pattern.clone() {
                execute_search(editor, pattern, !editor.search.forward)
            } else {
                Ok(())
            }
        },
    );

    cmds.register_fn("find-word-under-cursor", "Jump to the next occurrence of the word under the cursor (*)",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let found = word_and_next(editor, buf_id, true);
            if let Some((word, next)) = found {
                editor.search.pattern = Some(word);
                editor.search.forward = true;
                if let Some(np) = next
                    && let Some(view) = editor.view_of_buffer_mut(buf_id) {
                        view.set_cursor(np);
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
            let found = word_and_next(editor, buf_id, false);
            if let Some((word, prev)) = found {
                editor.search.pattern = Some(word);
                editor.search.forward = false;
                if let Some(np) = prev
                    && let Some(view) = editor.view_of_buffer_mut(buf_id) {
                        view.set_cursor(np);
                    }
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );

    cmds.register_fn("keyword-context", "List every line containing the word under the cursor in the Find in Files panel",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let Some(view) = editor.view_of_buffer(buf_id) else { return Ok(()) };
            let (word, entries) = {
                let pos = view.cursor_offset();
                let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                let word_start = buf.rfind_char_before(pos, |c| !is_word_char(c))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let word_end = buf.find_char_from(word_start, |c| !is_word_char(c))
                    .unwrap_or(buf.len());
                if word_start >= word_end {
                    return Ok(());
                }
                let word = buf.slice(word_start, word_end);
                let location = buf.path.clone().unwrap_or_else(|| buf.name.clone());
                let mut entries = Vec::new();
                let mut line = 0usize;
                while line < buf.line_count() {
                    if let Some(span) = buf.line_span(line) {
                        let text = buf.slice(span.start, span.end);
                        if text.contains(&word) {
                            // Column of the first match in grapheme-agnostic
                            // bytes; quickfix navigation is line-based.
                            let col = text.find(&word).map(|i| i + 1).unwrap_or(1);
                            entries.push(crate::kernel::task::QuickfixEntry {
                                filename: location.clone(),
                                line: line + 1,
                                col,
                                message: text,
                                severity: crate::kernel::semantic::provider::DiagnosticSeverity::Info,
                            });
                        }
                    }
                    line += 1;
                }
                (word, entries)
            };
            let count = entries.len();
            editor.search.set_results(entries);
            if count == 0 {
                editor.show_message(crate::kernel::state::MessageKind::Info,
                    format!("no other matches for “{word}”"));
            } else {
                editor.show_message(crate::kernel::state::MessageKind::Info,
                    format!("{count} match(es) for “{word}”"));
            }
            Ok(())
        },
    );
}

/// The word around the cursor plus the next/previous occurrence's offset.
fn word_and_next(editor: &Editor, buf_id: usize, forward: bool) -> Option<(String, Option<usize>)> {
    let view = editor.view_of_buffer(buf_id)?;
    let pos = view.cursor_offset();
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    let word_start = buf.rfind_char_before(pos, |c| !is_word_char(c))
        .map(|i| i + 1)
        .unwrap_or(0);
    let word_end = buf.find_char_from(word_start, |c| !is_word_char(c))
        .unwrap_or(buf.len());
    if word_start >= word_end {
        return None;
    }
    let word = buf.slice(word_start, word_end);
    let occurrence = if forward {
        buf.find_sub_from(pos + 1, &word)
    } else {
        // The word itself starts at word_start; look for an earlier one.
        buf.rfind_sub_before(pos.saturating_sub(1), &word)
            .filter(|&at| at < word_start || at + word.len() <= word_start)
    };
    Some((word, occurrence))
}

fn highlight_all_matches(editor: &mut Editor, buf_id: usize, pattern: &str) {
    let ranges = {
        let Some(arc) = editor.buffers.get(buf_id) else { return };
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let mut ranges = Vec::new();
        let mut from = 0usize;
        let plen = pattern.len();
        while let Some(at) = buf.find_sub_from(from, pattern) {
            ranges.push((at, at + plen, "search".to_string()));
            from = at + 1;
            if ranges.len() > 10_000 { break; } // sanity cap
        }
        ranges
    };
    if let Some(view) = editor.view_of_buffer_mut(buf_id) {
        view.set_highlights(ranges);
    }
}

fn execute_search(editor: &mut Editor, pattern: &str, forward: bool) -> CommandResult {
    let buf_id = get_current_buffer_id(editor);
    let pos = {
        let Some(view) = editor.view_of_buffer(buf_id) else { return Ok(()); };
        view.cursor_offset()
    };

    if pattern.is_empty() {
        if let Some(view) = editor.view_of_buffer_mut(buf_id) {
            view.clear_highlights();
        }
        return Ok(());
    }

    highlight_all_matches(editor, buf_id, pattern);

    let found = {
        let Some(arc) = editor.buffers.get(buf_id) else { return Ok(()); };
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        if forward {
            let start = (pos + 1).min(buf.len());
            buf.find_sub_from(start, pattern).or_else(|| {
                // Wrap: the first match that ends before the restart point.
                buf.find_sub_from(0, pattern)
                    .filter(|at| at + pattern.len() < start)
            })
        } else {
            buf.rfind_sub_before(pos, pattern).or_else(|| {
                // Wrap: the last match at/after the cursor.
                let mut last = None;
                let mut from = pos;
                while let Some(at) = buf.find_sub_from(from, pattern) {
                    last = Some(at);
                    from = at + 1;
                }
                last
            })
        }
    };
    if let Some(np) = found {
        if let Some(view) = editor.view_of_buffer_mut(buf_id) {
            view.set_cursor(np);
        }
        emit_cursor_moved(editor);
    }
    Ok(())
}
