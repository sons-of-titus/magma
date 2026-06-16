//! Buffer-related builtin commands.

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::state::Editor;
use crate::state::id::BufferId;
use crate::event::payload::*;
use crate::event::keys;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn("save-buffer", "Save current buffer to disk",
        vec![
            ArgSpec::optional("path", ArgType::Path, ArgValue::Path("".to_string())),
        ],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            let buf = editor.buffers.get_mut(buf_id)
                .ok_or_else(|| "No current buffer".to_string())?;
            let path = args.get("path").and_then(|a| a.as_string())
                .filter(|p| !p.is_empty())
                .map(|p| p.to_string())
                .or_else(|| buf.path.clone())
                .ok_or_else(|| "No path specified".to_string())?;
            let content = buf.slice(0, buf.len());
            editor.events.emit_typed(keys::events::BUFFER_BEFORE_SAVE, BufferBeforeSavePayload {
                path: path.clone(),
                buffer_id: buf_id.to_string(),
            });
            editor.fs.write(&path, &content)
                .map_err(|e| format!("Save failed: {}", e))?;
            if let Some(buf) = editor.buffers.get_mut(buf_id) {
                buf.mark_saved();
            }
            editor.events.emit_typed(keys::events::BUFFER_AFTER_SAVE, BufferAfterSavePayload {
                path,
                buffer_id: buf_id.to_string(),
            });
            Ok(())
        },
    );

    cmds.register_fn("open-file", "Open a file into a buffer",
        vec![
            ArgSpec::new("path", ArgType::Path),
        ],
        |editor, args| {
            let path = args.get("path")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "Path required".to_string())?;
            let content = editor.fs.read(path)
                .map_err(|e| format!("Read failed: {}", e))?;
            let name = std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled".to_string());
            let buf_id = editor.allocate_buffer_id();
            let mut buffer = crate::buffer::Buffer::from_string(
                BufferId(buf_id), &name, &content);
            buffer.path = Some(path.to_string());
            let entry = editor.buffers.vacant_entry();
            entry.insert(buffer);
            editor.events.emit_typed(keys::events::BUFFER_CREATED, BufferCreatedPayload {
                buffer_id: buf_id.to_string(),
                name,
                path: path.to_string(),
            });
            Ok(())
        },
    );

    cmds.register_fn("find-files", "Search files recursively by pattern",
        vec![
            ArgSpec::optional("pattern", ArgType::String, ArgValue::String("".to_string())),
        ],
        |editor, args| {
            let raw_pattern = args.get("pattern")
                .and_then(|a| a.as_string())
                .unwrap_or("");
            let pattern = raw_pattern.to_lowercase();

            // Collect files recursively
            let cwd = std::env::current_dir()
                .map_err(|e| format!("Can't get cwd: {}", e))?;
            let mut files: Vec<String> = Vec::new();
            let mut dirs = vec![cwd.clone()];
            let ignore = [".git", "node_modules", "target", ".DS_Store", ".zig-cache", ".cache"];

            while let Some(dir) = dirs.pop() {
                let entries = match std::fs::read_dir(&dir) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if ignore.iter().any(|i| path.to_string_lossy().contains(i)) {
                        continue;
                    }
                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.is_file() {
                        let rel = path.strip_prefix(&cwd)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();
                        if pattern.is_empty() || rel.to_lowercase().contains(&pattern) {
                            files.push(rel);
                        }
                    }
                }
            }

            files.sort();
            files.dedup();

            if files.is_empty() {
                return Err(format!("No files match '{}'", pattern));
            }

            // Create a finder buffer
            let content = files.join("\n") + "\n";
            let name = format!("*finder:{}*", pattern);
            let buf_id = editor.allocate_buffer_id();
            let mut buffer = crate::buffer::Buffer::from_string(
                BufferId(buf_id), &name, &content,
            );
            buffer.path = None;
            let entry = editor.buffers.vacant_entry();
            let key = entry.key();
            entry.insert(buffer);

            // Set the current window to show it
            if let Some(win) = editor.windows.focused_window_mut() {
                win.buffer_id = Some(key);
            }

            // Emit event so Janet can set up keybindings
            editor.events.emit_typed(keys::events::FINDER_RESULTS, FinderResultsPayload {
                buffer_id: buf_id.to_string(),
                count: files.len().to_string(),
                pattern,
            });

            Ok(())
        },
    );

    cmds.register_fn("close-buffer",
        "Close a buffer and emit buffer-closed (defaults to focused buffer)",
        vec![
            ArgSpec::optional("buffer-id", ArgType::Integer, ArgValue::Integer(0)),
        ],
        |editor, args| {
            let key = args.get("buffer-id")
                .and_then(|a| a.as_integer())
                .filter(|&k| k > 0)
                .map(|k| k as usize)
                .unwrap_or_else(|| get_current_buffer_id(editor));

            if !editor.buffers.contains(key) {
                return Ok(());
            }
            editor.events.emit_typed(keys::events::BUFFER_CLOSED, BufferClosedPayload {
                buffer_id: key.to_string(),
            });
            editor.buffers.remove(key);
            // If the focused window was showing the closed buffer, switch to another
            let focused_shows_closed = editor.windows.focused_window()
                .and_then(|wid| editor.windows.buffer(wid))
                == Some(key);
            if focused_shows_closed {
                let other = editor.buffers.iter().next().map(|(k, _)| k);
                if let Some(win) = editor.windows.focused_window_mut() {
                    win.buffer_id = other;
                }
                if let Some(new_id) = other {
                    editor.events.emit_typed(keys::events::BUFFER_FOCUSED, BufferFocusedPayload {
                        buffer_id: new_id.to_string(),
                    });
                }
            }
            Ok(())
        },
    );

    cmds.register_fn("alternate-buffer", "Jump to alternate buffer (ctrl-^)",
        vec![],
        |editor, _args| {
            let current = get_current_buffer_id(editor);
            let mut switched_to: Option<usize> = None;
            if let Some(alt_key) = editor.alternate_buffer.filter(|k| *k != current && editor.buffers.contains(*k)) {
                editor.alternate_buffer = Some(current);
                if let Some(win) = editor.windows.focused_window_mut() {
                    win.buffer_id = Some(alt_key);
                }
                switched_to = Some(alt_key);
            }
            // On first use with no alternate, set it to the last buffer if one exists
            if editor.alternate_buffer.is_none() {
                // Find any other buffer
                let others: Vec<usize> = editor.buffers.iter()
                    .map(|(k, _)| k)
                    .filter(|k| *k != current)
                    .collect();
                if let Some(&first_other) = others.first() {
                    editor.alternate_buffer = Some(current);
                    if let Some(win) = editor.windows.focused_window_mut() {
                        win.buffer_id = Some(first_other);
                    }
                    switched_to = Some(first_other);
                }
            }
            if let Some(new_id) = switched_to {
                editor.events.emit_typed(keys::events::BUFFER_FOCUSED, BufferFocusedPayload {
                    buffer_id: new_id.to_string(),
                });
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );
}
