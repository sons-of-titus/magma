//! Project file walk — gitignore rules, symlinks, and cycles.

#[test]
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
