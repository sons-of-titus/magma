//! Workspace persistence — save and restore editor state to/from disk.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::kernel::state::Editor;

// ── Data model ───────────────────────────────────────────────────────────────

/// A single buffer entry persisted in a workspace snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedBuffer {
    /// Buffer name (shown in the modeline).
    pub name: String,
    /// File path for file-backed buffers; `None` for scratch buffers.
    pub path: Option<String>,
    /// Full content — stored for scratch buffers; file buffers are re-read from disk.
    pub content: Option<String>,
}

/// Serializable snapshot of Editor state suitable for workspace persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceState {
    /// Non-ephemeral buffers open at snapshot time.
    pub buffers: Vec<PersistedBuffer>,
    /// File path of the focused buffer (used to restore focus after reload).
    pub focused_buffer: Option<String>,
    /// Command-mode history entries (most-recent-last).
    pub command_history: Vec<String>,
    /// Last active search pattern.
    pub search_pattern: Option<String>,
    /// Global editor options (`editor.options`).
    pub options: HashMap<String, String>,
    /// Named registers (`editor.registers`).
    pub registers: HashMap<String, String>,
    /// Plugin key-value state (`editor.plugin_state`).
    pub plugin_state: HashMap<String, String>,
}

// ── WorkspaceManager ─────────────────────────────────────────────────────────

/// Manages workspace persistence: snapshot, save, restore, and named sessions.
///
/// All public methods are stateless; they operate on the `Editor` passed in.
pub struct WorkspaceManager;

impl WorkspaceManager {
    /// Default workspace directory (`<magma-state-dir>/workspace/`).
    pub fn workspace_dir() -> PathBuf {
        crate::kernel::state::persist::state_dir().join("workspace")
    }

    /// Build a `WorkspaceState` snapshot from the current editor.
    pub fn snapshot(editor: &Editor) -> WorkspaceState {
        let mut buffers = Vec::new();
        for (_key, arc) in &editor.buffers {
            let buf = arc.lock().unwrap();
            if buf.ephemeral {
                continue;
            }
            let content = buf.path.as_ref().map_or_else(
                || Some(buf.slice(0, buf.len())),
                |_| None,
            );
            buffers.push(PersistedBuffer {
                name: buf.name.clone(),
                path: buf.path.clone(),
                content,
            });
        }
        let focused_buffer = editor
            .focused_view()
            .and_then(|v| v.buffer.lock().ok())
            .and_then(|b| b.path.clone());
        WorkspaceState {
            buffers,
            focused_buffer,
            command_history: editor.command_history.clone(),
            search_pattern: editor.search_pattern.clone(),
            options: editor.options.clone(),
            registers: editor.registers.clone(),
            plugin_state: editor.plugin_state.clone(),
        }
    }

    /// Apply a `WorkspaceState` to the editor — creates buffers and restores
    /// scalar state.  Returns the count of buffers successfully recreated.
    pub fn apply(editor: &mut Editor, state: &WorkspaceState) -> Result<usize, String> {
        let mut count = 0usize;
        for pb in &state.buffers {
            if let Some(ref file_path) = pb.path {
                match std::fs::read_to_string(file_path) {
                    Ok(content) => {
                        let key = editor.create_buffer_from_str(&pb.name, &content);
                        if let Some(arc) = editor.buffers.get(key) {
                            arc.lock().unwrap().path = Some(file_path.clone());
                        }
                        count += 1;
                    }
                    Err(_) => {}
                }
            } else if let Some(ref content) = pb.content {
                editor.create_buffer_from_str(&pb.name, content);
                count += 1;
            }
        }
        editor.command_history = state.command_history.clone();
        if state.search_pattern.is_some() {
            editor.search_pattern = state.search_pattern.clone();
        }
        for (k, v) in &state.options {
            editor.options.insert(k.clone(), v.clone());
        }
        for (k, v) in &state.registers {
            editor.registers.insert(k.clone(), v.clone());
        }
        for (k, v) in &state.plugin_state {
            editor.plugin_state.insert(k.clone(), v.clone());
        }
        Ok(count)
    }

    // ── Save ─────────────────────────────────────────────────────────────────

    /// Save the current workspace to `dir/workspace.json`.
    pub fn save_to(editor: &Editor, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let state = Self::snapshot(editor);
        let json = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("workspace.json"), &json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Save the current workspace to the default workspace directory.
    pub fn save(editor: &Editor) -> Result<(), String> {
        Self::save_to(editor, &Self::workspace_dir())
    }

    // ── Restore ──────────────────────────────────────────────────────────────

    /// Restore workspace state from `dir/workspace.json`.
    ///
    /// Returns the number of buffers recreated.
    pub fn restore_from(editor: &mut Editor, dir: &Path) -> Result<usize, String> {
        let path = dir.join("workspace.json");
        let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let state: WorkspaceState = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        Self::apply(editor, &state)
    }

    /// Restore workspace state from the default workspace directory.
    pub fn restore(editor: &mut Editor) -> Result<usize, String> {
        Self::restore_from(editor, &Self::workspace_dir())
    }

    // ── Named sessions ───────────────────────────────────────────────────────

    fn session_filename(name: &str) -> String {
        let safe: String = name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect();
        format!("{safe}.json")
    }

    /// Save the current editor state as named session `name` inside `sessions_dir`.
    pub fn session_save_to(editor: &Editor, name: &str, sessions_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(sessions_dir).map_err(|e| e.to_string())?;
        let state = Self::snapshot(editor);
        let json = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
        std::fs::write(sessions_dir.join(Self::session_filename(name)), &json)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Save the current editor state as named session `name` (default path).
    pub fn session_save(editor: &Editor, name: &str) -> Result<(), String> {
        let dir = Self::workspace_dir().join("sessions");
        Self::session_save_to(editor, name, &dir)
    }

    /// Load named session `name` from `sessions_dir` and apply it to the editor.
    ///
    /// Returns the number of buffers recreated.
    pub fn session_load_from(
        editor: &mut Editor,
        name: &str,
        sessions_dir: &Path,
    ) -> Result<usize, String> {
        let path = sessions_dir.join(Self::session_filename(name));
        let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let state: WorkspaceState = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        Self::apply(editor, &state)
    }

    /// Load named session `name` and apply it to the editor (default path).
    pub fn session_load(editor: &mut Editor, name: &str) -> Result<usize, String> {
        let dir = Self::workspace_dir().join("sessions");
        Self::session_load_from(editor, name, &dir)
    }

    /// List named sessions available in `sessions_dir`.
    pub fn session_list_from(sessions_dir: &Path) -> Vec<String> {
        match std::fs::read_dir(sessions_dir) {
            Ok(entries) => entries
                .filter_map(|e| {
                    let e = e.ok()?;
                    let fname = e.file_name().to_string_lossy().to_string();
                    fname.strip_suffix(".json").map(|s| s.to_string())
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List named sessions available in the default sessions directory.
    pub fn session_list() -> Vec<String> {
        let dir = Self::workspace_dir().join("sessions");
        Self::session_list_from(&dir)
    }
}
