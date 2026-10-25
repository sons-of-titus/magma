//! Side-by-side editor diff — hunk parsing, changed-line marks, and the
//! cursor→hunk resolution behind per-hunk stage/discard.
//!
//! These are the pure functions the two-pane diff is built from: the `@@`
//! headers give both the colour marks and the hunk the cursor addresses.
//! The fixture is a real zero-context patch, so each header's lengths match
//! the lines in its body.

use crate::kernel::command::builtin::vc_hunks::{
    changed_lines, hunk_covers, parse_hunks,
};

/// Two hunks: a replace-with-addition and a pure deletion (no new-side lines).
const PATCH: &str = "\
diff --git a/work.txt b/work.txt
index 111..222 100644
--- a/work.txt
+++ b/work.txt
@@ -2,1 +2,2 @@
-old
+new
+extra
@@ -10,3 +11,0 @@
-gone1
-gone2
-gone3
";

#[test]
fn parse_hunks_reads_ranges() {
    let hunks = parse_hunks(PATCH);
    assert_eq!(hunks.len(), 2);
    assert_eq!((hunks[0].old_start, hunks[0].old_len), (2, 1));
    assert_eq!((hunks[0].new_start, hunks[0].new_len), (2, 2));
    // A pure deletion reports a zero new-side length.
    assert_eq!((hunks[1].old_start, hunks[1].old_len), (10, 3));
    assert_eq!((hunks[1].new_start, hunks[1].new_len), (11, 0));
}

#[test]
fn parse_hunks_single_line_length_is_implicit() {
    let hunks = parse_hunks("@@ -5 +5 @@\n-x\n+y\n");
    assert_eq!(hunks.len(), 1);
    assert_eq!((hunks[0].old_start, hunks[0].old_len), (5, 1));
    assert_eq!((hunks[0].new_start, hunks[0].new_len), (5, 1));
}

#[test]
fn changed_lines_map_to_each_side() {
    let hunks = parse_hunks(PATCH);
    // Left (0-based): hunk 1's removed line, then hunk 2's three deleted lines.
    assert_eq!(changed_lines(&hunks, true), vec![1, 9, 10, 11]);
    // Right (0-based): hunk 1's two added lines; hunk 2 adds nothing.
    assert_eq!(changed_lines(&hunks, false), vec![1, 2]);
}

#[test]
fn hunk_covers_resolves_the_cursor_line() {
    let hunks = parse_hunks(PATCH);
    // Right side: new-side lines 2 and 3 (1-based) → 0-based 1 and 2.
    assert!(hunk_covers(&hunks[0], 1));
    assert!(hunk_covers(&hunks[0], 2));
    assert!(!hunk_covers(&hunks[0], 3), "past the added block");
    // A pure deletion has no new-side line; it matches where its lines were.
    assert!(hunk_covers(&hunks[1], 10), "0-based line 10 is new-side line 11");
    assert!(!hunk_covers(&hunks[1], 11));
}
