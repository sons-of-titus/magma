//! Completion and snippet commands.

use std::collections::HashMap;

use crate::command::CommandResult;
use crate::command::args::ArgValue;
use crate::state::Editor;
use crate::state::mode::{EditorMode, Selection};

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("completion-trigger",
        "Trigger completion popup (ctrl-n)",
        vec![],
        |editor, _args| {
            completion_trigger(editor, _args)
        },
    );

    cmds.register_fn("completion-next",
        "Select next completion item",
        vec![],
        |editor, _args| {
            completion_next(editor, _args)
        },
    );

    cmds.register_fn("completion-prev",
        "Select previous completion item (ctrl-p)",
        vec![],
        |editor, _args| {
            completion_prev(editor, _args)
        },
    );

    cmds.register_fn("completion-accept",
        "Accept current completion item (tab/return)",
        vec![],
        |editor, _args| {
            completion_accept(editor, _args)
        },
    );

    cmds.register_fn("completion-dismiss",
        "Dismiss completion popup (esc)",
        vec![],
        |editor, _args| {
            completion_dismiss(editor, _args)
        },
    );

    cmds.register_fn("snippet-expand",
        "Expand snippet at cursor (ctrl-])",
        vec![],
        |editor, _args| {
            snippet_expand(editor, _args)
        },
    );

    cmds.register_fn("snippet-next-tabstop",
        "Move to next snippet tabstop (tab)",
        vec![],
        |editor, _args| {
            snippet_next_tabstop(editor, _args)
        },
    );

    cmds.register_fn("snippet-prev-tabstop",
        "Move to previous snippet tabstop (shift-tab)",
        vec![],
        |editor, _args| {
            snippet_prev_tabstop(editor, _args)
        },
    );
}

fn completion_trigger(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    let buf_id = get_current_buffer_id(editor);
    if let Some(buf) = editor.buffers.get(buf_id) {
        let cursor = buf.cursor();
        let text = buf.slice(0, buf.len());

        // Find the current word prefix
        let before = &text[..cursor];
        let word_start = before.rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map(|i| i + 1).unwrap_or(0);
        let prefix = &text[word_start..cursor];

        if prefix.is_empty() { return Ok(()); }
        editor.completion.prefix = prefix.to_string();

        // Collect words from buffer as completion candidates
        let mut words: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for w in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if !w.is_empty() && w.starts_with(prefix) && w != prefix {
                words.insert(w.to_string());
            }
        }
        let mut items: Vec<String> = words.into_iter().collect();
        items.sort();
        items.truncate(50);

        if items.is_empty() { return Ok(()); }
        editor.completion.items = items;
        editor.completion.idx = 0;
        editor.completion.visible = true;
    }
    Ok(())
}

fn completion_next(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    if editor.completion.visible && !editor.completion.items.is_empty() {
        editor.completion.idx = (editor.completion.idx + 1) % editor.completion.items.len();
    }
    Ok(())
}

fn completion_prev(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    if editor.completion.visible && !editor.completion.items.is_empty() {
        editor.completion.idx = if editor.completion.idx == 0 {
            editor.completion.items.len() - 1
        } else {
            editor.completion.idx - 1
        };
    }
    Ok(())
}

fn completion_accept(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    if !editor.completion.visible || editor.completion.items.is_empty() {
        return Ok(());
    }
    let buf_id = get_current_buffer_id(editor);
    let item = &editor.completion.items[editor.completion.idx];
    let suffix = &item[editor.completion.prefix.len()..];

    if let Some(buf) = editor.buffers.get_mut(buf_id) {
        buf.insert(buf.cursor(), suffix);
    }
    editor.completion.visible = false;
    editor.completion.items.clear();
    emit_cursor_moved(editor);
    Ok(())
}

fn completion_dismiss(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    editor.completion.visible = false;
    editor.completion.items.clear();
    Ok(())
}

fn snippet_expand(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    let buf_id = get_current_buffer_id(editor);

    // Extract trigger word before any mutable borrow
    let (word_start, trigger) = {
        let Some(buf) = editor.buffers.get(buf_id) else { return Ok(()); };
        let cursor = buf.cursor();
        let text = buf.slice(0, buf.len());
        let before = &text[..cursor];
        let ws = before.rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map(|i| i + 1).unwrap_or(0);
        if cursor <= ws { return Ok(()); }
        let trig = text[ws..cursor].to_string();
        (ws, trig)
    };

    if let Some(expansion) = crate::snippet::expand_snippet(editor, &trigger)
        && let Some(buf) = editor.buffers.get_mut(buf_id) {
            buf.replace(word_start, word_start + trigger.len(), &expansion);
            buf.set_cursor(word_start);
            editor.snippet.active = true;
            editor.snippet.tabstops = crate::snippet::parse_tabstops(&expansion);
            editor.snippet.tabstop_idx = 0;
        }
    emit_cursor_moved(editor);
    Ok(())
}

fn snippet_next_tabstop(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    if !editor.snippet.active || editor.snippet.tabstops.is_empty() {
        completion_accept(editor, _args)?;
        // If completion was not visible either, insert a literal tab
        if !editor.completion.visible {
            let buf_id = get_current_buffer_id(editor);
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.insert(buf.cursor(), "\t");
            }
        }
        return Ok(());
    }
    let buf_id = get_current_buffer_id(editor);
    if let Some(buf) = editor.buffers.get_mut(buf_id) {
        if editor.snippet.tabstop_idx + 1 < editor.snippet.tabstops.len() {
            editor.snippet.tabstop_idx += 1;
            let ts = &editor.snippet.tabstops[editor.snippet.tabstop_idx];
            buf.set_cursor(ts.offset);
            // Select placeholder if present
            if !ts.text.is_empty() {
                editor.editor_mode = EditorMode::new("visual", false);
                editor.selection = Some(Selection { anchor: ts.offset, kind: "char".into() });
                editor.keymaps.push_layer("visual");
            }
        } else {
            editor.snippet.active = false;
            editor.snippet.tabstops.clear();
        }
    }
    emit_cursor_moved(editor);
    Ok(())
}

fn snippet_prev_tabstop(editor: &mut Editor, _args: &HashMap<String, ArgValue>) -> CommandResult {
    if !editor.snippet.active || editor.snippet.tabstops.is_empty() {
        return Ok(());
    }
    if editor.snippet.tabstop_idx > 0 {
        editor.snippet.tabstop_idx -= 1;
        let buf_id = get_current_buffer_id(editor);
        if let Some(buf) = editor.buffers.get_mut(buf_id) {
            let ts = &editor.snippet.tabstops[editor.snippet.tabstop_idx];
            buf.set_cursor(ts.offset);
        }
    }
    emit_cursor_moved(editor);
    Ok(())
}
