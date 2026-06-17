//! Dired file operation commands — rename, copy, move, symlink, mkdir, delete.

use std::path::Path;

use crate::kernel::command::CommandResult;
use crate::kernel::command::args::{ArgSpec, ArgType};
use crate::kernel::state::Editor;
use crate::kernel::vc::dired::MarkType;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("dired-execute-deletion",
        "Delete all files marked for deletion",
        vec![],
        |editor, _args| {
            let names: Vec<String> = editor.dired.marks.iter()
                .filter(|(_, mt)| **mt == MarkType::Delete)
                .map(|(name, _)| name.clone())
                .collect();
            if names.is_empty() {
                return Err("No files marked for deletion".to_string());
            }
            for name in &names {
                let path = editor.dired.full_path(name);
                delete_entry(&path)?;
                editor.dired.marks.remove(name);
            }
            dired_redraw_helper(editor)
        },
    );

    cmds.register_fn("dired-execute-copy",
        "Copy all marked files to target directory",
        vec![ArgSpec::new("target", ArgType::String)],
        |editor, args| {
            let target = args.get("target").and_then(|a| a.as_string())
                .ok_or_else(|| "No target directory specified".to_string())?;
            let target_path = std::path::PathBuf::from(target);
            if !target_path.is_dir() {
                return Err(format!("Not a directory: {target}"));
            }
            let names: Vec<String> = editor.dired.marks.iter()
                .filter(|(_, mt)| **mt == MarkType::Copy)
                .map(|(name, _)| name.clone())
                .collect();
            if names.is_empty() {
                return Err("No files marked for copy".to_string());
            }
            for name in &names {
                let src = editor.dired.full_path(name);
                let dst = target_path.join(name);
                copy_entry(&src, &dst)?;
                editor.dired.marks.remove(name);
            }
            dired_redraw_helper(editor)
        },
    );

    cmds.register_fn("dired-execute-move",
        "Move all marked files to target directory",
        vec![ArgSpec::new("target", ArgType::String)],
        |editor, args| {
            let target = args.get("target").and_then(|a| a.as_string())
                .ok_or_else(|| "No target directory specified".to_string())?;
            let target_path = std::path::PathBuf::from(target);
            if !target_path.is_dir() {
                return Err(format!("Not a directory: {target}"));
            }
            let names: Vec<String> = editor.dired.marks.iter()
                .filter(|(_, mt)| **mt == MarkType::Move)
                .map(|(name, _)| name.clone())
                .collect();
            if names.is_empty() {
                return Err("No files marked for move".to_string());
            }
            for name in &names {
                let src = editor.dired.full_path(name);
                let dst = target_path.join(name);
                std::fs::rename(&src, &dst)
                    .map_err(|e| format!("Cannot move '{}' to '{}': {e}", src.display(), dst.display()))?;
                editor.dired.marks.remove(name);
            }
            dired_redraw_helper(editor)
        },
    );

    cmds.register_fn("dired-rename",
        "Rename the file under the cursor",
        vec![ArgSpec::new("new-name", ArgType::String)],
        |editor, args| {
            let new_name = args.get("new-name").and_then(|a| a.as_string())
                .ok_or_else(|| "No new name specified".to_string())?;
            let line = dired_cursor_line(editor);
            let Some(entry) = editor.dired.entry_at_line(line) else {
                return Err("No entry under cursor".to_string());
            };
            let src = editor.dired.full_path(&entry.name);
            let dst = editor.dired.dir.join(new_name);
            if dst.exists() {
                return Err(format!("Target exists: {}", dst.display()));
            }
            std::fs::rename(&src, &dst)
                .map_err(|e| format!("Cannot rename '{}' to '{}': {e}", src.display(), dst.display()))?;
            dired_redraw_helper(editor)
        },
    );

    cmds.register_fn("dired-copy",
        "Copy the file under the cursor to a new path",
        vec![ArgSpec::new("target", ArgType::String)],
        |editor, args| {
            let target = args.get("target").and_then(|a| a.as_string())
                .ok_or_else(|| "No target specified".to_string())?;
            let line = dired_cursor_line(editor);
            let Some(entry) = editor.dired.entry_at_line(line) else {
                return Err("No entry under cursor".to_string());
            };
            let src = editor.dired.full_path(&entry.name);
            let dst = std::path::PathBuf::from(target);
            if dst.exists() {
                return Err(format!("Target exists: {}", dst.display()));
            }
            copy_entry(&src, &dst)?;
            dired_redraw_helper(editor)
        },
    );

    cmds.register_fn("dired-move",
        "Move the file under the cursor to a new path",
        vec![ArgSpec::new("target", ArgType::String)],
        |editor, args| {
            let target = args.get("target").and_then(|a| a.as_string())
                .ok_or_else(|| "No target specified".to_string())?;
            let line = dired_cursor_line(editor);
            let Some(entry) = editor.dired.entry_at_line(line) else {
                return Err("No entry under cursor".to_string());
            };
            let src = editor.dired.full_path(&entry.name);
            let dst = std::path::PathBuf::from(target);
            if dst.exists() {
                return Err(format!("Target exists: {}", dst.display()));
            }
            std::fs::rename(&src, &dst)
                .map_err(|e| format!("Cannot move '{}' to '{}': {e}", src.display(), dst.display()))?;
            dired_redraw_helper(editor)
        },
    );

    cmds.register_fn("dired-symlink",
        "Create a symbolic link to the file under the cursor",
        vec![ArgSpec::new("link-path", ArgType::String)],
        |editor, args| {
            let link = args.get("link-path").and_then(|a| a.as_string())
                .ok_or_else(|| "No link path specified".to_string())?;
            let line = dired_cursor_line(editor);
            let Some(entry) = editor.dired.entry_at_line(line) else {
                return Err("No entry under cursor".to_string());
            };
            let src = editor.dired.full_path(&entry.name);
            let dst = std::path::PathBuf::from(link);
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&src, &dst)
                    .map_err(|e| format!("Cannot symlink '{}' -> '{}': {e}", src.display(), dst.display()))?;
            }
            #[cfg(not(unix))]
            {
                let _ = (&src, &dst);
                return Err("Symlinks not supported on this platform".to_string());
            }
            dired_redraw_helper(editor)
        },
    );

    cmds.register_fn("dired-mkdir",
        "Create a new directory",
        vec![ArgSpec::new("dir-name", ArgType::String)],
        |editor, args| {
            let name = args.get("dir-name").and_then(|a| a.as_string())
                .ok_or_else(|| "No directory name specified".to_string())?;
            let path = editor.dired.dir.join(name);
            std::fs::create_dir(&path)
                .map_err(|e| format!("Cannot create directory '{}': {e}", path.display()))?;
            dired_redraw_helper(editor)
        },
    );
}

fn delete_entry(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("Cannot delete directory '{}': {e}", path.display()))
    } else {
        std::fs::remove_file(path)
            .map_err(|e| format!("Cannot delete file '{}': {e}", path.display()))
    }
}

fn copy_entry(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        dired_copy_recursive(src, dst)
    } else {
        std::fs::copy(src, dst)
            .map_err(|e| format!("Cannot copy '{}' to '{}': {e}", src.display(), dst.display()))?;
        Ok(())
    }
}

fn dired_copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Cannot create target '{}': {e}", dst.display()))?;
    let entries = std::fs::read_dir(src)
        .map_err(|e| format!("Cannot read '{}': {e}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Entry error: {e}"))?;
        let file_type = entry.file_type().map_err(|e| format!("File type error: {e}"))?;
        let name = entry.file_name();
        let src_child = entry.path();
        let dst_child = dst.join(&name);
        if file_type.is_dir() {
            dired_copy_recursive(&src_child, &dst_child)?;
        } else {
            std::fs::copy(&src_child, &dst_child)
                .map_err(|e| format!("Cannot copy '{}' to '{}': {e}",
                    src_child.display(), dst_child.display()))?;
        }
    }
    Ok(())
}

fn dired_cursor_line(editor: &Editor) -> usize {
    let Some(key) = editor.dired.buf_key else { return 0 };
    editor.views.get(&key).map(|v| v.cursor.line).unwrap_or(0)
}

fn dired_redraw_helper(editor: &mut Editor) -> CommandResult {
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
