//! File and dired commands.

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType, ArgValue};
use crate::kernel::state::Editor;
use crate::kernel::state::id::BufferId;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("open-file-at-cursor", "Open the file path under the cursor",
        vec![],
        |editor, _args| {
            let path = {
                let buf = editor.buffers.get(get_current_buffer_id(editor))
                    .ok_or_else(|| "No buffer".to_string())?;
                let line_num = current_line(buf);
                buf.line(line_num)
                    .ok_or_else(|| "Could not read line".to_string())?
                    .trim()
                    .to_string()
            };

            if path.is_empty() {
                return Err("No text on current line".to_string());
            }

            let content = editor.fs.read(&path)
                .map_err(|e| format!("Can't open '{}': {}", path, e))?;
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let new_buf_id = editor.allocate_buffer_id();
            let mut new_buf = crate::kernel::text_engine::Buffer::from_string(
                BufferId(new_buf_id), &name, &content,
            );
            new_buf.path = Some(path);
            let entry = editor.buffers.vacant_entry();
            let key = entry.key();
            entry.insert(new_buf);
            if let Some(win) = editor.windows.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            Ok(())
        },
    );

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

    cmds.register_fn("dired-mark-delete",
        "Toggle deletion mark on the entry under the cursor",
        vec![],
        |editor, _args| {
            let line = dired_cursor_line(editor);
            if let Some(name) = editor.dired.entry_at_line(line).map(|e| e.name.clone()) {
                if name == "." || name == ".." { return Ok(()); }
                if editor.dired.marks.contains(&name) {
                    editor.dired.marks.remove(&name);
                } else {
                    editor.dired.marks.insert(name);
                }
                dired_redraw(editor)?;
                // Advance cursor one line
                if let Some(key) = editor.dired.buf_key
                    && let Some(buf) = editor.buffers.get_mut(key) {
                        let cur_line = buf.slice(0, buf.cursor()).chars().filter(|&c| c == '\n').count();
                        if let Some(off) = buf.line_start_offset(cur_line + 1) {
                            buf.set_cursor(off);
                        }
                    }
            }
            Ok(())
        },
    );

    cmds.register_fn("dired-unmark-all",
        "Clear all deletion marks in dired",
        vec![],
        |editor, _args| {
            editor.dired.marks.clear();
            dired_redraw(editor)
        },
    );

    cmds.register_fn("dired-execute-deletion",
        "Delete all files marked for deletion in dired",
        vec![],
        |editor, _args| {
            let marked: Vec<String> = editor.dired.marks.iter().cloned().collect();
            if marked.is_empty() {
                return Err("No files marked for deletion".to_string());
            }
            for name in &marked {
                let path = editor.dired.full_path(name);
                std::fs::remove_dir_all(&path)
                    .or_else(|_| std::fs::remove_file(&path))
                    .map_err(|e| format!("Cannot delete {}: {e}", path.display()))?;
            }
            editor.dired.marks.clear();
            dired_refresh_helper(editor)
        },
    );

    cmds.register_fn("dired-rename",
        "Rename the file under the cursor to the given name",
        vec![ArgSpec::new("name", ArgType::String)],
        |editor, args| {
            let new_name = args.get("name").and_then(|a| a.as_string())
                .ok_or_else(|| "dired-rename requires a name".to_string())?
                .trim().to_string();
            let line = dired_cursor_line(editor);
            let old_name = editor.dired.entry_at_line(line)
                .map(|e| e.name.clone())
                .ok_or_else(|| "No entry under cursor".to_string())?;
            let src = editor.dired.full_path(&old_name);
            let dst = editor.dired.full_path(&new_name);
            std::fs::rename(&src, &dst).map_err(|e| format!("rename failed: {e}"))?;
            dired_refresh_helper(editor)
        },
    );

    cmds.register_fn("dired-copy",
        "Copy the file under the cursor to the given destination name",
        vec![ArgSpec::new("name", ArgType::String)],
        |editor, args| {
            let dst_name = args.get("name").and_then(|a| a.as_string())
                .ok_or_else(|| "dired-copy requires a destination name".to_string())?
                .trim().to_string();
            let line = dired_cursor_line(editor);
            let src_name = editor.dired.entry_at_line(line)
                .map(|e| e.name.clone())
                .ok_or_else(|| "No entry under cursor".to_string())?;
            let src = editor.dired.full_path(&src_name);
            let dst = editor.dired.full_path(&dst_name);
            dired_copy_recursive(&src, &dst).map_err(|e| format!("copy failed: {e}"))?;
            dired_refresh_helper(editor)
        },
    );

    cmds.register_fn("dired-mkdir",
        "Create a new directory inside the current dired directory",
        vec![ArgSpec::new("name", ArgType::String)],
        |editor, args| {
            let name = args.get("name").and_then(|a| a.as_string())
                .ok_or_else(|| "dired-mkdir requires a name".to_string())?
                .trim().to_string();
            let path = editor.dired.full_path(&name);
            std::fs::create_dir_all(&path).map_err(|e| format!("mkdir failed: {e}"))?;
            dired_refresh_helper(editor)
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

    // Set up dired keymap layer bindings
    editor.keymaps.set_layer("dired", "return",    "dired-open-at-cursor");
    editor.keymaps.set_layer("dired", "l",         "dired-open-at-cursor");
    editor.keymaps.set_layer("dired", "h",         "dired-parent");
    editor.keymaps.set_layer("dired", "d",         "dired-mark-delete");
    editor.keymaps.set_layer("dired", "u",         "dired-unmark-all");
    editor.keymaps.set_layer("dired", "x",         "dired-execute-deletion");
    editor.keymaps.set_layer("dired", "g",         "dired-refresh");
    editor.keymaps.set_layer("dired", "r",         "dired-refresh");
    editor.keymaps.set_layer("dired", "q",         "dired-close");
}

/// Return the 0-indexed line the dired buffer cursor is on.
fn dired_cursor_line(editor: &Editor) -> usize {
    let Some(key) = editor.dired.buf_key else { return 0 };
    let Some(buf) = editor.buffers.get(key) else { return 0 };
    let cursor = buf.cursor();
    buf.slice(0, cursor).chars().filter(|&c| c == '\n').count()
}

/// Rebuild the dired buffer content from the current state (no disk read).
fn dired_redraw(editor: &mut Editor) -> CommandResult {
    let text = crate::kernel::vc::dired::build_display(&editor.dired.dir.clone(), &editor.dired.entries.clone(), &editor.dired.marks);
    let Some(key) = editor.dired.buf_key else { return Ok(()); };
    let Some(buf) = editor.buffers.get_mut(key) else { return Ok(()); };
    let saved_line = buf.slice(0, buf.cursor()).chars().filter(|&c| c == '\n').count();
    let len = buf.len();
    if len > 0 { buf.delete(0, len); }
    buf.insert(0, &text);
    if let Some(off) = buf.line_start_offset(saved_line) { buf.set_cursor(off); }
    Ok(())
}

/// Re-read the directory from disk and redraw.
fn dired_refresh_helper(editor: &mut Editor) -> CommandResult {
    let text = editor.dired.reload()?;
    let Some(key) = editor.dired.buf_key else { return Ok(()); };
    let Some(buf) = editor.buffers.get_mut(key) else { return Ok(()); };
    let saved_line = buf.slice(0, buf.cursor()).chars().filter(|&c| c == '\n').count();
    let len = buf.len();
    if len > 0 { buf.delete(0, len); }
    buf.insert(0, &text);
    if let Some(off) = buf.line_start_offset(saved_line) { buf.set_cursor(off); }
    Ok(())
}

/// Open a directory in a dired buffer, creating or reusing `*dired*`.
fn dired_open(editor: &mut Editor, dir: std::path::PathBuf) -> CommandResult {
    let text = {
        editor.dired.dir = dir;
        editor.dired.marks.clear();
        editor.dired.reload()?
    };
    // Find or create the *dired* buffer
    let buf_key = editor.dired.buf_key
        .and_then(|k| editor.buffers.get(k).map(|_| k))
        .or_else(|| {
            let entry = editor.buffers.vacant_entry();
            let k = entry.key();
            let id = editor.next_buffer_id;
            editor.next_buffer_id += 1;
            entry.insert(crate::kernel::text_engine::Buffer::new(crate::kernel::state::id::BufferId(id), "*dired*"));
            Some(k)
        })
        .unwrap();
    editor.dired.buf_key = Some(buf_key);
    editor.dired.active = true;
    // Write content
    {
        let buf = editor.buffers.get_mut(buf_key).unwrap();
        let len = buf.len();
        if len > 0 { buf.delete(0, len); }
        buf.insert(0, &text);
        // Place cursor on first entry
        if let Some(off) = buf.line_start_offset(crate::kernel::vc::dired::HEADER_LINES) {
            buf.set_cursor(off);
        }
    }
    // Display in focused window
    if let Some(win) = editor.windows.focused_window_mut() {
        win.buffer_id = Some(buf_key);
    }
    // Push dired keymap layer if not already active
    editor.keymaps.push_layer("dired");
    Ok(())
}

/// Open the file or descend into the directory under the cursor.
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
        // Load the file into a new buffer and switch to it
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        let fname = path.file_name().map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        let id = editor.next_buffer_id;
        editor.next_buffer_id += 1;
        let mut buf = crate::kernel::text_engine::Buffer::from_string(crate::kernel::state::id::BufferId(id), &fname, &content);
        buf.path = Some(path.to_string_lossy().into_owned());
        let k = {
            let entry = editor.buffers.vacant_entry();
            let k = entry.key();
            entry.insert(buf);
            k
        };
        if let Some(win) = editor.windows.focused_window_mut() {
            win.buffer_id = Some(k);
        }
        editor.keymaps.pop_layer("dired");
        editor.dired.active = false;
        Ok(())
    }
}

/// Recursively copy `src` to `dst` (files and directories).
fn dired_copy_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            dired_copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}
