//! Project session file format — the per-project working state that survives
//! close and reopen: open files with cursor positions, the focused file,
//! breakpoints, and watch expressions.  Persisted to
//! `<root>/.magma/session.json`.
//!
//! This crate owns the shape and the on-disk encoding only.  Deciding what to
//! capture from a live editor, and applying a loaded session back onto one, is
//! the editor's job.

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

pub fn session_file(root: &Path) -> PathBuf {
    root.join(".magma").join("session.json")
}

/// Write the session to `<root>/.magma/session.json`, creating the directory
/// if needed.
pub fn write(root: &Path, state: &SessionState) -> Result<(), String> {
    let path = session_file(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create .magma: {e}"))?;
    }
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&path, text + "\n").map_err(|e| format!("write {}: {e}", path.display()))
}

/// Read the session for `root`.  A missing or unparsable file yields `None` —
/// a damaged session is not an error the caller can act on.
pub fn read(root: &Path) -> Option<SessionState> {
    let text = std::fs::read_to_string(session_file(root)).ok()?;
    serde_json::from_str(&text).ok()
}
