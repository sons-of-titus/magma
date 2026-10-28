use super::*;

use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("fs-{tag}-{}-{nanos}", std::process::id()))
}

fn dir_string(dir: &Path) -> String {
    dir.to_string_lossy().into_owned()
}

// ── Sync trait ───────────────────────────────────────────────────────────────

#[test]
fn write_then_read_round_trips() {
    let dir = temp_dir("roundtrip");
    let file = dir.join("a.txt");
    let fs = DiskFileSystem::new();

    fs.write(&dir_string(&file), "hello").unwrap();
    assert_eq!(fs.read(&dir_string(&file)).unwrap(), "hello");
    assert!(fs.exists(&dir_string(&file)));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn write_creates_missing_parent_directories() {
    let dir = temp_dir("parents").join("nested").join("deeper");
    let file = dir.join("a.txt");
    let fs = DiskFileSystem::new();

    fs.write(&dir_string(&file), "x").unwrap();
    assert!(fs.exists(&dir_string(&file)));

    std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).ok();
}

#[test]
fn reading_a_missing_file_reports_not_found() {
    let fs = DiskFileSystem::new();
    let missing = temp_dir("missing");
    match fs.read(&dir_string(&missing.join("nope.txt"))) {
        Err(FsError::NotFound(_)) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn a_missing_file_is_not_reported_as_existing() {
    let fs = DiskFileSystem::new();
    assert!(!fs.exists(&dir_string(&temp_dir("no-exist").join("nope"))));
}

#[test]
fn dir_lists_entries_sorted() {
    let dir = temp_dir("list");
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["c.txt", "a.txt", "b.txt"] {
        std::fs::write(dir.join(name), "").unwrap();
    }

    let entries = DiskFileSystem::new().dir(&dir_string(&dir)).unwrap();
    assert_eq!(entries, vec!["a.txt", "b.txt", "c.txt"]);

    std::fs::remove_dir_all(&dir).ok();
}

// ── Atomic write ─────────────────────────────────────────────────────────────

#[test]
fn atomic_write_replaces_content_and_leaves_no_temp_file() {
    let dir = temp_dir("atomic");
    let file = dir.join("a.txt");

    atomic_write(&dir_string(&file), "first").unwrap();
    atomic_write(&dir_string(&file), "second").unwrap();

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "second");
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("magma-tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

// ── Async trait ──────────────────────────────────────────────────────────────

#[test]
fn async_operations_round_trip() {
    let dir = temp_dir("async");
    let file = dir.join("a.txt");
    let fs = DiskFileSystem::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        fs.write_async(&dir_string(&file), "async body")
            .await
            .unwrap();
        assert_eq!(
            fs.read_async(&dir_string(&file)).await.unwrap(),
            "async body"
        );
        assert!(fs.exists_async(&dir_string(&file)).await);
        let entries = fs.dir_async(&dir_string(&dir)).await.unwrap();
        assert_eq!(entries, vec!["a.txt"]);
    });

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn async_read_of_a_missing_file_reports_not_found() {
    let fs = DiskFileSystem::new();
    let missing = temp_dir("async-missing");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let error = runtime.block_on(fs.read_async(&dir_string(&missing.join("nope.txt"))));
    assert!(matches!(error, Err(FsError::NotFound(_))));
}

#[test]
fn async_exists_is_false_for_a_missing_path() {
    let fs = DiskFileSystem::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let exists = runtime.block_on(fs.exists_async(&dir_string(&temp_dir("async-no-exist"))));
    assert!(!exists);
}

// ── Error mapping ────────────────────────────────────────────────────────────

#[test]
fn io_errors_map_to_the_matching_variant() {
    let not_found = std::io::Error::from(std::io::ErrorKind::NotFound);
    let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    let other = std::io::Error::from(std::io::ErrorKind::BrokenPipe);

    assert!(matches!(FsError::from(not_found), FsError::NotFound(_)));
    assert!(matches!(
        FsError::from(denied),
        FsError::PermissionDenied(_)
    ));
    assert!(matches!(FsError::from(other), FsError::IoError(_)));
}

#[test]
fn errors_display_a_human_readable_message() {
    assert_eq!(FsError::NotFound("x".into()).to_string(), "Not found: x");
    assert_eq!(FsError::IoError("y".into()).to_string(), "IO error: y");
    assert_eq!(
        FsError::PermissionDenied("z".into()).to_string(),
        "Permission denied: z"
    );
}
