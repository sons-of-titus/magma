//! Crash recovery of unsaved buffers (R-16 / F-36).
//!
//! The snapshot format, the manifest, and the leftover-session scan live in
//! the `crash_recovery` crate.  This module opens the survivors as
//! `*recovered …*` buffers and reports the result.

pub use crash_recovery::{
    CrashSnapshot, RecoverableSession, clear_session, collect_from, collect_recoverable,
    drop_snapshot, mark_recovered, snapshot_stem, write_snapshot,
};

use std::path::{Path, PathBuf};

/// Where this run's snapshots live, under the editor's state dir.
pub fn new_session_dir() -> PathBuf {
    crash_recovery::new_session_dir(&crate::kernel::state::persist::state_dir())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("magma-crash-kernel-{tag}-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        root
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
