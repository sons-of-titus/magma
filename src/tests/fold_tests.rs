use crate::kernel::text_engine::Buffer;
use crate::kernel::state::id::BufferId;

fn buf(content: &str) -> Buffer {
    Buffer::from_string(BufferId(1), "test", content)
}

#[test]
fn new_buffer_has_no_folds() {
    let b = buf("line1\nline2\nline3\n");
    assert!(b.folds.is_empty());
}

#[test]
fn add_fold_stores_range() {
    let mut b = buf("line1\nline2\nline3\n");
    b.add_fold(0, 12);
    assert_eq!(b.folds.len(), 1);
    assert_eq!(b.folds[0], (0, 12));
}

#[test]
fn remove_fold_removes_matching_range() {
    let mut b = buf("line1\nline2\nline3\n");
    b.add_fold(0, 12);
    b.remove_fold(0, 12);
    assert!(b.folds.is_empty());
}

#[test]
fn remove_fold_only_matching() {
    let mut b = buf("line1\nline2\nline3\n");
    b.add_fold(0, 6);
    b.add_fold(12, 18);
    b.remove_fold(0, 6);
    assert_eq!(b.folds.len(), 1);
    assert_eq!(b.folds[0], (12, 18));
}

#[test]
fn clear_folds_removes_all() {
    let mut b = buf("line1\nline2\nline3\n");
    b.add_fold(0, 6);
    b.add_fold(12, 18);
    b.clear_folds();
    assert!(b.folds.is_empty());
}

#[test]
fn is_folded_returns_true_within_fold() {
    let mut b = buf("line1\nline2\nline3\n");
    b.add_fold(0, 12);
    assert!(b.is_folded(0, 6));
    assert!(!b.is_folded(12, 18));
}

#[test]
fn folds_are_sorted_by_start() {
    let mut b = buf("line1\nline2\nline3\n");
    b.add_fold(12, 18);
    b.add_fold(0, 6);
    assert_eq!(b.folds[0].0, 0);
    assert_eq!(b.folds[1].0, 12);
}
