//! Workspace lifecycle — file watching, debounced re-index, and session
//! persistence.
//!
//! These are frontend-neutral responsibilities.  They used to live in the
//! egui shell, which meant a second frontend (or a headless host) could not
//! reuse them.  The runtime owns only watcher bookkeeping; every mutation
//! still happens through `Editor` and the command surface.

use std::time::{Duration, Instant};

use crate::state::Editor;
use crate::storage::watcher::FileWatcher;

const INDEX_DEBOUNCE: Duration = Duration::from_secs(2);

pub struct WorkspaceRuntime {
    file_watcher: Option<FileWatcher>,
    /// Canonical project root currently under recursive watch.
    watched_project: Option<String>,
    /// When `index-project` was last requested — debounces rescan storms.
    last_index_request: Option<Instant>,
}

impl WorkspaceRuntime {
    pub fn new(ed: &Editor) -> Self {
        let file_watcher = ed
            .background
            .as_ref()
            .and_then(|bg| FileWatcher::new(bg.sender.clone()).ok())
            .map(|mut w| {
                for (_, arc) in ed.buffers.iter() {
                    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(ref path) = buf.path {
                        w.watch(path);
                    }
                }
                w
            });
        Self {
            file_watcher,
            watched_project: None,
            last_index_request: None,
        }
    }

    /// Watch the project root on first sight and rescan the index when the
    /// file watcher marks it dirty (debounced).
    pub fn maintain(&mut self, ed: &mut Editor) {
        // Files opened from outside the project root have no recursive watch
        // covering them — keep every open buffer path watched so external
        // changes still reach the reload path.
        if let Some(w) = self.file_watcher.as_mut() {
            let paths: Vec<String> = ed
                .buffers
                .iter()
                .filter_map(|(_, arc)| arc.lock().unwrap_or_else(|e| e.into_inner()).path.clone())
                .collect();
            for path in paths {
                if !w.is_watched(&path) {
                    w.watch(&path);
                }
            }
        }

        let Some(root) = ed.project_manager.project.root.clone() else {
            if self.watched_project.take().is_some()
                && let Some(w) = self.file_watcher.as_mut()
            {
                w.retain_project_roots("");
                w.clear_watched_files();
            }
            return;
        };
        let root_str = root.to_string_lossy().into_owned();
        if self.watched_project.as_deref() != Some(root_str.as_str()) {
            if let Some(w) = self.file_watcher.as_mut() {
                // The outgoing project's recursive root watch and per-file
                // watches must not keep feeding events after a switch.
                w.retain_project_roots(&root_str);
                w.clear_watched_files();
                if let Err(e) = w.watch_project(&root_str) {
                    ed.show_message(
                        crate::state::MessageKind::Error,
                        format!("file watching unavailable: {e} — the project tree will not refresh automatically"),
                    );
                }
            }
            self.watched_project = Some(root_str);
        }

        if ed.project_manager.project.file_index_dirty
            && self
                .last_index_request
                .is_none_or(|t| t.elapsed() >= INDEX_DEBOUNCE)
        {
            let _ = crate::command::execute_command(
                ed,
                "index-project",
                &std::collections::HashMap::new(),
            );
            self.last_index_request = Some(Instant::now());
        }
    }
}

/// Teardown when the editor session goes away (quit, guarded close): the
/// active project ends with it and terminal sessions are killed.  Without
/// this the welcome gate would immediately re-open the editor instead of
/// returning to the welcome flow.
pub fn close_editor_session(ed: &mut Editor) {
    crate::session::save_if_project(ed);
    // The outgoing project's files must not still be on screen when the
    // launcher hands the editor to a different project.
    crate::project::close_all_buffers(ed);
    if ed.project_manager.project.root.take().is_some() {
        ed.project_manager.current_project = None;
        ed.events.emit_typed(
            crate::event::keys::events::PROJECT_CLOSED,
            crate::event::payload::EmptyPayload,
        );
    }
    ed.io.terminals.close_all();
    ed.debug.disconnect_all();
    logging::LogErr::log_err(ed.index.flush());
}

/// Persist history + marks on shutdown; the project session joins them when
/// a project is open.  A clean exit also discards this run's crash-recovery
/// snapshots — nothing unsaved remains (R-16).
pub fn persist_session(ed: &Editor) {
    crate::state::persist::save_session(ed);
    crate::state::persist::save_settings(ed);
    crate::session::save_if_project(ed);
    if let Some(dir) = ed.io.crash_dir.clone() {
        // Synchronous: the process exits right after this, and a detached
        // thread loses that race — leaving "recovered" buffers the user
        // explicitly discarded by quitting clean.
        crate::storage::crash_recovery::clear_session(&dir);
    }
}
