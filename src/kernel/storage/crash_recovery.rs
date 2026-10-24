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
/// Sessions already handed back to the user (`recovered: true` in their
/// manifest) are skipped — recovering twice would let a second crash
/// destroy the only copy of work the user never got to save.
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

/// Open every survivor under `crash_root` as a read-only `*recovered …*`
/// buffer and return how many buffers were restored.  Called once at
/// startup with the real state dir (via `recover_into`); tests pass a
/// scratch root.
pub fn recover_from(ed: &mut crate::kernel::state::Editor, crash_root: &Path) -> usize {
    let mut count = 0usize;
    for session in collect_from(crash_root) {
        for snap in &session.snapshots {
            let label = format!(
                "*recovered {}*",
                snap.label.trim_matches('*')
            );
            let key = ed.create_buffer_from_str(&label, &snap.content);
            if let Some(arc) = ed.buffers.get(key) {
                arc.lock().unwrap_or_else(|e| e.into_inner()).read_only = true;
            }
            count += 1;
        }
        mark_recovered(&session.dir);
    }
    count
}

/// Startup path: recover from the user's real state dir and surface a
/// status message when anything came back.
pub fn recover_into(ed: &mut crate::kernel::state::Editor) {
    let crash_root = crate::kernel::state::persist::state_dir().join("crash");
    let count = recover_from(ed, &crash_root);
    if count > 0 {
        ed.show_message(
            crate::kernel::state::MessageKind::Info,
            format!("recovered {count} unsaved buffer(s) from the last run — see the *recovered* buffers"),
        );
    }
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

    /// A test-owned root: every session dir lives inside it and cleanup
    /// removes only that root.  collect_from() prunes siblings, so tests
    /// must never point it at a shared directory.
    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("magma-crash-test-{tag}-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn snapshot_roundtrip_preserves_content_and_label() {
        let root = temp_root("roundtrip");
        let dir = root.join("s1");
        write_snapshot(&dir, "*scratch*", None, "unsaved work");
        write_snapshot(&dir, "main.rs", Some("/proj/src/main.rs"), "fn main() {}\n");

        let sessions = collect_from(&root);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].snapshots.len(), 2);
        let scratch = sessions[0].snapshots.iter().find(|s| s.label == "*scratch*").unwrap();
        assert_eq!(scratch.content, "unsaved work");
        assert_eq!(scratch.path, None);
        let main = sessions[0].snapshots.iter().find(|s| s.label == "main.rs").unwrap();
        assert_eq!(main.path.as_deref(), Some("/proj/src/main.rs"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rewriting_a_snapshot_replaces_not_duplicates() {
        let root = temp_root("rewrite");
        let dir = root.join("s1");
        write_snapshot(&dir, "*scratch*", None, "v1");
        write_snapshot(&dir, "*scratch*", None, "v2");
        let snapshots = read_snapshots(&dir);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].content, "v2");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn drop_snapshot_removes_only_that_buffer() {
        let root = temp_root("drop");
        let dir = root.join("s1");
        write_snapshot(&dir, "a.rs", None, "aaa");
        write_snapshot(&dir, "b.rs", None, "bbb");
        drop_snapshot(&dir, "a.rs", None);
        let snapshots = read_snapshots(&dir);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].label, "b.rs");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clear_session_leaves_nothing_recoverable() {
        let root = temp_root("clear");
        let dir = root.join("s1");
        write_snapshot(&dir, "a.rs", None, "aaa");
        clear_session(&dir);
        assert!(collect_from(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pruning_keeps_only_the_newest_sessions() {
        let root = temp_root("prune");
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
        let root = temp_root("oversize");
        let dir = root.join("s1");
        let big = "x".repeat(MAX_SNAPSHOT_BYTES + 1);
        write_snapshot(&dir, "big", None, &big);
        assert!(read_snapshots(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&root);
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

    #[test]
    fn recover_opens_read_only_buffers_and_never_twice() {
        let root = temp_root("recover");
        let dir = root.join("s1");
        write_snapshot(&dir, "notes.txt", None, "half-written note");
        let mut ed = crate::kernel::state::Editor::new(
            Box::new(crate::kernel::storage::disk::DiskFileSystem::new()));

        let first = recover_from(&mut ed, &root);
        assert_eq!(first, 1);
        let (name, ro, content) = ed.buffers.iter()
            .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).name.starts_with("*recovered"))
            .map(|(_, a)| {
                let b = a.lock().unwrap_or_else(|e| e.into_inner());
                (b.name.clone(), b.read_only, b.slice(0, b.len()))
            })
            .expect("recovered buffer exists");
        assert!(ro, "recovered buffers open read-only");
        assert_eq!(content, "half-written note");
        assert!(name.contains("notes.txt"), "label preserved: {name}");

        // A second pass must not re-open the same work.
        assert_eq!(recover_from(&mut ed, &root), 0,
            "a recovered session is never recovered twice");
        let _ = std::fs::remove_dir_all(&root);
    }
}
