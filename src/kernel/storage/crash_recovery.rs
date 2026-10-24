//! Crash recovery of unsaved buffers (R-16 / F-36).
//!
//! The auto-save heartbeat periodically snapshots every modified buffer
//! into a per-session directory under the Magma state dir.  A clean exit
//! removes the directory; a crash leaves it behind.  The next startup
//! scans for leftovers and opens each survivor as a `*recovered …*`
//! buffer — no modal, no data loss, and "recovered" never includes work
//! that made it to disk (save confirmations delete the snapshot).
//!
//! The directory-handling functions take the session dir as a parameter
//! so they are testable without touching the user's real state dir.

use std::path::{Path, PathBuf};

use super::disk::atomic_write;

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
pub fn new_session_dir() -> PathBuf {
    let id = format!(
        "{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        std::process::id(),
    );
    crate::kernel::state::persist::state_dir().join("crash").join(id)
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

/// Scan the state dir for sessions whose owner died without a clean exit.
pub fn collect_recoverable() -> Vec<RecoverableSession> {
    let crash_root = crate::kernel::state::persist::state_dir().join("crash");
    collect_from(&crash_root)
}

fn collect_from(crash_root: &Path) -> Vec<RecoverableSession> {
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
        .map(|(dir, _)| RecoverableSession {
            snapshots: read_snapshots(&dir),
            dir,
        })
        .filter(|s| !s.snapshots.is_empty())
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("magma-crash-test-{tag}-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn snapshot_roundtrip_preserves_content_and_label() {
        let dir = temp_dir("roundtrip");
        write_snapshot(&dir, "*scratch*", None, "unsaved work");
        write_snapshot(&dir, "main.rs", Some("/proj/src/main.rs"), "fn main() {}\n");

        let sessions = collect_from(&dir.parent().unwrap());
        // The parent holds sibling temp dirs from other tests too — filter.
        let ours = sessions.iter().find(|s| s.dir == dir).expect("session found");
        assert_eq!(ours.snapshots.len(), 2);
        let scratch = ours.snapshots.iter().find(|s| s.label == "*scratch*").unwrap();
        assert_eq!(scratch.content, "unsaved work");
        assert_eq!(scratch.path, None);
        let main = ours.snapshots.iter().find(|s| s.label == "main.rs").unwrap();
        assert_eq!(main.path.as_deref(), Some("/proj/src/main.rs"));
        let _ = std::fs::remove_dir_all(&dir.parent().unwrap());
    }

    #[test]
    fn rewriting_a_snapshot_replaces_not_duplicates() {
        let dir = temp_dir("rewrite");
        write_snapshot(&dir, "*scratch*", None, "v1");
        write_snapshot(&dir, "*scratch*", None, "v2");
        let snapshots = read_snapshots(&dir);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].content, "v2");
        let _ = std::fs::remove_dir_all(&dir.parent().unwrap());
    }

    #[test]
    fn drop_snapshot_removes_only_that_buffer() {
        let dir = temp_dir("drop");
        write_snapshot(&dir, "a.rs", None, "aaa");
        write_snapshot(&dir, "b.rs", None, "bbb");
        drop_snapshot(&dir, "a.rs", None);
        let snapshots = read_snapshots(&dir);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].label, "b.rs");
        let _ = std::fs::remove_dir_all(&dir.parent().unwrap());
    }

    #[test]
    fn clear_session_leaves_nothing_recoverable() {
        let root = temp_dir("clear");
        let dir = root.join("s1");
        write_snapshot(&dir, "a.rs", None, "aaa");
        clear_session(&dir);
        assert!(collect_from(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pruning_keeps_only_the_newest_sessions() {
        let root = temp_dir("prune");
        for i in 0..(MAX_SESSIONS + 2) {
            let dir = root.join(format!("s{i}"));
            write_snapshot(&dir, &format!("f{i}"), None, "x");
            // Ensure distinct mtimes even on coarse filesystems.
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let sessions = collect_from(&root);
        assert_eq!(sessions.len(), MAX_SESSIONS, "oldest sessions pruned");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn oversized_snapshots_are_refused() {
        let dir = temp_dir("oversize");
        let big = "x".repeat(MAX_SNAPSHOT_BYTES + 1);
        write_snapshot(&dir, "big", None, &big);
        assert!(read_snapshots(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir.parent().unwrap());
    }

    #[test]
    fn snapshot_stem_is_stable_and_identity_separating() {
        let a1 = snapshot_stem("same", Some("/a"));
        let a2 = snapshot_stem("same", Some("/a"));
        let b = snapshot_stem("same", Some("/b"));
        let c = snapshot_stem("same", None);
        assert_eq!(a1, a2);
        assert_ne!(a1, b);
        assert_ne!(a1, c, "path and name identities never collide");
    }
}
