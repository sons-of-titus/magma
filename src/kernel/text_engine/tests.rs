use super::*;
use crate::kernel::state::id::BufferId;
use std::sync::{Arc, Mutex};

fn buf_view(content: &str) -> BufferView {
    let buf = Buffer::from_string(BufferId(1), "test", content);
    BufferView::new(Arc::new(Mutex::new(buf)))
}

fn empty_view() -> BufferView {
    let buf = Buffer::new(BufferId(1), "test");
    BufferView::new(Arc::new(Mutex::new(buf)))
}

// ── Basic ops ─────────────────────────────────────────────────────────

#[test]
fn new_buffer_is_empty() {
    let view = empty_view();
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
    drop(buf);
    assert_eq!(view.cursor_offset(), 0);
}

#[test]
fn insert_appends_text_and_advances_cursor() {
    let mut view = empty_view();
    view.insert(0, "hello");
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.slice(0, buf.len()), "hello");
    drop(buf);
    assert_eq!(view.cursor_offset(), 5);
}

#[test]
fn insert_at_cursor_advances_correctly() {
    let mut view = buf_view("world");
    view.set_cursor(0);
    let cur = view.cursor_offset();
    view.insert(cur, "hello ");
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.slice(0, buf.len()), "hello world");
    drop(buf);
    assert_eq!(view.cursor_offset(), 6);
}

#[test]
fn insert_multibyte_advances_by_byte_length() {
    let mut view = empty_view();
    view.insert(0, "é"); // 2 bytes in UTF-8
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.len(), 2);
    assert_eq!(buf.slice(0, buf.len()), "é");
    drop(buf);
    assert_eq!(view.cursor_offset(), 2);
}

#[test]
fn delete_removes_range_and_adjusts_cursor() {
    let mut view = buf_view("hello world");
    view.set_cursor(5);
    view.delete(5, 6); // remove space
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.slice(0, buf.len()), "helloworld");
    drop(buf);
    assert_eq!(view.cursor_offset(), 5);
}

#[test]
fn delete_before_cursor_adjusts_cursor_back() {
    let mut view = buf_view("hello");
    view.set_cursor(5);
    view.delete(0, 2); // remove "he"
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.slice(0, buf.len()), "llo");
    drop(buf);
    assert_eq!(view.cursor_offset(), 3);
}

#[test]
fn delete_multibyte_char() {
    let mut view = buf_view("héllo"); // 'é' at bytes 1-2
    view.set_cursor(0);
    view.delete(1, 3); // delete 'é'
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.slice(0, buf.len()), "hllo");
}

// ── Cursor ────────────────────────────────────────────────────────────

#[test]
fn set_cursor_clamps_to_len() {
    let mut view = buf_view("hi");
    view.set_cursor(100);
    assert_eq!(view.cursor_offset(), 2); // len = 2
}

#[test]
fn set_cursor_on_empty_buffer_stays_zero() {
    let mut view = empty_view();
    view.set_cursor(5);
    assert_eq!(view.cursor_offset(), 0);
}

#[test]
fn cursor_does_not_go_past_end() {
    let mut view = buf_view("abc");
    view.set_cursor(3);
    assert_eq!(view.cursor_offset(), 3); // allowed to sit at len
    view.set_cursor(4);
    assert_eq!(view.cursor_offset(), 3); // clamped
}

// ── Undo / redo ───────────────────────────────────────────────────────

#[test]
fn undo_reverts_insert() {
    let mut view = empty_view();
    view.insert(0, "hello");
    assert!(view.undo());
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.slice(0, buf.len()), "");
}

#[test]
fn undo_then_redo_restores() {
    let mut view = empty_view();
    view.insert(0, "hello");
    view.undo();
    assert!(view.redo());
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.slice(0, buf.len()), "hello");
}

#[test]
fn undo_on_empty_history_returns_false() {
    let mut view = empty_view();
    assert!(!view.undo());
}

#[test]
fn redo_on_no_future_returns_false() {
    let mut view = empty_view();
    view.insert(0, "x");
    assert!(!view.redo()); // nothing to redo yet
}

#[test]
fn undo_multiple_steps() {
    let mut view = empty_view();
    view.insert(0, "a");
    view.insert(1, "b");
    let before = { let b = view.buffer.lock().unwrap(); b.slice(0, b.len()) };
    view.undo();
    let after = { let b = view.buffer.lock().unwrap(); b.slice(0, b.len()) };
    assert_ne!(before, after);
}

// ── char_at ───────────────────────────────────────────────────────────

#[test]
fn char_at_ascii() {
    let view = buf_view("hello");
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.char_at(0), Some('h'));
    assert_eq!(buf.char_at(4), Some('o'));
    assert_eq!(buf.char_at(5), None);
}

#[test]
fn char_at_multibyte() {
    let view = buf_view("héllo");
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.char_at(0), Some('h'));
    assert_eq!(buf.char_at(1), Some('é')); // 'é' starts at byte 1
    assert_eq!(buf.char_at(3), Some('l')); // after 'é' (2 bytes)
}

// ── Line ops ──────────────────────────────────────────────────────────

#[test]
fn line_count_multiline() {
    let view = buf_view("a\nb\nc");
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.line_count(), 3);
}

#[test]
fn line_retrieval() {
    let view = buf_view("first\nsecond\nthird");
    let buf = view.buffer.lock().unwrap();
    assert_eq!(buf.line(0), Some("first".to_string()));
    assert_eq!(buf.line(1), Some("second".to_string()));
    assert_eq!(buf.line(2), Some("third".to_string()));
    assert_eq!(buf.line(3), None);
}
