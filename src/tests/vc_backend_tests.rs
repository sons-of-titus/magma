use crate::kernel::vc::backends::{parse_porcelain_z, split_hunks, GitBackend, ShellTemplateBackend};
use crate::kernel::vc::{VcBackend, VcStatus};
use std::path::Path;

#[test]
fn shell_template_detect_with_marker() {
    let tmp = std::env::temp_dir().join("magma_vc_shell_detect");
    let sub = tmp.join("subdir").join("deep");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(tmp.join(".magma_test_marker"), "").unwrap();

    let backend = ShellTemplateBackend {
        name: "test-vcs".to_string(),
        detect_marker: ".magma_test_marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };

    assert!(backend.detect(&sub), "should detect from deep subdirectory");
    assert!(backend.detect(&tmp), "should detect from root");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn shell_template_detect_without_marker() {
    let tmp = std::env::temp_dir().join("magma_vc_no_marker");
    std::fs::create_dir_all(&tmp).unwrap();

    let backend = ShellTemplateBackend {
        name: "test-vcs".to_string(),
        detect_marker: ".nonexistent_marker_xyz".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };

    assert!(!backend.detect(&tmp));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn shell_template_name() {
    let backend = ShellTemplateBackend {
        name: "fossil".to_string(),
        detect_marker: ".fslckout".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    assert_eq!(backend.name(), "fossil");
}

#[test]
fn shell_template_status_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "fossil".to_string(),
        detect_marker: ".fslckout".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir().join("magma_vc_no_status_cmd");
    std::fs::create_dir_all(&tmp).unwrap();
    let r = backend.status(&tmp);
    assert!(r.is_err());
    let err = r.unwrap_err();
    assert!(err.contains("no status-cmd"), "error should mention missing cmd: {err}");
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn shell_template_diff_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "fossil".to_string(),
        detect_marker: ".fslckout".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.diff(&tmp, Some("file.txt"));
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no diff-cmd"));
}

#[test]
fn shell_template_root_returns_none_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    assert!(backend.root(&tmp).is_none());
}

#[test]
fn shell_template_push_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.push(&tmp);
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no push-cmd"));
}

#[test]
fn shell_template_stage_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.stage(&tmp, "f");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("staging not supported"));
}

#[test]
fn shell_template_unstage_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.unstage(&tmp, "f");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("unstaging not supported"));
}

#[test]
fn shell_template_commit_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.commit(&tmp, "msg");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no commit-cmd"));
}

#[test]
fn shell_template_log_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.log(&tmp, 10);
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no log-cmd"));
}

#[test]
fn shell_template_blame_errors_when_no_cmd() {
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: None,
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let tmp = std::env::temp_dir();
    let r = backend.blame(&tmp, "f");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("no blame-cmd"));
}

#[test]
fn template_values_are_shell_quoted_not_executed() {
    // A commit message containing command-substitution syntax must arrive
    // at the child as literal data.  Before quoting, this ran `touch`
    // as a side effect of the commit.
    let dir = std::env::temp_dir().join("magma_vc_inject");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sentinel = dir.join("pwned");
    let backend = ShellTemplateBackend {
        name: "test".to_string(),
        detect_marker: ".marker".to_string(),
        root_cmd: None,
        status_cmd: None,
        diff_cmd: None,
        log_cmd: None,
        stage_cmd: None,
        unstage_cmd: None,
        commit_cmd: Some(format!("echo {{msg}} > {}/out", dir.to_string_lossy())),
        push_cmd: None,
        blame_cmd: None,
        has_staging: false,
    };
    let hostile = format!("$(touch {}); `id` | grep root", sentinel.to_string_lossy());
    backend.commit(&dir, &hostile).unwrap();

    let out = std::fs::read_to_string(dir.join("out")).unwrap();
    assert_eq!(out.trim(), hostile,
        "the message must be echoed verbatim, never interpreted");
    assert!(!sentinel.exists(), "command substitution must not execute");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn porcelain_z_handles_renames_and_unicode() {
    use crate::kernel::vc::backends::parse_porcelain_z;
    // -z records: `XY <path>\0`; renames follow with the ORIGINAL path as
    // its own NUL field, which must be consumed, not treated as a record.
    let raw = "R  new_name.rs\0old_name.rs\0 M src/ünicode.rs\0?? scratch dir\0";
    let (staged, unstaged, untracked, _conflicts) = parse_porcelain_z(raw);
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].code, 'R');
    assert_eq!(staged[0].path, "new_name.rs", "rename entries carry the destination path");
    assert_eq!(unstaged.len(), 1);
    assert_eq!(unstaged[0].path, "src/ünicode.rs", "unicode paths pass through unquoted");
    assert_eq!(untracked, vec!["scratch dir".to_string()]);
}

#[test]
fn porcelain_z_ignores_empty_trailing_record() {
    use crate::kernel::vc::backends::parse_porcelain_z;
    let (staged, _u, _t, _c) = parse_porcelain_z("A  ok.rs\0");
    assert_eq!(staged.len(), 1);
    let (staged, _u, _t, _c) = parse_porcelain_z("A  ok.rs\0\0");
    assert_eq!(staged.len(), 1, "trailing NUL never becomes a phantom entry");
}

// ── F-23: conflicts, branches, stash, hunks ──────────────────────────────────

#[test]
fn conflict_pairs_land_in_conflicts_not_staged_or_unstaged() {
    let (staged, unstaged, untracked, conflicts) =
        parse_porcelain_z("UU both.rs\0AA both-added.rs\0DU deleted-by-them.rs\0");
    assert_eq!(conflicts, vec!["both.rs", "both-added.rs", "deleted-by-them.rs"]);
    assert!(staged.is_empty() && unstaged.is_empty(),
        "a conflicted path must not be double-listed");
    assert_eq!(untracked.len(), 0);
    // The normal modification still parses alongside conflicts.
    let (staged, unstaged, _, conflicts2) =
        parse_porcelain_z("UU both.rs\0 M normal.rs\0");
    assert_eq!(conflicts2, vec!["both.rs"]);
    assert_eq!(staged.len(), 0);
    assert_eq!(unstaged.len(), 1);
}

#[test]
fn conflict_row_appears_once_and_maps_to_its_path() {
    use crate::kernel::vc::{build_display, path_at_line};
    let status = VcStatus {
        backend_name: "git".into(),
        branch: "main".into(),
        has_staging: true,
        staged: vec![],
        unstaged: vec![],
        untracked: vec![],
        conflicts: vec!["src/merge.rs".to_string()],
    };
    let root = Path::new("/repo");
    let text = build_display(root, &status);
    assert!(text.contains("Conflicts"), "conflicts get their own section:\n{text}");
    assert!(text.contains("src/merge.rs"));
    // Line 3 is the first entry row (header, blank, label).
    assert_eq!(path_at_line(&status, 3).as_deref(), Some("src/merge.rs"));
}

#[test]
fn branch_and_stash_parsers() {
    use crate::kernel::vc::backends::{parse_branch_lines, parse_stash_lines};
    let branches = parse_branch_lines("* main\n  feature/x\n  remotes-only-is-skipped");
    assert_eq!(branches.len(), 3);
    assert_eq!(branches[0], crate::kernel::vc::BranchInfo { name: "main".into(), current: true });
    assert_eq!(branches[1].name, "feature/x");
    assert!(!branches[1].current);

    let stashes = parse_stash_lines(
        "stash@{0}: WIP on main: abc123 fix\nstash@{1}: On dev: experiment\n");
    assert_eq!(stashes.len(), 2);
    assert_eq!(stashes[0].index, 0);
    assert_eq!(stashes[0].message, "WIP on main: abc123 fix");
    assert_eq!(stashes[1].index, 1);
    assert_eq!(stashes[1].message, "On dev: experiment");
}

#[test]
fn diff_splits_into_selfcontained_single_hunk_patches() {
    let diff = "\
diff --git a/f.rs b/f.rs
index 1111111..2222222 100644
--- a/f.rs
+++ b/f.rs
@@ -1,3 +1,4 @@
 one
+two
 three
@@ -10,2 +11,3 @@
 ten
+eleven
 twelve";
    let patches = split_hunks(diff);
    assert_eq!(patches.len(), 2);
    for p in &patches {
        assert!(p.starts_with("diff --git a/f.rs b/f.rs"),
            "each patch carries the file header: {p}");
        assert!(p.matches("@@").count() >= 2, "exactly one hunk header");
    }
    let hunk_starts = patches.iter().map(|p| p.matches("\n@@").count() + p.starts_with("@@") as usize).sum::<usize>();
    assert_eq!(hunk_starts, 2);
    assert!(patches[0].contains("+two") && !patches[0].contains("+eleven"));
    assert!(patches[1].contains("+eleven") && !patches[1].contains("+two"));
}

#[test]
fn hunks_without_diff_header_only_carry_their_own_body() {
    let diff = "@@ -1,2 +1,3 @@\n a\n+b\n c";
    let patches = split_hunks(diff);
    assert_eq!(patches.len(), 1);
    assert!(patches[0].starts_with("@@"));
}

// ── Live-git verification of the F-23 operations ─────────────────────────────

/// A throwaway git repo with one committed file.  Returns (root, file path).
fn scratch_git_repo(tag: &str) -> (std::path::PathBuf, String) {
    let root = std::env::temp_dir().join(format!(
        "magma-vc-f23-{tag}-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("work.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\nline6\n").unwrap();
    run_git(&root, &["init", "-q"]);
    run_git(&root, &["-c", "user.email=t@t", "-c", "user.name=t", "add", "."]);
    run_git(&root, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "initial"]);
    (root, file.to_string_lossy().into_owned())
}

fn run_git(root: &Path, args: &[&str]) -> String {
    crate::kernel::vc::run_cmd(root, "git", args).unwrap_or_default()
}

#[test]
fn live_git_branch_amend_stash_and_hunk_staging_round_trip() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
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
    assert!(staged_text.contains(&"work.txt"), "hunk staging marks the file staged");
    // The staged patch contains only the second hunk's change.
    let staged_diff = run_git(&root, &["diff", "--cached", "--", "work.txt"]);
    assert!(staged_diff.contains("CHANGED6"));
    assert!(!staged_diff.contains("CHANGED2"), "only the chosen hunk was staged");

    // ── amend: rewrite the commit message (the staged hunk folds in) ──
    b.amend(&root, Some("amended title")).unwrap();
    let log = run_git(&root, &["log", "-1", "--format=%s"]);
    assert_eq!(log.trim(), "amended title");
    let head = run_git(&root, &["show", "--format=", "--unified=0", "HEAD"]);
    assert!(head.contains("CHANGED6"), "the staged hunk joined the amended commit");
    assert!(!head.contains("CHANGED2"), "the unstaged hunk stayed out");
    assert!(std::fs::read_to_string(&file).unwrap().contains("CHANGED2"),
        "the worktree keeps the unstaged edit");

    // ── stash: push, list, apply, drop ──
    std::fs::write(&file, "line1\nline2\nWIP\nline4\nline5\nline6\n").unwrap();
    b.stash_push(&root, Some("wip note")).unwrap();
    let stashes = b.stash_list(&root).unwrap();
    assert_eq!(stashes.len(), 1);
    assert!(stashes[0].message.contains("wip note"));
    let after_stash = std::fs::read_to_string(&file).unwrap();
    assert!(after_stash.contains("line2"), "stash restored the committed state");

    b.stash_apply(&root, 0, false).unwrap();
    assert!(std::fs::read_to_string(&file).unwrap().contains("WIP"),
        "apply brings the stashed edit back and keeps the stash");
    b.stash_drop(&root, 0).unwrap();
    assert_eq!(b.stash_list(&root).unwrap().len(), 0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn live_git_unresolved_conflict_appears_in_conflicts_and_resolve_stages() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping live conflict test");
        return;
    }
    let (root, file) = scratch_git_repo("conflict");
    let b = GitBackend;

    // Branch feature diverges on the same line, then both branches change it.
    b.checkout(&root, "feature", true).unwrap();
    std::fs::write(&file, "line1\nfrom-feature\nline3\nline4\nline5\nline6\n").unwrap();
    run_git(&root, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-aqm", "feature edit"]);
    b.checkout(&root, "main", false).unwrap();
    std::fs::write(&file, "line1\nfrom-main\nline3\nline4\nline5\nline6\n").unwrap();
    run_git(&root, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-aqm", "main edit"]);

    run_git(&root, &["merge", "feature"]); // expected to fail with a conflict
    let status = b.status(&root).unwrap();
    assert!(status.conflicts.iter().any(|p| p == "work.txt"),
        "the conflicted file is listed once under conflicts: {:?}", status.conflicts);

    // Resolution = edit + stage.
    std::fs::write(&file, "line1\nmerged\nline3\nline4\nline5\nline6\n").unwrap();
    b.stage(&root, "work.txt").unwrap();
    let status = b.status(&root).unwrap();
    assert!(status.conflicts.is_empty(), "staging clears the conflict");
    assert!(status.staged.iter().any(|f| f.path == "work.txt"));
    let _ = std::fs::remove_dir_all(&root);
}
