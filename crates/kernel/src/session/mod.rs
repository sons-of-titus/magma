//! Project sessions — the per-project working state that survives close and
//! reopen: open files with cursor positions, the focused file, breakpoints,
//! and watch expressions.  Persisted to `<root>/.magma/session.json`.
//!
//! The file format lives in the `session` crate; this module captures it from
//! `Editor`, applies it back, and decides when to save.

pub use session::{SessionBreakpoint, SessionBuffer, SessionState, session_file};

use crate::state::Editor;

/// Snapshot the per-project working state.  Only file-backed buffers
/// currently shown in a pane are recorded — scratch buffers are
/// editor-session-local by contract, and a buffer whose tab was closed must
/// not resurrect on the next launch.
pub fn capture(ed: &Editor) -> SessionState {
    let mut buffers: Vec<SessionBuffer> = Vec::new();
    let mut focused: Option<String> = None;

    let open_tabs: std::collections::HashSet<usize> = ed
        .view_tree
        .panes()
        .iter()
        .filter_map(|pane| pane.buffer_id)
        .collect();
    let focused_key = ed.focused_buffer_key();
    for (key, arc) in ed.buffers.iter() {
        if !open_tabs.contains(&key) {
            continue;
        }
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = &buf.path else { continue };
        let cursor = ed.views.get(&key).map(|v| v.cursor_offset()).unwrap_or(0);
        buffers.push(SessionBuffer {
            path: path.clone(),
            cursor,
        });
        if key == focused_key {
            focused = Some(path.clone());
        }
    }

    let breakpoints = ed
        .debug
        .breakpoints
        .values()
        .flat_map(|bps| bps.iter())
        .map(|bp| SessionBreakpoint {
            file: bp.file.clone(),
            line: bp.line,
            enabled: bp.enabled,
        })
        .collect();

    SessionState {
        buffers,
        focused,
        breakpoints,
        watches: ed.debug.watches.clone(),
    }
}

/// Write the snapshot to `<root>/.magma/session.json`.
pub fn save(ed: &Editor, root: &std::path::Path) -> Result<(), String> {
    session::write(root, &capture(ed))
}

/// Save when a project is active; a no-op without one.
pub fn save_if_project(ed: &Editor) {
    if let Some(root) = ed.project_manager.project.root.clone() {
        let _ = save(ed, &root);
    }
}

/// Reopen the recorded files with their cursors, breakpoints, and watches.
/// Missing files are skipped; the recorded focus wins at the end.
pub fn restore(ed: &mut Editor, root: &std::path::Path) {
    let Some(state) = session::read(root) else {
        return;
    };

    for entry in &state.buffers {
        if !std::path::Path::new(&entry.path).exists() {
            continue;
        }
        if open_path(ed, &entry.path)
            && let Some((key, _)) = ed.buffers.iter().find(|(_, arc)| {
                arc.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .path
                    .as_deref()
                    == Some(entry.path.as_str())
            })
            && let Some(view) = ed.views.get_mut(&key)
        {
            view.set_cursor(entry.cursor);
        }
    }

    apply_debug_state(ed, &state);

    if let Some(focused) = &state.focused {
        open_path(ed, focused);
    }
}

/// Restore without making the foreground wait on disk: breakpoints and
/// watches apply immediately (in-memory), the recorded buffers load on the
/// pool and stream back as `SessionBufferLoaded` events, in session order.
/// The shell — and whatever the user is already doing — never waits for a
/// session file to read.
pub fn restore_async(ed: &mut Editor, root: &std::path::Path) {
    let Some(state) = session::read(root) else {
        return;
    };
    apply_debug_state(ed, &state);
    let Some(bg) = ed.background.clone() else {
        restore(ed, root);
        return;
    };
    let generation = ed.project_manager.project.generation;
    let sender = bg.sender.clone();
    ed.session_restore_pending = state.buffers.len();
    let focused = state.focused.clone();
    debug!(
        "session: {} recorded buffers queued for background restore",
        state.buffers.len()
    );
    // Coalesced: when projects are switched faster than reads complete, the
    // queued restore of the older project skips itself.
    bg.spawn_on(crate::runtime::Lane::Maintenance, Some("session-restore"), move || {
        for entry in state.buffers {
            let content = std::fs::read_to_string(&entry.path).map_err(|e| e.to_string());
            let _ = sender.send(crate::runtime::BackgroundEvent::SessionBufferLoaded {
                generation,
                path: entry.path,
                cursor: entry.cursor,
                content,
            });
        }
        let _ = sender.send(crate::runtime::BackgroundEvent::SessionRestoreDone {
            generation,
            focused,
        });
    });
}

/// Breakpoints and watches are in-memory per-project state — refill them as
/// soon as the session file is read, before any buffer lands.
fn apply_debug_state(ed: &mut Editor, state: &SessionState) {
    for bp in &state.breakpoints {
        if ed
            .debug
            .breakpoints_for(&bp.file)
            .iter()
            .all(|b| b.line != bp.line)
        {
            ed.debug.add_breakpoint(bp.file.clone(), bp.line);
            if !bp.enabled {
                ed.debug.set_enabled(&bp.file, bp.line, false);
            }
        }
    }
    for watch in &state.watches {
        if !ed.debug.watches.contains(watch) {
            ed.debug.watches.push(watch.clone());
        }
    }
}

/// Open a file through the `open-file` command (dedupe + focus semantics).
pub(crate) fn open_path(ed: &mut Editor, path: &str) -> bool {
    let mut args = std::collections::HashMap::new();
    args.insert(
        "path".to_string(),
        crate::command::args::ArgValue::Path(path.to_string()),
    );
    crate::command::execute_command(ed, "open-file", &args).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_flag_defaults_true_for_older_sessions() {
        let state: SessionState =
            serde_json::from_str(r#"{"breakpoints":[{"file":"/a.rs","line":3}]}"#).unwrap();
        assert!(state.breakpoints[0].enabled);
    }
}
