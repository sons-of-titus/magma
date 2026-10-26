use crate::backends::{parse_branch_lines, parse_porcelain_z, parse_stash_lines, split_hunks};

#[test]
fn porcelain_z_handles_renames_and_unicode() {
    // -z records: `XY <path>\0`; renames follow with the ORIGINAL path as
    // its own NUL field, which must be consumed, not treated as a record.
    let raw = "R  new_name.rs\0old_name.rs\0 M src/ünicode.rs\0?? scratch dir\0";
    let (staged, unstaged, untracked, _conflicts) = parse_porcelain_z(raw);
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].code, 'R');
    assert_eq!(
        staged[0].path, "new_name.rs",
        "rename entries carry the destination path"
    );
    assert_eq!(unstaged.len(), 1);
    assert_eq!(
        unstaged[0].path, "src/ünicode.rs",
        "unicode paths pass through unquoted"
    );
    assert_eq!(untracked, vec!["scratch dir".to_string()]);
}

#[test]
fn porcelain_z_ignores_empty_trailing_record() {
    let (staged, _u, _t, _c) = parse_porcelain_z("A  ok.rs\0");
    assert_eq!(staged.len(), 1);
    let (staged, _u, _t, _c) = parse_porcelain_z("A  ok.rs\0\0");
    assert_eq!(
        staged.len(),
        1,
        "trailing NUL never becomes a phantom entry"
    );
}

// ── F-23: conflicts, branches, stash, hunks ──────────────────────────────────

#[test]
fn conflict_pairs_land_in_conflicts_not_staged_or_unstaged() {
    let (staged, unstaged, untracked, conflicts) =
        parse_porcelain_z("UU both.rs\0AA both-added.rs\0DU deleted-by-them.rs\0");
    assert_eq!(
        conflicts,
        vec!["both.rs", "both-added.rs", "deleted-by-them.rs"]
    );
    assert!(
        staged.is_empty() && unstaged.is_empty(),
        "a conflicted path must not be double-listed"
    );
    assert_eq!(untracked.len(), 0);
    // The normal modification still parses alongside conflicts.
    let (staged, unstaged, _, conflicts2) = parse_porcelain_z("UU both.rs\0 M normal.rs\0");
    assert_eq!(conflicts2, vec!["both.rs"]);
    assert_eq!(staged.len(), 0);
    assert_eq!(unstaged.len(), 1);
}

#[test]
fn branch_and_stash_parsers() {
    let branches = parse_branch_lines("* main\n  feature/x\n  remotes-only-is-skipped");
    assert_eq!(branches.len(), 3);
    assert_eq!(
        branches[0],
        crate::BranchInfo {
            name: "main".into(),
            current: true
        }
    );
    assert_eq!(branches[1].name, "feature/x");
    assert!(!branches[1].current);

    let stashes =
        parse_stash_lines("stash@{0}: WIP on main: abc123 fix\nstash@{1}: On dev: experiment\n");
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
        assert!(
            p.starts_with("diff --git a/f.rs b/f.rs"),
            "each patch carries the file header: {p}"
        );
        assert!(p.matches("@@").count() >= 2, "exactly one hunk header");
    }
    let hunk_starts = patches
        .iter()
        .map(|p| p.matches("\n@@").count() + p.starts_with("@@") as usize)
        .sum::<usize>();
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
