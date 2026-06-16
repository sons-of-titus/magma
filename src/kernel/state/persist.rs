/// Persistent state — save/load command history, marks, and session data
/// to/from `~/.magma/`.
use std::path::PathBuf;

/// Return the Magma state directory (~/.magma), creating it if needed.
pub fn state_dir() -> PathBuf {
    let dir = dirs_data_dir().join("magma");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn dirs_data_dir() -> PathBuf {
    // Follow XDG or fall back to ~/.magma
    if let Ok(dir) = std::env::var("MAGMA_DIR") {
        return PathBuf::from(dir);
    }
    xdg_data_home()
        .or_else(home_fallback)
        .unwrap_or_else(|| PathBuf::from("~/.magma"))
}

#[cfg(target_os = "macos")]
fn home_fallback() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
}

#[cfg(not(target_os = "macos"))]
fn home_fallback() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".local").join("share"))
}

fn xdg_data_home() -> Option<PathBuf> {
    std::env::var("XDG_DATA_HOME").ok().map(PathBuf::from)
}

// ── Command-line history ─────────────────────────────────────────────

const HISTORY_FILE: &str = "history";

/// Load command history from `~/.magma/history`. Returns an empty vec on
/// first run or if the file is missing/corrupt.
pub fn load_command_history() -> Vec<String> {
    let path = state_dir().join(HISTORY_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => content
            .lines()
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Save command history to `~/.magma/history` (last 500 entries).
pub fn save_command_history(history: &[String]) {
    let path = state_dir().join(HISTORY_FILE);
    let max = history.len().min(500);
    let content = history[history.len() - max..]
        .join("\n");
    let _ = std::fs::write(&path, &content);
}

// ── Marks persistence ────────────────────────────────────────────────

const MARKS_FILE: &str = "marks.json";

/// Persistent representation of a named mark.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedMark {
    pub name: char,
    pub buffer_id: u64,
    pub byte_offset: usize,
}

/// Load persisted marks from `~/.magma/marks.json`.
pub fn load_marks() -> Vec<PersistedMark> {
    let path = state_dir().join(MARKS_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Save marks to `~/.magma/marks.json`.
pub fn save_marks(marks: &[PersistedMark]) {
    let path = state_dir().join(MARKS_FILE);
    if let Ok(content) = serde_json::to_string(marks) {
        let _ = std::fs::write(&path, &content);
    }
}
