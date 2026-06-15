use super::*;
use crate::state::id::BufferId;

fn buf(content: &str) -> Buffer {
    Buffer::from_string(BufferId(1), "test", content)
}

fn empty() -> Buffer { Buffer::new(BufferId(1), "test") }

// ── Basic ops ─────────────────────────────────────────────────────────

#[test]
fn new_buffer_is_empty() {
    let b = empty();
    assert_eq!(b.len(), 0);
    assert!(b.is_empty());
    assert_eq!(b.cursor(), 0);
}

#[test]
fn insert_appends_text_and_advances_cursor() {
    let mut b = empty();
    b.insert(0, "hello");
    assert_eq!(b.slice(0, b.len()), "hello");
    assert_eq!(b.cursor(), 5);
}

#[test]
fn insert_at_cursor_advances_correctly() {
    let mut b = buf("world");
    b.set_cursor(0);
    b.insert(b.cursor(), "hello ");
    assert_eq!(b.slice(0, b.len()), "hello world");
    assert_eq!(b.cursor(), 6);
}

#[test]
fn insert_multibyte_advances_by_byte_length() {
    let mut b = empty();
    b.insert(0, "é"); // 2 bytes in UTF-8
    assert_eq!(b.len(), 2);
    assert_eq!(b.cursor(), 2);
    assert_eq!(b.slice(0, b.len()), "é");
}

#[test]
fn delete_removes_range_and_adjusts_cursor() {
    let mut b = buf("hello world");
    b.set_cursor(5);
    b.delete(5, 6); // remove space
    assert_eq!(b.slice(0, b.len()), "helloworld");
    assert_eq!(b.cursor(), 5);
}

#[test]
fn delete_before_cursor_adjusts_cursor_back() {
    let mut b = buf("hello");
    b.set_cursor(5);
    b.delete(0, 2); // remove "he"
    assert_eq!(b.slice(0, b.len()), "llo");
    assert_eq!(b.cursor(), 3);
}

#[test]
fn delete_multibyte_char() {
    let mut b = buf("héllo"); // 'é' at bytes 1-2
    b.set_cursor(0);
    b.delete(1, 3); // delete 'é'
    assert_eq!(b.slice(0, b.len()), "hllo");
}

// ── Cursor ────────────────────────────────────────────────────────────

#[test]
fn set_cursor_clamps_to_len() {
    let mut b = buf("hi");
    b.set_cursor(100);
    assert_eq!(b.cursor(), 2); // len = 2
}

#[test]
fn set_cursor_on_empty_buffer_stays_zero() {
    let mut b = empty();
    b.set_cursor(5);
    assert_eq!(b.cursor(), 0);
}

#[test]
fn cursor_does_not_go_past_end() {
    let mut b = buf("abc");
    b.set_cursor(3);
    assert_eq!(b.cursor(), 3); // allowed to sit at len
    b.set_cursor(4);
    assert_eq!(b.cursor(), 3); // clamped
}

// ── Undo / redo ───────────────────────────────────────────────────────

#[test]
fn undo_reverts_insert() {
    let mut b = empty();
    b.insert(0, "hello");
    assert!(b.undo());
    assert_eq!(b.slice(0, b.len()), "");
}

#[test]
fn undo_then_redo_restores() {
    let mut b = empty();
    b.insert(0, "hello");
    b.undo();
    assert!(b.redo());
    assert_eq!(b.slice(0, b.len()), "hello");
}

#[test]
fn undo_on_empty_history_returns_false() {
    let mut b = empty();
    assert!(!b.undo());
}

#[test]
fn redo_on_no_future_returns_false() {
    let mut b = empty();
    b.insert(0, "x");
    assert!(!b.redo()); // nothing to redo yet
}

#[test]
fn undo_multiple_steps() {
    let mut b = empty();
    b.insert(0, "a");
    b.insert(1, "b");
    let before = b.slice(0, b.len());
    b.undo();
    let after = b.slice(0, b.len());
    assert_ne!(before, after);
}

// ── char_at ───────────────────────────────────────────────────────────

#[test]
fn char_at_ascii() {
    let b = buf("hello");
    assert_eq!(b.char_at(0), Some('h'));
    assert_eq!(b.char_at(4), Some('o'));
    assert_eq!(b.char_at(5), None);
}

#[test]
fn char_at_multibyte() {
    let b = buf("héllo");
    assert_eq!(b.char_at(0), Some('h'));
    assert_eq!(b.char_at(1), Some('é')); // 'é' starts at byte 1
    assert_eq!(b.char_at(3), Some('l')); // after 'é' (2 bytes)
}

// ── Line ops ──────────────────────────────────────────────────────────

#[test]
fn line_count_multiline() {
    let b = buf("a\nb\nc");
    assert_eq!(b.line_count(), 3);
}

#[test]
fn line_retrieval() {
    let b = buf("first\nsecond\nthird");
    assert_eq!(b.line(0), Some("first".to_string()));
    assert_eq!(b.line(1), Some("second".to_string()));
    assert_eq!(b.line(2), Some("third".to_string()));
    assert_eq!(b.line(3), None);
}
