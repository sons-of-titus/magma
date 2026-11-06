//! Buffer-related builtin commands.

use crate::command::args::{ArgSpec, ArgType, ArgValue};
use crate::event::keys;
use crate::event::payload::*;
use crate::state::Editor;

use super::helpers::*;

pub(super) fn register(editor: &mut Editor) {
    let cmds = &mut editor.commands;

    cmds.register_fn(
        "save-buffer",
        "Save current buffer to disk",
        vec![ArgSpec::optional(
            "path",
            ArgType::Path,
            ArgValue::Path("".to_string()),
        )],
        |editor, args| {
            let buf_id = get_current_buffer_id(editor);
            save_buffer_common(
                editor,
                buf_id,
                args.get("path")
                    .and_then(|a| a.as_string())
                    .map(str::to_string),
            )
        },
    );

    cmds.register_fn(
        "save-all",
        "Save all modified buffers to disk",
        vec![],
        |editor, _args| {
            let dirty: Vec<(usize, Option<String>)> = editor
                .buffers
                .iter()
                .filter_map(|(key, arc)| {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    (buf.modified() || editor.io.saves_in_flight.contains_key(&key))
                        .then(|| (key, buf.path.clone()))
                })
                .collect();
            for (key, _) in dirty {
                if let Err(e) = save_buffer_common(editor, key, None) {
                    editor.show_message(crate::state::MessageKind::Error, e);
                }
            }
            Ok(())
        },
    );

    cmds.register_fn(
        "open-file",
        "Open a file into a buffer",
        vec![ArgSpec::new("path", ArgType::Path)],
        |editor, args| {
            let raw = args
                .get("path")
                .and_then(|a| a.as_string())
                .ok_or_else(|| "Path required".to_string())?;
            // Sidebar rows, finder results, and sessions speak
            // project-relative paths; the fs layer reads against the
            // process cwd — anchor them to the project root here.
            let path = crate::project::resolve_path(
                editor.project_manager.project.root.as_deref(),
                raw,
            );
            // Opening an already-open path focuses it — never fork a duplicate.
            if let Some((key, _)) = editor.buffers.iter().find(|(_, arc)| {
                arc.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .path
                    .as_deref()
                    == Some(path.as_str())
            }) {
                let pane = editor
                    .view_tree
                    .panes()
                    .iter()
                    .find(|p| p.buffer_id == Some(key))
                    .map(|p| p.id);
                match pane {
                    Some(id) => editor.view_tree.focus(id),
                    None => {
                        if let Some(win) = editor.view_tree.focused_window_mut() {
                            win.buffer_id = Some(key);
                        }
                    }
                }
                editor.db.active_tab = None;
                editor.events.emit_typed(
                    keys::events::BUFFER_FOCUSED,
                    BufferFocusedPayload {
                        buffer_id: key.to_string(),
                    },
                );
                return Ok(());
            }
            // The read stays synchronous by design: open-file's callers
            // (session restore, navigation, scripts) position cursors in
            // the opened buffer immediately, and single-file reads are
            // sub-millisecond — the UI-thread stall that mattered was the
            // find-files tree walk, which runs on the pool.
            let path_lower = path.to_lowercase();
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled".to_string());
            let ext = std::path::Path::new(&path_lower)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();
            let is_image = matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif"
            );
            let is_pdf = ext == "pdf";
            if is_image || is_pdf {
                // Binary document: read bytes and decode, never treat as text.
                let bytes = std::fs::read(&path).map_err(|e| format!("Read failed: {path}: {e}"))?;
                if is_image {
                    let dyn_img = image::load_from_memory(&bytes)
                        .map_err(|e| format!("Image decode failed: {path}: {e}"))?;
                    let rgba = dyn_img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let img = crate::text_engine::ImageData {
                        width: w,
                        height: h,
                        pixels: rgba.into_raw(),
                    };
                    let key = editor.create_buffer_from_str(&name, "");
                    let buf_id = key as u64;
                    if let Some(arc) = editor.buffers.get(key) {
                        let mut b = arc.lock().unwrap_or_else(|e| e.into_inner());
                        b.path = Some(path.to_string());
                        b.image = Some(img);
                        b.read_only = true;
                    }
                    // reuse tab logic below by jumping to common? We'll inline tab handling.
                    let reusable = editor.view_tree.focused_window().and_then(|wid| {
                        let pane = editor.view_tree.window(wid)?;
                        match pane.buffer_id {
                            None => Some((wid, None)),
                            Some(buf_key) => {
                                let arc = editor
                                    .buffers
                                    .get(buf_key)?
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner());
                                (arc.path.is_none()
                                    && !arc.modified()
                                    && arc.slice(0, arc.len()).is_empty())
                                .then_some((wid, Some(buf_key)))
                            }
                        }
                    });
                    match reusable {
                        Some((wid, _old)) => {
                            editor.view_tree.set_buffer(wid, key);
                            editor.view_tree.focus(wid);
                        }
                        None => {
                            editor.view_tree.open_tab(Some(key));
                        }
                    }
                    editor.db.active_tab = None;
                    editor.events.emit_typed(
                        keys::events::BUFFER_CREATED,
                        BufferCreatedPayload {
                            buffer_id: buf_id.to_string(),
                            name,
                            path: path.to_string(),
                        },
                    );
                    crate::semantic::fallback_syntax::on_buffer_created(editor, buf_id as usize);
                    editor.events.emit_typed(
                        keys::events::BUFFER_FOCUSED,
                        BufferFocusedPayload {
                            buffer_id: buf_id.to_string(),
                        },
                    );
                    return Ok(());
                } else {
                    // PDF: extract text.
                    let text_content = pdf_extract::extract_text_from_mem(&bytes)
                        .map_err(|e| format!("PDF read failed: {path}: {e}"))?;
                    let content = if text_content.trim().is_empty() {
                        format!("[No extractable text in {name}]")
                    } else {
                        text_content
                    };
                    // fall through to normal open with content by setting a flag
                    // We'll handle via a variable.
                    // Use open_loaded_file with extracted content.
                    open_loaded_file(editor, &path, &content);
                    return Ok(());
                }
            }
            let content = editor
                .fs
                .read(&path)
                .map_err(|e| format!("Read failed: {path}: {e}"))?;
            open_loaded_file(editor, &path, &content);
            Ok(())
        },
    );
}

/// Build the buffer/view/tab for file content that finished loading —
/// the async open event's sink and the sync path share this.
pub fn open_loaded_file(editor: &mut Editor, path: &str, content: &str) {
    editor.db.active_tab = None;
    let name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string());
    let key = editor.create_buffer_from_str(&name, content);
    let buf_id = key as u64;
    if let Some(arc) = editor.buffers.get(key) {
        arc.lock().unwrap_or_else(|e| e.into_inner()).path = Some(path.to_string());
    }
    // A newly opened file opens its own tab — never over the file already on
    // screen.  Two exceptions reuse the focused pane: an untouched, empty
    // scratch buffer (JetBrains Untitled behavior), and an empty editor pane
    // with no buffer at all — otherwise a bufferless pane would linger
    // underneath the file, swallowing clicks and wheel events.
    let reusable = editor.view_tree.focused_window().and_then(|wid| {
        let pane = editor.view_tree.window(wid)?;
        match pane.buffer_id {
            None => Some((wid, None)),
            Some(buf_key) => {
                let arc = editor
                    .buffers
                    .get(buf_key)?
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                (arc.path.is_none() && !arc.modified() && arc.slice(0, arc.len()).is_empty())
                    .then_some((wid, Some(buf_key)))
            }
        }
    });
    match reusable {
        Some((wid, _old)) => {
            editor.view_tree.set_buffer(wid, key);
            editor.view_tree.focus(wid);
        }
        None => {
            editor.view_tree.open_tab(Some(key));
        }
    }
    editor.events.emit_typed(
        keys::events::BUFFER_CREATED,
        BufferCreatedPayload {
            buffer_id: buf_id.to_string(),
            name,
            path: path.to_string(),
        },
    );
    crate::semantic::fallback_syntax::on_buffer_created(editor, buf_id as usize);
    // The buffer is now the focused pane's buffer.  open-file is the one
    // path that changes focus without a user window switch, so without
    // this the focus event never fires for opened files.
    editor.events.emit_typed(
        keys::events::BUFFER_FOCUSED,
        BufferFocusedPayload {
            buffer_id: buf_id.to_string(),
        },
    );
}

/// Present find-files results (the async event's sink and the sync path
/// share this).
pub fn present_file_search(editor: &mut Editor, pattern: &str, files: Vec<String>) {
    if files.is_empty() {
        editor.show_message(
            crate::state::MessageKind::Info,
            format!("No files match '{pattern}'"),
        );
        return;
    }
    let content = files.join("\n") + "\n";
    let name = format!("*finder:{pattern}*");
    let key = editor.create_buffer_from_str(&name, &content);
    let buf_id = key as u64;
    if let Some(win) = editor.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    editor.events.emit_typed(
        keys::events::FINDER_RESULTS,
        FinderResultsPayload {
            buffer_id: buf_id.to_string(),
            count: files.len().to_string(),
            pattern: pattern.to_string(),
        },
    );
}

pub(super) fn find_files_register(cmds: &mut crate::command::CommandRegistry) {
    cmds.register_fn(
        "find-files",
        "Search files recursively by pattern",
        vec![ArgSpec::optional(
            "pattern",
            ArgType::String,
            ArgValue::String("".to_string()),
        )],
        |editor, args| {
            let raw_pattern = args
                .get("pattern")
                .and_then(|a| a.as_string())
                .unwrap_or("");
            let pattern = raw_pattern.to_lowercase();

            // Project-scoped like every IDE file search: walk the open
            // project's root, falling back to the process cwd.
            let cwd = editor
                .project_manager
                .project
                .root
                .clone()
                .or_else(|| std::env::current_dir().ok())
                .ok_or_else(|| "no project open and no cwd".to_string())?;

            // The walk runs on the pool when one exists; a big tree must
            // never stall a frame.
            if let Some(bg) = editor.background.clone() {
                let sender = bg.sender.clone();
                bg.spawn_blocking(move || {
                    let files = walk_matching(&cwd, &pattern);
                    logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::Project(
                        crate::project::Event::FileSearchListed { pattern, files },
                    )));
                });
                return Ok(());
            }
            present_file_search(editor, &pattern, walk_matching(&cwd, &pattern));
            Ok(())
        },
    );
}

pub(super) fn close_buffer_register(cmds: &mut crate::command::CommandRegistry) {
    cmds.register_fn(
        "close-buffer",
        "Close a buffer and emit buffer-closed (defaults to focused buffer)",
        vec![ArgSpec::optional(
            "buffer-id",
            ArgType::Integer,
            ArgValue::Integer(0),
        )],
        |editor, args| {
            let explicit_key = args
                .get("buffer-id")
                .and_then(|a| a.as_integer())
                .filter(|&k| k > 0)
                .map(|k| k as usize);
            if explicit_key.is_none()
                && let Some(tab_id) = editor.db.active_tab
            {
                editor.db.close_tab(tab_id);
                editor.frame_dirty = true;
                return Ok(());
            }
            let key = explicit_key.unwrap_or_else(|| get_current_buffer_id(editor));

            if !editor.buffers.contains(key) {
                return Ok(());
            }
            editor.events.emit_typed(
                keys::events::BUFFER_CLOSED,
                BufferClosedPayload {
                    buffer_id: key.to_string(),
                },
            );
            editor.drop_buffer(key);
            crate::session::save_if_project(editor);
            let focused_shows_closed = editor
                .view_tree
                .focused_window()
                .and_then(|wid| editor.view_tree.buffer(wid))
                == Some(key);
            if focused_shows_closed {
                let other = editor.buffers.iter().next().map(|(k, _)| k);
                if let Some(new_id) = other {
                    editor.events.emit_typed(
                        keys::events::BUFFER_FOCUSED,
                        BufferFocusedPayload {
                            buffer_id: new_id.to_string(),
                        },
                    );
                }
            }
            Ok(())
        },
    );

    cmds.register_fn(
        "alternate-buffer",
        "Jump to alternate buffer (ctrl-^)",
        vec![],
        |editor, _args| {
            let current = get_current_buffer_id(editor);
            let mut switched_to: Option<usize> = None;
            if let Some(alt_key) = editor
                .alternate_buffer
                .filter(|k| *k != current && editor.buffers.contains(*k))
            {
                editor.alternate_buffer = Some(current);
                if let Some(win) = editor.view_tree.focused_window_mut() {
                    win.buffer_id = Some(alt_key);
                }
                switched_to = Some(alt_key);
            }
            if editor.alternate_buffer.is_none() {
                let others: Vec<usize> = editor
                    .buffers
                    .iter()
                    .map(|(k, _)| k)
                    .filter(|k| *k != current)
                    .collect();
                if let Some(&first_other) = others.first() {
                    editor.alternate_buffer = Some(current);
                    if let Some(win) = editor.view_tree.focused_window_mut() {
                        win.buffer_id = Some(first_other);
                    }
                    switched_to = Some(first_other);
                }
            }
            if let Some(new_id) = switched_to {
                editor.db.active_tab = None;
                editor.events.emit_typed(
                    keys::events::BUFFER_FOCUSED,
                    BufferFocusedPayload {
                        buffer_id: new_id.to_string(),
                    },
                );
            }
            emit_cursor_moved(editor);
            Ok(())
        },
    );
}

/// Save one buffer.  With a background pool the disk write (fsync × 2 in
/// the atomic writer) runs off the UI thread: the buffer is marked clean
/// only when the write-confirmed event lands, and stays in the quit
/// guard's dirty set until then.  Without a pool (tests, embedders) the
/// write is synchronous.
fn save_buffer_common(
    editor: &mut Editor,
    buf_id: usize,
    path_arg: Option<String>,
) -> Result<(), String> {
    let arc = editor
        .buffers
        .get(buf_id)
        .ok_or_else(|| "No current buffer".to_string())?
        .clone();
    let (path, content, saved_rev) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let path = path_arg
            .filter(|p| !p.is_empty())
            .or_else(|| buf.path.clone())
            .ok_or_else(|| "No path specified".to_string())?;
        let content = buf.slice(0, buf.len());
        (path, content, buf.revision())
    };
    editor.events.emit_typed(
        keys::events::BUFFER_BEFORE_SAVE,
        BufferBeforeSavePayload {
            path: path.clone(),
            buffer_id: buf_id.to_string(),
        },
    );

    let Some(bg) = editor.background.clone() else {
        editor
            .fs
            .write(&path, &content)
            .map_err(|e| format!("Save failed: {e}"))?;
        if let Some(arc) = editor.buffers.get(buf_id) {
            arc.lock()
                .unwrap_or_else(|e| e.into_inner())
                .mark_saved_at(saved_rev);
        }
        editor.events.emit_typed(
            keys::events::BUFFER_AFTER_SAVE,
            BufferAfterSavePayload {
                path: path.clone(),
                buffer_id: buf_id.to_string(),
            },
        );
        crate::debug::resync_after_save(editor, &path);
        return Ok(());
    };

    editor.io.saves_in_flight.insert(buf_id, saved_rev);
    let sender = bg.sender.clone();
    bg.spawn_blocking(move || {
        use crate::storage::FileSystem as _;
        let fs = crate::storage::DiskFileSystem::new();
        match fs.write(&path, &content) {
            Ok(()) => {
                logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::SaveWritten {
                    buf_key: buf_id,
                    path,
                    saved_rev,
                }));
            }
            Err(e) => {
                logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::SaveFailed {
                    buf_key: buf_id,
                    error: e.to_string(),
                }));
            }
        }
    });
    Ok(())
}

/// Substring file search under `cwd`, skipping VCS/build directories.
fn walk_matching(cwd: &std::path::Path, pattern: &str) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    let mut dirs = vec![cwd.to_path_buf()];
    let ignore = [
        ".git",
        "node_modules",
        "target",
        ".DS_Store",
        ".zig-cache",
        ".cache",
    ];
    while let Some(dir) = dirs.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => { logging::error!("buffer.rs:541: {e}"); continue }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if ignore.iter().any(|i| path.to_string_lossy().contains(i)) {
                continue;
            }
            // Symlinked dirs are excluded entirely (like the project walk):
            // `is_dir` follows links, and a cycle (`ln -s . loop`) would
            // otherwise spin this blocking walk forever.  Symlinked files
            // still match by their own name.
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => { logging::error!("buffer.rs:554: {e}"); continue }
            };
            if file_type.is_symlink() {
                let rel = path
                    .strip_prefix(cwd)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                if !file_type.is_dir() && (pattern.is_empty() || rel.to_lowercase().contains(pattern))
                {
                    files.push(rel);
                }
                continue;
            }
            if path.is_dir() {
                dirs.push(path);
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(cwd)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                if pattern.is_empty() || rel.to_lowercase().contains(pattern) {
                    files.push(rel);
                }
            }
        }
    }
    files.sort();
    files.dedup();
    files
}
