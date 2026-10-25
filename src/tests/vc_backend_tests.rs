use crate::kernel::vc::backends::{parse_porcelain_z, split_hunks, GitBackend, ShellTemplateBackend};
use crate::kernel::vc::VcBackend;
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

// ── Side-by-side editor diff ─────────────────────────────────────────────────

#[test]
fn vc_open_diff_opens_two_linked_read_only_panes() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff test");
        return;
    }
    let (root, file) = scratch_git_repo("diff");
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nline6\n").unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    // `vc` detects the repo and syncs state (no background pool → synchronous).
    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();

    let view = ed.vc.diff_view.clone().expect("the diff view must open");
    assert!(ed.view_tree.pane(view.left).is_some() && ed.view_tree.pane(view.right).is_some(),
        "both diff panes must exist");
    assert_eq!(ed.view_tree.panes().len(), 2, "the editor is split in two");

    // Left = HEAD, right = working tree, both read-only.
    let left = ed.buffers.get(view.left_buf).unwrap().lock().unwrap();
    let (left_text, left_ro) = (left.slice(0, left.len()), left.read_only);
    drop(left);
    let right = ed.buffers.get(view.right_buf).unwrap().lock().unwrap();
    let (right_text, right_ro) = (right.slice(0, right.len()), right.read_only);
    drop(right);
    assert!(left_ro && right_ro, "both diff panes are read-only");
    assert!(left_text.contains("line2"), "the left pane shows the committed version");
    assert!(right_text.contains("CHANGED2"), "the right pane shows the working tree");

    // The two halves scroll together.
    let link = ed.view_tree.pane(view.left).unwrap().scroll_link;
    assert!(link.is_some());
    assert_eq!(ed.view_tree.pane(view.right).unwrap().scroll_link, link,
        "both halves share one scroll link");

    // Hunk actions are buffer-local to the worktree pane — no global layer.
    assert_eq!(ed.keymaps.resolve_for_buffer("x", Some(view.right_buf as u64)).as_deref(),
        Some("vc-discard-hunk"));
    assert!(!ed.keymaps.is_layer_active("vc"),
        "opening the diff must not activate a global vc layer");
    assert!(ed.gutter.sign_cache.keys().any(|(name, buf, _)| name == ":vc-diff" && *buf == view.right_buf),
        "the worktree pane carries gutter marks");

    // Closing restores the pre-split layout and drops nothing else.
    crate::kernel::command::builtin::vc_diff::close_diff(&mut ed);
    assert!(ed.vc.diff_view.is_none(), "the diff view is closed");
    assert_eq!(ed.view_tree.panes().len(), 1, "back to a single editor pane");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn vc_open_diff_is_one_tab_and_hands_other_tabs_back() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff-tab test");
        return;
    }
    let (root, file) = scratch_git_repo("difftab");
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nline6\n").unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    // A second file open in its own tab, so the diff has something to conceal.
    let other = ed.create_buffer_from_str("other.txt", "keep me\n");
    ed.view_tree.open_tab(Some(other));

    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();

    let view = ed.vc.diff_view.clone().expect("diff view");
    // Only the two diff halves carry a buffer; the other tab is concealed.
    let showing: Vec<_> = ed.view_tree.panes().iter()
        .filter(|p| p.buffer_id.is_some()).map(|p| p.id).collect();
    assert_eq!(showing.len(), 2, "only the two diff halves render");
    assert!(ed.view_tree.panes().iter()
        .any(|p| p.id != view.left && p.id != view.right && p.buffer_id.is_none()),
        "the other tab is concealed, not dropped");

    // The pair reads as one tab, labelled `{file}:HEAD->TREE`.
    let tabs = crate::kernel::render::gui_tabs::TabBar::collect_tabs(&ed);
    assert_eq!(tabs.len(), 1, "the diff is a single tab");
    assert_eq!(tabs[0].label, "work.txt:HEAD->TREE");

    // Closing hands the concealed tab back, buffer and all.
    crate::kernel::command::builtin::vc_diff::close_diff(&mut ed);
    assert!(ed.view_tree.panes().iter().any(|p| p.buffer_id == Some(other)),
        "the concealed tab returns on close");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn vc_open_diff_highlights_changed_lines() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff-highlight test");
        return;
    }
    let (root, file) = scratch_git_repo("diffhl");
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nline6\n").unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();
    let view = ed.vc.diff_view.clone().expect("diff view");

    for (buf, needle, face) in [
        (view.right_buf, "CHANGED2", "diff-add-face"),
        (view.left_buf, "line2", "diff-del-face"),
    ] {
        let text = {
            let arc = ed.buffers.get(buf).unwrap();
            let b = arc.lock().unwrap_or_else(|e| e.into_inner());
            b.slice(0, b.len())
        };
        let at = text.find(needle).expect("needle in pane") as usize;
        let bview = ed.views.get(&buf).expect("diff pane view");
        let layer = bview.highlight_layers.get("vc-diff")
            .unwrap_or_else(|| panic!("the vc-diff highlight layer must exist"));
        assert!(layer.iter().any(|(s, e, f)| *s <= at && at < *e && f == face),
            "the changed line at byte {at} must be covered by {face}: {layer:?}");
        assert!(ed.faces.contains_key(face), "{face} must be a registered face");
    }

    // The band reaches the painted surface: the changed row's cells carry the
    // face, so the line reads as removed/added, not plain text.
    ed.view_tree.resize(120, 20);
    let mut surface = crate::kernel::render::surface::Surface::new(120, 20);
    crate::kernel::render::frame::render_frame(&ed, &mut surface);
    for (pane_id, face) in [(view.left, "diff-del-face"), (view.right, "diff-add-face")] {
        let pane = ed.view_tree.pane(pane_id).unwrap();
        let style = *ed.faces.get(face).unwrap();
        let painted = (pane.x..pane.x + pane.width)
            .flat_map(|x| (pane.y..pane.y + pane.height).map(move |y| (x, y)))
            .any(|(x, y)| surface.cell(x, y).map(|c| c.style == style).unwrap_or(false));
        assert!(painted, "{face} must paint the changed row on the surface");
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn vc_diff_panes_take_syntax_from_the_source_path() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff-syntax test");
        return;
    }
    let (root, file) = scratch_git_repo("diffsyn");
    std::fs::write(&file, "line1\nCHANGED2\nline3\nline4\nline5\nline6\n").unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();
    let view = ed.vc.diff_view.clone().expect("diff view");

    for buf in [view.left_buf, view.right_buf] {
        let arc = ed.buffers.get(buf).unwrap();
        let b = arc.lock().unwrap_or_else(|e| e.into_inner());
        assert!(b.path.is_none(),
            "a diff pane must not claim the file — :w would overwrite it");
        assert_eq!(b.language_path.as_deref(), Some("work.txt"),
            "the pane's syntax must be inferred from the source path");
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn vc_diff_renders_when_the_two_sides_differ_in_length() {
    let git_ok = crate::kernel::vc::run_cmd(Path::new("."), "git", &["--version"]).is_ok();
    if !git_ok {
        eprintln!("git unavailable; skipping diff-length test");
        return;
    }
    let (root, file) = scratch_git_repo("difflen");
    // The working tree grows far past HEAD, so the linked scroll mirrors a
    // top line the shorter HEAD pane does not have.
    let long: String = (0..120).map(|i| format!("line {i}\n")).collect();
    std::fs::write(&file, &long).unwrap();

    let mut ed = crate::tests::helpers::make_editor();
    let focused = crate::tests::helpers::focused_key(&ed);
    ed.buffers.get(focused).unwrap().lock().unwrap().path = Some(file);

    crate::tests::helpers::run(&mut ed, "vc");
    crate::kernel::command::builtin::vc_diff::spawn_diff(&mut ed, "work.txt".to_string()).unwrap();
    let view = ed.vc.diff_view.clone().expect("diff view");

    let end = {
        let arc = ed.buffers.get(view.right_buf).unwrap();
        let b = arc.lock().unwrap_or_else(|e| e.into_inner());
        b.len()
    };
    ed.views.get_mut(&view.right_buf).unwrap().set_cursor(end);
    ed.view_tree.resize(80, 6);
    crate::kernel::render::frame::sync_viewport_scroll(&mut ed);

    // Rendering the pinned, shorter pane must not underflow its line count.
    let mut surface = crate::kernel::render::surface::Surface::new(80, 6);
    crate::kernel::render::frame::render_frame(&ed, &mut surface);

    let _ = std::fs::remove_dir_all(&root);
}
