//! File commands (open-file-at-cursor, sidebar file operations).

use crate::state::Editor;

use super::helpers::*;

/// Open (or focus) `path` in the editor.  Shared by create-file and
/// rename-path so the result of a sidebar operation is on screen.
fn open_path(editor: &mut Editor, path: &str) -> Result<(), String> {
    let mut args = std::collections::HashMap::new();
    args.insert(
        "path".to_string(),
        crate::command::args::ArgValue::Path(path.to_string()),
    );
    crate::command::execute_command(editor, "open-file", &args)
}

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "open-file-at-cursor",
        "Open the file path under the cursor",
        vec![],
        |editor, _args| {
            let buf_id = get_current_buffer_id(editor);
            let path = {
                let view = editor
                    .view_of_buffer(buf_id)
                    .ok_or_else(|| "No buffer".to_string())?;
                let line_num = current_line(view);
                let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
                buf.line(line_num)
                    .ok_or_else(|| "Could not read line".to_string())?
                    .trim()
                    .to_string()
            };

            if path.is_empty() {
                return Err("No text on current line".to_string());
            }
            let path = crate::project::resolve_path(
                editor.project_manager.project.root.as_deref(),
                &path,
            );

            let content = editor
                .fs
                .read(&path)
                .map_err(|e| format!("Can't open '{}': {}", path, e))?;
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let key = editor.create_buffer_from_str(&name, &content);
            if let Some(arc) = editor.buffers.get(key) {
                arc.lock().unwrap_or_else(|e| e.into_inner()).path = Some(path);
            }
            if let Some(win) = editor.view_tree.focused_window_mut() {
                win.buffer_id = Some(key);
            }
            Ok(())
        },
    );

    cmds.register_fn(
        "create-file",
        "Create an empty file (and its parent dirs) and open it",
        vec![crate::command::args::ArgSpec::new(
            "path",
            crate::command::args::ArgType::Path,
        )],
        |editor, args| {
            let raw = args
                .get("path")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "Path required".to_string())?;
            // Anchor before writing, or the file lands next to the process
            // cwd while open-file resolves against the project root.
            let path = crate::project::resolve_path(
                editor.project_manager.project.root.as_deref(),
                raw,
            );
            if std::path::Path::new(&path).exists() {
                return Err(format!("Already exists: {path}"));
            }
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
            }
            std::fs::write(&path, "").map_err(|e| format!("Cannot create {path}: {e}"))?;
            open_path(editor, &path)
        },
    );

    cmds.register_fn(
        "rename-path",
        "Rename a file; open buffers follow",
        vec![
            crate::command::args::ArgSpec::new("from", crate::command::args::ArgType::Path),
            crate::command::args::ArgSpec::new("to", crate::command::args::ArgType::Path),
        ],
        |editor, args| {
            let raw_from = args
                .get("from")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "from required".to_string())?;
            let raw_to = args
                .get("to")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "to required".to_string())?;
            let root = editor.project_manager.project.root.as_deref();
            let from = crate::project::resolve_path(root, raw_from);
            let to = crate::project::resolve_path(root, raw_to);
            if std::path::Path::new(&to).exists() {
                return Err(format!("Already exists: {to}"));
            }
            std::fs::rename(&from, &to).map_err(|e| format!("Cannot rename {from}: {e}"))?;
            // Open buffers follow the rename so they keep saving to disk.
            for (_, arc) in editor.buffers.iter() {
                let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                if buf.path.as_deref() == Some(from.as_str()) {
                    buf.path = Some(to.to_string());
                }
            }
            Ok(())
        },
    );

    cmds.register_fn(
        "delete-path",
        "Delete a file (or empty directory) permanently",
        vec![crate::command::args::ArgSpec::new(
            "path",
            crate::command::args::ArgType::Path,
        )],
        |editor, args| {
            let raw = args
                .get("path")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "Path required".to_string())?;
            let path = crate::project::resolve_path(
                editor.project_manager.project.root.as_deref(),
                raw,
            );
            let p = std::path::Path::new(&path);
            if p.is_dir() {
                std::fs::remove_dir(p)
                    .map_err(|_| format!("Refusing to delete non-empty directory: {path}"))?;
            } else {
                std::fs::remove_file(p).map_err(|e| format!("Cannot delete {path}: {e}"))?;
            }
            // Close any buffer that pointed at the deleted file.
            let doomed: Vec<usize> = editor
                .buffers
                .iter()
                .filter(|(_, arc)| {
                    arc.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .path
                        .as_deref()
                        == Some(path.as_str())
                })
                .map(|(k, _)| k)
                .collect();
            for key in doomed {
                for pane in editor.view_tree.panes_mut() {
                    if pane.buffer_id == Some(key) {
                        pane.buffer_id = None;
                    }
                }
                let _ = editor.buffers.try_remove(key);
                editor.views.remove(&key);
            }
            Ok(())
        },
    );
}
