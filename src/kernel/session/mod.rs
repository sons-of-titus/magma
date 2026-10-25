//! Project sessions — the per-project working state that survives close and
//! reopen: open files with cursor positions, the focused file, breakpoints,
//! and watch expressions.  Persisted to `<root>/.magma/session.json`.
//! Restored automatically when a project opens; saved when a project closes
//! or the editor quits.

use serde::{Deserialize, Serialize};

use crate::kernel::state::Editor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionBuffer {
    pub path: String,
    pub cursor: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionBreakpoint {
    pub file: String,
    pub line: usize,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    #[serde(default)]
    pub buffers: Vec<SessionBuffer>,
    #[serde(default)]
    pub focused: Option<String>,
    #[serde(default)]
    pub breakpoints: Vec<SessionBreakpoint>,
    #[serde(default)]
    pub watches: Vec<String>,
}

pub fn session_file(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".magma").join("session.json")
}

/// Snapshot the per-project working state.  Only file-backed buffers are
/// recorded — scratch buffers are editor-session-local by contract.
pub fn capture(ed: &Editor) -> SessionState {
    let mut buffers: Vec<SessionBuffer> = Vec::new();
    let mut focused: Option<String> = None;

    let focused_key = ed.focused_buffer_key();
    for (key, arc) in ed.buffers.iter() {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = &buf.path else { continue };
        let cursor = ed.views.get(&key)
            .map(|v| v.cursor_offset())
            .unwrap_or(0);
        buffers.push(SessionBuffer { path: path.clone(), cursor });
        if key == focused_key {
            focused = Some(path.clone());
        }
    }

    let breakpoints = ed.debug.breakpoints.values()
        .flat_map(|bps| bps.iter())
        .map(|bp| SessionBreakpoint { file: bp.file.clone(), line: bp.line, enabled: bp.enabled })
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
    let path = session_file(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create .magma: {e}"))?;
    }
    let text = serde_json::to_string_pretty(&capture(ed)).map_err(|e| e.to_string())?;
    std::fs::write(&path, text + "\n").map_err(|e| format!("write {}: {e}", path.display()))
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
    let Ok(text) = std::fs::read_to_string(session_file(root)) else { return };
    let Ok(state) = serde_json::from_str::<SessionState>(&text) else { return };

    for entry in &state.buffers {
        if !std::path::Path::new(&entry.path).exists() {
            continue;
        }
        if open_path(ed, &entry.path)
            && let Some((key, _)) = ed.buffers.iter().find(|(_, arc)| {
                arc.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref() == Some(entry.path.as_str())
            })
                && let Some(view) = ed.views.get_mut(&key) {
                    view.set_cursor(entry.cursor);
                }
    }

    for bp in &state.breakpoints {
        if ed.debug.breakpoints_for(&bp.file).iter().all(|b| b.line != bp.line) {
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

    if let Some(focused) = &state.focused {
        open_path(ed, focused);
    }
}

/// Open a file through the `open-file` command (dedupe + focus semantics).
fn open_path(ed: &mut Editor, path: &str) -> bool {
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        crate::kernel::command::args::ArgValue::Path(path.to_string()));
    crate::kernel::command::execute_command(ed, "open-file", &args).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_flag_defaults_true_for_older_sessions() {
        let state: SessionState = serde_json::from_str(
            r#"{"breakpoints":[{"file":"/a.rs","line":3}]}"#).unwrap();
        assert!(state.breakpoints[0].enabled);
    }
}
