//! Crash recovery of unsaved buffers (R-16 / F-36).
//!
//! The auto-save heartbeat periodically snapshots every modified buffer
//! into a per-session directory under the Magma state dir.  A clean exit
//! removes the directory; a crash leaves it behind.  The next startup
//! scans for leftovers and opens each survivor as a `*recovered …*`
//! buffer — no modal, no data loss, and "recovered" never includes work
//! that made it to disk (save confirmations delete the snapshot).
//!
//! Every directory-taking function takes the session dir or crash root as a
//! parameter, so tests never touch the user's real state dir.  Opening the
//! survivors as buffers is the editor's job.

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use fs::atomic_write;

/// Sessions retained after pruning (each is one editor run).
const MAX_SESSIONS: usize = 4;
/// Buffers larger than this are not snapshotted — recovery must never
/// turn a crash into a disk-filling loop.
const MAX_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;
const MANIFEST: &str = "manifest.json";

/// One crash survivor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashSnapshot {
    /// Buffer name or file path label, as shown to the user.
    pub label: String,
    /// The file path when the buffer had one.
    pub path: Option<String>,
    /// Recovered text.
    pub content: String,
}

/// Where this run's snapshots live.  Unique per editor process.
pub fn new_session_dir(state_dir: &Path) -> PathBuf {
    let id = format!(
        "{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        std::process::id(),
    );
    state_dir.join("crash").join(id)
}

/// FNV-1a over a buffer's identity — the snapshot filename stem.
pub fn snapshot_stem(name: &str, path: Option<&str>) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in path.unwrap_or(name).as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// Write one buffer's snapshot into `dir`.  Runs on the background pool.
pub fn write_snapshot(dir: &Path, name: &str, path: Option<&str>, content: &str) {
    if content.len() > MAX_SNAPSHOT_BYTES {
        return;
    }
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let stem = snapshot_stem(name, path);
    let _ = atomic_write(dir.join(format!("{stem}.txt")).to_string_lossy().as_ref(), content);
    // Rewrite the manifest with this buffer included.
    let mut entries = read_manifest(dir);
    entries.retain(|e| e["stem"].as_str() != Some(stem.as_str()));
    entries.push(serde_json::json!({
        "stem": stem,
        "label": name,
        "path": path,
    }));
    let json = serde_json::to_string(&entries).unwrap_or_default();
    let _ = atomic_write(dir.join(MANIFEST).to_string_lossy().as_ref(), &json);
}

/// Remove a saved buffer's snapshot — its content reached disk.
pub fn drop_snapshot(dir: &Path, name: &str, path: Option<&str>) {
    let stem = snapshot_stem(name, path);
    let _ = std::fs::remove_file(dir.join(format!("{stem}.txt")));
    let mut entries = read_manifest(dir);
    entries.retain(|e| e["stem"].as_str() != Some(stem.as_str()));
    let json = serde_json::to_string(&entries).unwrap_or_default();
    let _ = atomic_write(dir.join(MANIFEST).to_string_lossy().as_ref(), &json);
}

/// The run ended cleanly: nothing left to recover.
pub fn clear_session(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// A leftover session directory with its recoverable snapshots.
#[derive(Debug, Clone)]
pub struct RecoverableSession {
    pub dir: PathBuf,
    pub snapshots: Vec<CrashSnapshot>,
}

/// Scan `<state_dir>/crash` for sessions whose owner died without a clean
/// exit.
pub fn collect_recoverable(state_dir: &Path) -> Vec<RecoverableSession> {
    collect_from(&state_dir.join("crash"))
}

/// Scan `crash_root` for sessions whose owner died without a clean exit.
/// Sessions already handed back to the user (`recovered: true` in their
/// manifest) are skipped — recovering twice would let a second crash
/// destroy the only copy of work the user never got to save.
pub fn collect_from(crash_root: &Path) -> Vec<RecoverableSession> {
    let Ok(entries) = std::fs::read_dir(crash_root) else { return Vec::new() };
    let mut sessions: Vec<(PathBuf, std::time::SystemTime)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((e.path(), meta.modified().ok()?))
        })
        .collect();
    sessions.sort_by_key(|(_, mtime)| *mtime);
    // Prune while we are here: keep only the newest MAX_SESSIONS dirs so a
    // long run of crashes cannot accumulate unbounded recovery state.
    while sessions.len() > MAX_SESSIONS {
        let (oldest, _) = sessions.remove(0);
        clear_session(&oldest);
    }
    sessions.into_iter()
        .filter(|(dir, _)| !manifest_flag(dir, "recovered"))
        .map(|(dir, _)| RecoverableSession {
            snapshots: read_snapshots(&dir),
            dir,
        })
        .filter(|s| !s.snapshots.is_empty())
        .collect()
}

/// Mark a session as handed back to the user.
pub fn mark_recovered(dir: &Path) {
    let mut entries = read_manifest(dir);
    for e in &mut entries {
        e["recovered"] = serde_json::Value::Bool(true);
    }
    let json = serde_json::to_string(&entries).unwrap_or_default();
    let _ = atomic_write(dir.join(MANIFEST).to_string_lossy().as_ref(), &json);
}

fn manifest_flag(dir: &Path, key: &str) -> bool {
    read_manifest(dir).iter().any(|e| e[key] == serde_json::Value::Bool(true))
}

fn read_snapshots(dir: &Path) -> Vec<CrashSnapshot> {
    read_manifest(dir).into_iter().filter_map(|e| {
        let stem = e["stem"].as_str()?;
        let content = std::fs::read_to_string(dir.join(format!("{stem}.txt"))).ok()?;
        Some(CrashSnapshot {
            label: e["label"].as_str().unwrap_or("buffer").to_string(),
            path: e["path"].as_str().map(String::from),
            content,
        })
    }).collect()
}

fn read_manifest(dir: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join(MANIFEST)).ok()
        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
        .unwrap_or_default()
}
