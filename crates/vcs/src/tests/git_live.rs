use crate::VcBackend;
use crate::backends::{GitBackend, split_hunks};
use std::path::Path;

// ── Live-git verification of the F-23 operations ─────────────────────────────

/// A throwaway git repo with one committed file.  Returns (root, file path).
fn scratch_git_repo(tag: &str) -> (std::path::PathBuf, String) {
    let root = std::env::temp_dir().join(format!(
        "magma-vc-f23-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("work.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\nline6\n").unwrap();
    // Pin the initial branch: without a platform `init.defaultBranch` the
    // default is still `master`, and the tests below walk `main`.
    run_git(&root, &["init", "-q", "-b", "main"]);
    run_git(
        &root,
        &["-c", "user.email=t@t", "-c", "user.name=t", "add", "."],
    );
    run_git(
        &root,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "initial",
        ],
    );
    (root, file.to_string_lossy().into_owned())
}

fn run_git(root: &Path, args: &[&str]) -> String {
    crate::run_cmd(root, "git", args).unwrap_or_default()
}

#[test]
fn live_git_branch_amend_stash_and_hunk_staging_round_trip() {
    let git_ok = crate::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping live F-23 test");
        return;
    }

    // ── branches: list, create, checkout ──
    let (root, file) = scratch_git_repo("all");
    let b = GitBackend;
    let branches = b.list_branches(&root).unwrap();
    assert_eq!(branches.len(), 1);
    assert!(branches[0].current);

    b.checkout(&root, "feature", true).unwrap();
    let branches = b.list_branches(&root).unwrap();
    assert_eq!(branches.len(), 2);
    let feature = branches.iter().find(|x| x.name == "feature").unwrap();
    assert!(feature.current, "checkout -b switched to the new branch");
    assert!(!branches.iter().find(|x| x.name == "main").unwrap().current);

    // ── hunks: two changed regions → stage only the second ──
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nCHANGED6\n").unwrap();
    let diff = b.hunk_diff(&root, "work.txt").unwrap();
    let patches = split_hunks(&diff);
    assert_eq!(patches.len(), 2, "two separated edits make two hunks");
    b.stage_patch(&root, &patches[1]).unwrap();

    let status = b.status(&root).unwrap();
    let staged_text: Vec<&str> = status.staged.iter().map(|f| f.path.as_str()).collect();
    assert!(
        staged_text.contains(&"work.txt"),
        "hunk staging marks the file staged"
    );
    // The staged patch contains only the second hunk's change.
    let staged_diff = run_git(&root, &["diff", "--cached", "--", "work.txt"]);
    assert!(staged_diff.contains("CHANGED6"));
    assert!(
        !staged_diff.contains("CHANGED2"),
        "only the chosen hunk was staged"
    );

    // ── amend: rewrite the commit message (the staged hunk folds in) ──
    b.amend(&root, Some("amended title")).unwrap();
    let log = run_git(&root, &["log", "-1", "--format=%s"]);
    assert_eq!(log.trim(), "amended title");
    let head = run_git(&root, &["show", "--format=", "--unified=0", "HEAD"]);
    assert!(
        head.contains("CHANGED6"),
        "the staged hunk joined the amended commit"
    );
    assert!(!head.contains("CHANGED2"), "the unstaged hunk stayed out");
    assert!(
        std::fs::read_to_string(&file).unwrap().contains("CHANGED2"),
        "the worktree keeps the unstaged edit"
    );

    // ── stash: push, list, apply, drop ──
    std::fs::write(&file, "line1\nline2\nWIP\nline4\nline5\nline6\n").unwrap();
    b.stash_push(&root, Some("wip note")).unwrap();
    let stashes = b.stash_list(&root).unwrap();
    assert_eq!(stashes.len(), 1);
    assert!(stashes[0].message.contains("wip note"));
    let after_stash = std::fs::read_to_string(&file).unwrap();
    assert!(
        after_stash.contains("line2"),
        "stash restored the committed state"
    );

    b.stash_apply(&root, 0, false).unwrap();
    assert!(
        std::fs::read_to_string(&file).unwrap().contains("WIP"),
        "apply brings the stashed edit back and keeps the stash"
    );
    b.stash_drop(&root, 0).unwrap();
    assert_eq!(b.stash_list(&root).unwrap().len(), 0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn live_git_unresolved_conflict_appears_in_conflicts_and_resolve_stages() {
    let git_ok = crate::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping live conflict test");
        return;
    }
    let (root, file) = scratch_git_repo("conflict");
    let b = GitBackend;

    // Branch feature diverges on the same line, then both branches change it.
    b.checkout(&root, "feature", true).unwrap();
    std::fs::write(&file, "line1\nfrom-feature\nline3\nline4\nline5\nline6\n").unwrap();
    run_git(
        &root,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-aqm",
            "feature edit",
        ],
    );
    b.checkout(&root, "main", false).unwrap();
    std::fs::write(&file, "line1\nfrom-main\nline3\nline4\nline5\nline6\n").unwrap();
    run_git(
        &root,
        &[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-aqm",
            "main edit",
        ],
    );

    run_git(&root, &["merge", "feature"]); // expected to fail with a conflict
    let status = b.status(&root).unwrap();
    assert!(
        status.conflicts.iter().any(|p| p == "work.txt"),
        "the conflicted file is listed once under conflicts: {:?}",
        status.conflicts
    );

    // Resolution = edit + stage.
    std::fs::write(&file, "line1\nmerged\nline3\nline4\nline5\nline6\n").unwrap();
    b.stage(&root, "work.txt").unwrap();
    let status = b.status(&root).unwrap();
    assert!(status.conflicts.is_empty(), "staging clears the conflict");
    assert!(status.staged.iter().any(|f| f.path == "work.txt"));
    let _ = std::fs::remove_dir_all(&root);
}
