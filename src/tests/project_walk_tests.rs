//! Project file walk — gitignore rules, symlinks, and cycles.

#[test]
#[cfg(unix)]
fn walk_respects_gitignore_symlinks_and_cycles() {
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join("magma_walk_gitignore");
    let _ = std::fs::remove_dir_all(&root);
    for d in ["src/deep", "logs", "build", "vendor"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(
        root.join(".gitignore"),
        "*.log\nbuild/\nvendor\n!keep.log\nsrc/legacy\n",
    )
    .unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(root.join("src/deep/mod.rs"), "").unwrap();
    std::fs::write(root.join("logs/app.log"), "").unwrap();
    std::fs::write(root.join("keep.log"), "").unwrap();
    std::fs::write(root.join("build/artifact.o"), "").unwrap();
    std::fs::write(root.join("vendor/lib.c"), "").unwrap();
    std::fs::create_dir_all(root.join("src/legacy")).unwrap();
    std::fs::write(root.join("src/legacy/old.rs"), "").unwrap();

    symlink(root.join("src"), root.join("src/loop")).unwrap();
    symlink("/etc", root.join("etc-link")).unwrap();

    let files = crate::kernel::project::walk_files_for_test(&root);

    assert!(
        files.iter().any(|f| f == "src/main.rs"),
        "normal files index: {files:?}"
    );
    assert!(files.iter().any(|f| f == "src/deep/mod.rs"));
    assert!(
        files.iter().any(|f| f == "keep.log"),
        "!negation restores files"
    );
    assert!(
        !files.iter().any(|f| f.ends_with("app.log")),
        "*.log ignored"
    );
    assert!(
        !files.iter().any(|f| f.starts_with("build/")),
        "build/ ignored"
    );
    assert!(
        !files.iter().any(|f| f.starts_with("vendor")),
        "vendor ignored"
    );
    assert!(
        !files.iter().any(|f| f.starts_with("src/legacy")),
        "anchored dir rule ignored"
    );
    assert!(
        !files.iter().any(|f| f.contains("loop")),
        "symlinks are not traversed"
    );
    assert!(
        !files.iter().any(|f| f.contains("etc-link")),
        "escape symlinks skipped"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn atomic_write_temp_files_never_mark_the_index_dirty() {
    use kernel::project::index_change_marks_dirty;
    let root = std::path::Path::new("/tmp/magma-walk-test");
    let indexed = std::collections::HashSet::from(["src/main.rs".to_string(), "src/lib.rs".to_string()]);

    // The editor's own atomic-write temp file: saving an indexed file must
    // not re-walk the project.
    let temp = format!(
        "{}/src/main.rs.magma-tmp-{}",
        root.display(),
        std::process::id()
    );
    assert!(
        !index_change_marks_dirty(root, &temp, "modified", &indexed),
        "the editor's own temp file is invisible to the index"
    );
    assert!(
        !index_change_marks_dirty(root, &temp, "created", &indexed),
        "temp files never become index entries"
    );

    // Real writes to an indexed file change no list either.
    assert!(!index_change_marks_dirty(
        root,
        &format!("{}/src/main.rs", root.display()),
        "modified",
        &indexed
    ));
    // Creates, removals, and unknown paths still do.
    assert!(index_change_marks_dirty(
        root,
        &format!("{}/src/new.rs", root.display()),
        "created",
        &indexed
    ));
    assert!(index_change_marks_dirty(
        root,
        &format!("{}/src/main.rs", root.display()),
        "removed",
        &indexed
    ));
}

#[test]
#[cfg(unix)]
fn find_files_survives_a_symlink_cycle_in_the_tree() {
    // `walk_matching` used `path.is_dir()` (follows links) with no cycle
    // guard: one `ln -s . loop` spun the blocking walk forever.  The sync
    // path (no background pool in this editor) must return.
    use crate::kernel::command;
    use crate::tests::helpers;
    use std::collections::HashMap;

    let dir = std::env::temp_dir().join("magma_findfiles_cycle");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
    std::os::unix::fs::symlink(&dir, dir.join("loop")).unwrap();

    let mut ed = helpers::make_editor();
    ed.project_manager.project.root = Some(dir.to_string_lossy().into_owned().into());

    let mut args = HashMap::new();
    args.insert(
        "pattern".to_string(),
        kernel::command::args::ArgValue::String("main".to_string()),
    );
    let result = command::execute_command(&mut ed, "find-files", &args);
    assert!(result.is_ok(), "find-files must not hang or error on a cycle");
    // Results land in the *finder:<pattern>* buffer.
    let finder = ed
        .buffers
        .iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).name == "*finder:main*")
        .map(|(_, a)| {
            let b = a.lock().unwrap_or_else(|e| e.into_inner());
            b.slice(0, b.len())
        })
        .unwrap_or_default();
    assert!(
        finder.contains("src/main.rs"),
        "the real file is still found, got {finder:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
