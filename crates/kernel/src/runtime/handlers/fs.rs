//! Filesystem background-event handlers — file reads/writes, external
//! change reloads, async save confirmations, and auto-save.

use crate::event::keys;
use crate::event::payload::*;
use crate::state::Editor;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::FileReadResult {
            path,
            content,
            request_id: _,
        } => match content {
            Ok(text) => {
                ed.events.emit_typed(
                    keys::events::FILE_LOADED,
                    FileLoadedPayload {
                        path,
                        content: text,
                    },
                );
            }
            Err(e) => {
                ed.events.emit_typed(
                    keys::events::FILE_ERROR,
                    FileErrorPayload { path, error: e },
                );
            }
        },
        BackgroundEvent::FileWriteResult {
            path,
            result,
            request_id: _,
        } => match result {
            Ok(()) => {
                ed.events
                    .emit_typed(keys::events::FILE_SAVED, FileSavedPayload { path });
            }
            Err(e) => {
                ed.events.emit_typed(
                    keys::events::FILE_ERROR,
                    FileErrorPayload { path, error: e },
                );
            }
        },
        BackgroundEvent::FileChanged { path, kind } => {
            // Only list-relevant changes mark the index stale for the GUI's
            // debounced rescan — churn in ignored dirs (build output, VCS,
            // tool state) and writes to already-indexed files don't change
            // the file list, and re-walking on each of them re-indexed the
            // project every couple of seconds during any build.
            if let Some(root) = ed.project_manager.project.root.clone()
                && let Some(canonical) = std::fs::canonicalize(&root).ok()
                && path.starts_with(canonical.to_string_lossy().as_ref())
                && crate::project::index_change_marks_dirty(
                    &canonical,
                    &path,
                    &kind,
                    &ed.project_manager.project.files,
                )
            {
                ed.project_manager.project.file_index_dirty = true;
            }
            crate::storage::reload::handle_file_changed(ed, &path, &kind);
            ed.events.emit_typed(
                keys::events::FILE_CHANGED,
                FileChangedPayload { path, kind },
            );
        }
        BackgroundEvent::BufferReloaded { buf_key, content } => {
            crate::storage::reload::apply_reload(ed, buf_key, content);
        }
        BackgroundEvent::SaveWritten {
            buf_key,
            path,
            saved_rev,
        } => {
            ed.io.saves_in_flight.remove(&buf_key);
            let name = ed
                .buffers
                .get(buf_key)
                .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).name.clone());
            if let Some(arc) = ed.buffers.get(buf_key) {
                arc.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .mark_saved_at(saved_rev);
            }
            // The content reached disk — its crash snapshot is obsolete.
            if let Some(dir) = ed.io.crash_dir.clone() {
                let label = name.unwrap_or_else(|| path.clone());
                let dir2 = dir.clone();
                let path2 = path.clone();
                if let Some(bg) = ed.background.as_ref() {
                    bg.spawn_blocking(move || {
                        crate::storage::crash_recovery::drop_snapshot(&dir2, &label, Some(&path2));
                    })
                }
            }
            ed.events.emit_typed(
                keys::events::FILE_SAVED,
                FileSavedPayload { path: path.clone() },
            );
            crate::debug::resync_after_save(ed, &path);
            crate::command::builtin::vc_blame::refresh_blame_after_save(ed, &path);
            if let Some(buf) = ed
                .buffers
                .iter()
                .find(|(_, a)| {
                    a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref()
                        == Some(path.as_str())
                })
                .map(|(k, _)| k)
            {
                crate::inlay_hints::request(ed, buf);
            }
            ed.show_message(crate::state::MessageKind::Info, "saved");
        }
        BackgroundEvent::SaveFailed { buf_key, error } => {
            ed.io.saves_in_flight.remove(&buf_key);
            ed.show_message(
                crate::state::MessageKind::Error,
                format!("save failed: {error}"),
            );
        }
        BackgroundEvent::AutoSaveTick => {
            // The heartbeat doubles as the maintenance tick: persist the
            // project index when it has pending changes.
            if ed.index.is_dirty()
                && let Err(e) = ed.index.flush()
            {
                debug!("index flush failed: {e}");
            }
            crash_snapshot_dirty_buffers(ed);
            // Opt-in only: the editor must never rewrite user files uninvited.
            if ed.options.get("auto-save").map(|v| v.as_str()) != Some("true") {
                return;
            }
            auto_save_dirty_buffers(ed);
        }
        BackgroundEvent::AutoSaveWritten { buf_key, saved_rev } => {
            if let Some(arc) = ed.buffers.get_mut(buf_key) {
                // Only the revision actually written becomes clean; edits
                // made after the snapshot keep the buffer modified
                // (docs/core-redesign.md §5.1).
                arc.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .mark_saved_at(saved_rev);
            }
        }
        _ => {}
    }
}

/// Snapshot every dirty buffer into the crash-recovery dir (R-16).  Unlike
/// auto-save this is unconditional (opt-out via the `crash-recovery`
/// option): it rewrites only the state-dir snapshots, never user files.
fn crash_snapshot_dirty_buffers(ed: &mut Editor) {
    let Some(dir) = ed.io.crash_dir.clone() else {
        return;
    };
    if ed.options.get("crash-recovery").map(|v| v.as_str()) == Some("false") {
        return;
    }
    let dirty: Vec<(String, Option<String>, String)> = ed
        .buffers
        .iter()
        .filter(|(_, arc)| {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            buf.modified() && !buf.ephemeral && !buf.read_only
        })
        .map(|(_key, arc)| {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            (buf.name.clone(), buf.path.clone(), buf.slice(0, buf.len()))
        })
        .collect();
    if dirty.is_empty() {
        return;
    };
    if let Some(bg) = ed.background.clone() {
        bg.spawn_on(
            crate::runtime::Lane::Maintenance,
            Some("crash-snapshot"),
            move || {
                for (name, path, content) in dirty {
                    crate::storage::crash_recovery::write_snapshot(
                        &dir,
                        &name,
                        path.as_deref(),
                        &content,
                    );
                }
            },
        );
    }
}

/// Snapshot every dirty buffer and hand the writes to the background pool.
fn auto_save_dirty_buffers(ed: &mut Editor) {
    let Some(bg) = ed.background.clone() else {
        return;
    };
    let dirty: Vec<(usize, String, String, u64)> = ed
        .buffers
        .iter()
        .filter(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).modified())
        .filter_map(|(key, arc)| {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            buf.path
                .clone()
                .map(|p| (key, p, buf.slice(0, buf.len()), buf.revision()))
        })
        .collect();
    for (key, path, content, saved_rev) in dirty {
        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let result = crate::storage::disk::atomic_write(&path, &content);
            if result.is_ok() {
                let _ = sender.send(BackgroundEvent::AutoSaveWritten {
                    buf_key: key,
                    saved_rev,
                });
            }
        });
    }
}
