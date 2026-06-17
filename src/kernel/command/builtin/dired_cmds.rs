//! Dired commands — navigation, display, and mark management.

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::event::keys::events;
use crate::kernel::event::payload::{BufferChangedPayload, BufferFocusedPayload};
use crate::kernel::state::Editor;
use crate::kernel::vc::dired::{self, MarkType, SortField};

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("dired",
        "Open a directory browser buffer (:dired [path])",
        vec![ArgSpec::optional("path", ArgType::String, ArgValue::String(String::new()))],
        |editor, args| {
            let raw = args.get("path").and_then(|a| a.as_string()).unwrap_or("").trim().to_string();
            let dir = if raw.is_empty() {
                std::env::current_dir().map_err(|e| e.to_string())?
            } else {
                std::path::PathBuf::from(&raw).canonicalize().map_err(|e| e.to_string())?
            };
            dired_open(editor, dir)
        },
    );

    cmds.register_fn("dired-refresh",
        "Refresh the dired buffer listing",
        vec![],
        |editor, _args| dired_refresh_helper(editor),
    );

    cmds.register_fn("dired-open-at-cursor",
        "Open the file or directory under the cursor in dired",
        vec![],
        |editor, _args| dired_open_at_cursor_helper(editor),
    );

    cmds.register_fn("dired-parent",
        "Navigate to the parent directory in dired",
        vec![],
        |editor, _args| {
            let parent = {
                let dir = editor.dired.dir.clone();
                dir.parent().map(|p| p.to_path_buf()).unwrap_or(dir)
            };
            dired_open(editor, parent)
        },
    );

    cmds.register_fn("dired-mark",
        "Toggle a mark on the entry under the cursor (default: delete)",
        vec![ArgSpec::optional("type", ArgType::String, ArgValue::String("delete".into()))],
        |editor, args| {
            let mt = parse_mark_type(args.get("type").and_then(|a| a.as_string()).unwrap_or("delete"));
            let line = dired_cursor_line(editor);
            if let Some(name) = editor.dired.entry_at_line(line).map(|e| e.name.clone()) {
                editor.dired.toggle_mark(&name, mt);
                dired_redraw(editor)?;
                if let Some(key) = editor.dired.buf_key
                    && let Some(view) = editor.views.get_mut(&key) {
                        let next_off = view.buffer.lock().unwrap().line_start_offset(view.cursor.line + 1);
                        if let Some(off) = next_off { view.set_cursor(off); }
                    }
            }
            Ok(())
        },
    );

    cmds.register_fn("dired-mark-with-type",
        "Mark entry with explicit type: delete, copy, or move",
        vec![ArgSpec::new("type", ArgType::String)],
        |editor, args| {
            let mt = parse_mark_type(args.get("type").and_then(|a| a.as_string()).unwrap_or("delete"));
            let line = dired_cursor_line(editor);
            if let Some(name) = editor.dired.entry_at_line(line).map(|e| e.name.clone()) {
                editor.dired.toggle_mark(&name, mt);
                dired_redraw(editor)?;
            }
            Ok(())
        },
    );

    cmds.register_fn("dired-unmark-all",
        "Clear all marks in dired",
        vec![],
        |editor, _args| {
            editor.dired.marks.clear();
            dired_redraw(editor)
        },
    );

    cmds.register_fn("dired-invert-marks",
        "Invert marks (mark unmarked, unmark marked)",
        vec![],
        |editor, _args| {
            editor.dired.invert_marks();
            dired_redraw(editor)
        },
    );

    cmds.register_fn("dired-filter",
        "Filter dired entries by pattern (empty clears)",
        vec![ArgSpec::new("pattern", ArgType::String)],
        |editor, args| {
            let pat = args.get("pattern").and_then(|a| a.as_string())
                .unwrap_or("").trim().to_string();
            if pat.is_empty() { editor.dired.filter_pattern = None; }
            else { editor.dired.filter_pattern = Some(pat); }
            dired_redraw(editor)
        },
    );

    cmds.register_fn("dired-clear-filter",
        "Clear the dired filter",
        vec![],
        |editor, _args| {
            editor.dired.filter_pattern = None;
            dired_redraw(editor)
        },
    );

    cmds.register_fn("dired-toggle-hidden",
        "Toggle display of hidden files (dotfiles)",
        vec![],
        |editor, _args| {
            editor.dired.show_hidden = !editor.dired.show_hidden;
            dired_redraw(editor)
        },
    );

    cmds.register_fn("dired-toggle-sort",
        "Cycle sort field: name → size → date",
        vec![],
        |editor, _args| {
            editor.dired.sort_field = match editor.dired.sort_field {
                SortField::Name => SortField::Size,
                SortField::Size => SortField::Date,
                SortField::Date => SortField::Name,
            };
            dired_redraw(editor)
        },
    );

    cmds.register_fn("dired-toggle-sort-reverse",
        "Reverse the current sort direction",
        vec![],
        |editor, _args| {
            editor.dired.sort_reverse = !editor.dired.sort_reverse;
            dired_redraw(editor)
        },
    );

    cmds.register_fn("dired-close",
        "Close the dired buffer and return to the previous buffer",
        vec![],
        |editor, _args| {
            editor.keymaps.pop_layer("dired");
            editor.dired.active = false;
            editor.dired.buf_key = None;
            Ok(())
        },
    );

}

fn parse_mark_type(s: &str) -> MarkType {
    match s.trim().to_lowercase().as_str() {
        "copy" => MarkType::Copy,
        "move" => MarkType::Move,
        _      => MarkType::Delete,
    }
}

fn dired_cursor_line(editor: &Editor) -> usize {
    let Some(key) = editor.dired.buf_key else { return 0 };
    editor.views.get(&key).map(|v| v.cursor.line).unwrap_or(0)
}

fn dired_redraw(editor: &mut Editor) -> CommandResult {
    let text = dired::build_display(
        &editor.dired.dir, &editor.dired.entries, &editor.dired.marks,
        editor.dired.sort_field, editor.dired.sort_reverse,
        editor.dired.filter_pattern.as_deref(), editor.dired.show_hidden);
    let Some(key) = editor.dired.buf_key else { return Ok(()); };
    let saved_line = editor.views.get(&key).map(|v| v.cursor.line).unwrap_or(0);
    if let Some(arc) = editor.buffers.get(key) {
        let mut buf = arc.lock().unwrap();
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, &text);
        let new_off = buf.line_start_offset(saved_line.min(buf.line_count().saturating_sub(1)));
        drop(buf);
        if let Some(off) = new_off {
            if let Some(view) = editor.views.get_mut(&key) {
                view.set_cursor(off);
            }
        }
    }
    Ok(())
}

fn dired_refresh_helper(editor: &mut Editor) -> CommandResult {
    let text = editor.dired.reload()?;
    let Some(key) = editor.dired.buf_key else { return Ok(()); };
    let saved_line = editor.views.get(&key).map(|v| v.cursor.line).unwrap_or(0);
    if let Some(arc) = editor.buffers.get(key) {
        let mut buf = arc.lock().unwrap();
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, &text);
        let new_off = buf.line_start_offset(saved_line.min(buf.line_count().saturating_sub(1)));
        drop(buf);
        if let Some(off) = new_off {
            if let Some(view) = editor.views.get_mut(&key) {
                view.set_cursor(off);
            }
        }
    }
    Ok(())
}

fn dired_open(editor: &mut Editor, dir: std::path::PathBuf) -> CommandResult {
    let text = {
        editor.dired.dir = dir;
        editor.dired.marks.clear();
        editor.dired.filter_pattern = None;
        editor.dired.sort_field = SortField::Name;
        editor.dired.sort_reverse = false;
        editor.dired.show_hidden = false;
        editor.dired.reload()?
    };
    let buf_key = editor.dired.buf_key
        .filter(|k| editor.buffers.contains(*k))
        .unwrap_or_else(|| editor.create_buffer("*dired*"));
    editor.dired.buf_key = Some(buf_key);
    editor.dired.active = true;
    {
        let arc = editor.buffers.get(buf_key).unwrap();
        let mut buf = arc.lock().unwrap();
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, &text);
        let header_off = buf.line_start_offset(dired::HEADER_LINES);
        drop(buf);
        if let Some(off) = header_off {
            if let Some(view) = editor.views.get_mut(&buf_key) {
                view.set_cursor(off);
            }
        }
    }
    if let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = Some(buf_key);
    }
    editor.events.emit_typed(events::BUFFER_FOCUSED, BufferFocusedPayload {
        buffer_id: buf_key.to_string(),
    });
    editor.keymaps.push_layer("dired");
    Ok(())
}

fn dired_open_at_cursor_helper(editor: &mut Editor) -> CommandResult {
    let line = dired_cursor_line(editor);
    let Some(entry) = editor.dired.entry_at_line(line) else { return Ok(()); };
    let name = entry.name.clone();
    let is_dir = entry.is_dir;
    let path = editor.dired.full_path(&name);

    if name == "." { return Ok(()); }

    if is_dir {
        let canonical = path.canonicalize().map_err(|e| e.to_string())?;
        editor.keymaps.pop_layer("dired");
        dired_open(editor, canonical)
    } else {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        let fname = path.file_name().map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        let key = editor.create_buffer_from_str(&fname, &content);
        let path_str = path.to_string_lossy().into_owned();
        if let Some(arc) = editor.buffers.get(key) {
            arc.lock().unwrap().path = Some(path_str.clone());
        }
        editor.events.emit_typed(events::BUFFER_CHANGED, BufferChangedPayload {
            buffer_id: key.to_string(),
        });
        if let Some(win) = editor.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        editor.keymaps.pop_layer("dired");
        editor.dired.active = false;
        Ok(())
    }
}
