use crate::backends::{
    parse_branch_lines, parse_porcelain_blame, parse_porcelain_z, parse_stash_lines, short_date,
    split_hunks,
};
use crate::{BlameLine, parse_blame_tsv, serialize_blame};

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

// ── Blame ────────────────────────────────────────────────────────────────────

/// Two commits; the second block reuses the first commit's sha, so its
/// author/date must come from the metadata cache (porcelain only emits
/// metadata on a commit's first block).
const PORCELAIN: &str = concat!(
    "830f5911abcdefabcdefabcdefabcdefabcdef 1 1 2\n",
    "author Mohamed Abdellahi\n",
    "author-mail <m@catosystems.dev>\n",
    "author-time 1792886400\n",
    "author-tz +0200\n",
    "committer Mohamed Abdellahi\n",
    "committer-mail m@catosystems.dev\n",
    "committer-time 1792886400\n",
    "committer-tz +0200\n",
    "summary feat: add workspace skeleton\n",
    "boundary\n",
    "\tline one\n",
    "\tline two\n",
    "5543b0bbef2bfefefeaa0b0bbef2bfefefeaa0b0 2 3 1\n",
    "author Alice\n",
    "author-mail a@x.dev\n",
    "author-time 1692886400\n",
    "author-tz +0100\n",
    "summary fix: later change\n",
    "\tline three\n",
    "830f5911abcdefabcdefabcdefabcdefabcdef 2 4 1\n",
    "\tline two (moved)\n",
);

#[test]
fn porcelain_blame_parses_every_content_line_in_order() {
    let blame = parse_porcelain_blame(PORCELAIN);
    assert_eq!(
        blame.len(),
        4,
        "one record per tab-prefixed line: {blame:?}"
    );
    assert_eq!(blame[0].author, "Mohamed Abdellahi");
    assert_eq!(blame[1].author, "Mohamed Abdellahi");
    assert_eq!(blame[2].author, "Alice");
    // The last block repeats commit #1 with no metadata — cache hit.
    assert_eq!(blame[3].author, "Mohamed Abdellahi");
    assert_eq!(blame[3].date, blame[0].date);
    assert_eq!(blame[0].commit.len(), 8, "short hashes for display");
}

#[test]
fn porcelain_blame_dates_are_short_utc_dates() {
    let blame = parse_porcelain_blame(PORCELAIN);
    assert_eq!(blame[0].date, "2026-10-25");
    assert_eq!(blame[2].date, "2023-08-24");
}

#[test]
fn short_date_handles_epoch_and_leap_day() {
    assert_eq!(short_date(0), "1970-01-01");
    // 2024-02-29 00:00 UTC = 1709164800.
    assert_eq!(short_date(1_709_164_800), "2024-02-29");
}

#[test]
fn blame_tsv_round_trips_through_the_run_op_channel() {
    let lines = vec![
        BlameLine {
            commit: "abc12345".into(),
            author: "Alice".into(),
            date: "2026-10-25".into(),
        },
        BlameLine {
            commit: "def67890".into(),
            author: String::new(),
            date: String::new(),
        },
    ];
    let parsed = parse_blame_tsv(&serialize_blame(&lines));
    assert_eq!(parsed, lines);
}

#[test]
fn blame_annotation_prefers_author_and_date_and_falls_back_to_commit() {
    let full = BlameLine {
        commit: "abc".into(),
        author: "Alice".into(),
        date: "2026-10-25".into(),
    };
    assert_eq!(full.annotation(), "Alice 2026-10-25");
    let no_date = BlameLine {
        commit: "abc".into(),
        author: "Alice".into(),
        date: String::new(),
    };
    assert_eq!(no_date.annotation(), "Alice");
    let no_author = BlameLine {
        commit: "abc".into(),
        author: String::new(),
        date: "2026-10-25".into(),
    };
    assert_eq!(no_author.annotation(), "2026-10-25");
    let bare = BlameLine {
        commit: "abc".into(),
        author: String::new(),
        date: String::new(),
    };
    assert_eq!(bare.annotation(), "abc");
}
