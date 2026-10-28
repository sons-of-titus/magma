use super::*;

/// A test-owned root: every session dir lives inside it and cleanup
/// removes only that root.  collect_from() prunes siblings, so tests
/// must never point it at a shared directory.
fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "magma-crash-test-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
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
    let scratch = sessions[0]
        .snapshots
        .iter()
        .find(|s| s.label == "*scratch*")
        .unwrap();
    assert_eq!(scratch.content, "unsaved work");
    assert_eq!(scratch.path, None);
    let main = sessions[0]
        .snapshots
        .iter()
        .find(|s| s.label == "main.rs")
        .unwrap();
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
fn a_marked_session_is_not_collected_again() {
    let root = temp_root("recovered");
    let dir = root.join("s1");
    write_snapshot(&dir, "a.rs", None, "aaa");
    mark_recovered(&dir);

    assert!(collect_from(&root).is_empty());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn collect_recoverable_reads_the_crash_root_under_the_given_state_dir() {
    let root = temp_root("state");
    let state = root.join("state");
    let session = state.join("crash").join("s1");
    write_snapshot(&session, "notes.txt", None, "hello");

    let sessions = collect_recoverable(&state);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].dir, session);
    assert_eq!(sessions[0].snapshots[0].content, "hello");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn new_session_dir_lives_under_the_given_state_dir() {
    let dir = new_session_dir(Path::new("/state"));
    assert!(dir.starts_with("/state/crash"));
    assert!(
        dir.file_name()
            .is_some_and(|n| n.to_string_lossy().contains('-'))
    );
    assert!(new_session_dir(Path::new("/other")).starts_with("/other/crash"));
}
