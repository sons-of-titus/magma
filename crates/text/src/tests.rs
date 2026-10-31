use super::*;
use id::BufferId;
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
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
    drop(buf);
    assert_eq!(view.cursor_offset(), 0);
}

#[test]
fn insert_appends_text_and_advances_cursor() {
    let mut view = empty_view();
    view.insert(0, "hello");
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
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
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.slice(0, buf.len()), "hello world");
    drop(buf);
    assert_eq!(view.cursor_offset(), 6);
}

#[test]
fn insert_multibyte_advances_by_byte_length() {
    let mut view = empty_view();
    view.insert(0, "é"); // 2 bytes in UTF-8
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
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
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.slice(0, buf.len()), "helloworld");
    drop(buf);
    assert_eq!(view.cursor_offset(), 5);
}

#[test]
fn delete_before_cursor_adjusts_cursor_back() {
    let mut view = buf_view("hello");
    view.set_cursor(5);
    view.delete(0, 2); // remove "he"
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.slice(0, buf.len()), "llo");
    drop(buf);
    assert_eq!(view.cursor_offset(), 3);
}

#[test]
fn delete_multibyte_char() {
    let mut view = buf_view("héllo"); // 'é' at bytes 1-2
    view.set_cursor(0);
    view.delete(1, 3); // delete 'é'
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
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
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.slice(0, buf.len()), "");
}

#[test]
fn undo_then_redo_restores() {
    let mut view = empty_view();
    view.insert(0, "hello");
    view.undo();
    assert!(view.redo());
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
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
    let before = {
        let b = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
        b.slice(0, b.len())
    };
    view.undo();
    let after = {
        let b = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
        b.slice(0, b.len())
    };
    assert_ne!(before, after);
}

// ── char_at ───────────────────────────────────────────────────────────

#[test]
fn char_at_ascii() {
    let view = buf_view("hello");
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.char_at(0), Some('h'));
    assert_eq!(buf.char_at(4), Some('o'));
    assert_eq!(buf.char_at(5), None);
}

#[test]
fn char_at_multibyte() {
    let view = buf_view("héllo");
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.char_at(0), Some('h'));
    assert_eq!(buf.char_at(1), Some('é')); // 'é' starts at byte 1
    assert_eq!(buf.char_at(3), Some('l')); // after 'é' (2 bytes)
}

// ── Line ops ──────────────────────────────────────────────────────────

#[test]
fn line_count_multiline() {
    let view = buf_view("a\nb\nc");
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.line_count(), 3);
}

#[test]
fn line_retrieval() {
    let view = buf_view("first\nsecond\nthird");
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(buf.line(0), Some("first".to_string()));
    assert_eq!(buf.line(1), Some("second".to_string()));
    assert_eq!(buf.line(2), Some("third".to_string()));
    assert_eq!(buf.line(3), None);
}

// ── Highlight ranges survive edits (search hits track the text) ──────

#[test]
fn insert_before_a_highlight_shifts_it() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(6, 10, "search".into())]);
    view.insert(0, "XX");
    assert_eq!(view.highlights, vec![(8, 12, "search".into())]);
}

#[test]
fn insert_at_the_highlight_start_pushes_the_range_right() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(6, 10, "search".into())]);
    view.insert(6, ">>");
    assert_eq!(view.highlights, vec![(8, 12, "search".into())]);
}

#[test]
fn deleting_a_highlighted_range_drops_it() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(6, 10, "search".into())]);
    view.delete(6, 10);
    assert!(view.highlights.is_empty());
}

#[test]
fn delete_before_a_highlight_shifts_it_left() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(6, 10, "search".into())]);
    view.delete(0, 2);
    assert_eq!(view.highlights, vec![(4, 8, "search".into())]);
}

#[test]
fn undo_transforms_highlights_back() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(6, 10, "search".into())]);
    view.insert(0, "XX");
    assert_eq!(view.highlights[0].0, 8);
    assert!(view.undo());
    assert_eq!(view.highlights, vec![(6, 10, "search".into())]);
    assert!(view.redo());
    assert_eq!(view.highlights, vec![(8, 12, "search".into())]);
}

#[test]
fn insert_inside_a_highlight_grows_it() {
    // Typing mid-word keeps the word colored through the new character
    // instead of dropping the span until the next parse lands.
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(6, 10, "syntax".into())]);
    view.insert(8, "XX");
    assert_eq!(view.highlights, vec![(6, 12, "syntax".into())]);
}

#[test]
fn deleting_a_highlight_head_clamps_to_the_surviving_tail() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(6, 10, "syntax".into())]);
    view.delete(4, 8); // " b" — half of "beta"
    assert_eq!(view.highlights, vec![(4, 6, "syntax".into())]);
}

#[test]
fn deleting_a_highlight_tail_clamps_to_the_surviving_head() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(6, 10, "syntax".into())]);
    view.delete(8, 12); // "ta g" — the tail of "beta"
    assert_eq!(view.highlights, vec![(6, 8, "syntax".into())]);
}

#[test]
fn a_span_bridging_an_insert_stretches_with_it() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights(vec![(4, 12, "syntax".into())]);
    view.insert(6, ">>");
    assert_eq!(view.highlights, vec![(4, 14, "syntax".into())]);
}

#[test]
fn highlight_layers_transform_with_the_flat_list() {
    let mut view = buf_view("alpha beta gamma");
    view.set_highlights_layer("semantic", vec![(6, 10, "sel".into())]);
    view.delete(0, 2);
    assert_eq!(
        view.highlight_layers.get("semantic").map(|v| v.as_slice()),
        Some(&[(4, 8, "sel".to_string())][..])
    );
}

// ── Fold ranges survive edits (folds track the text, not stale bytes) ──

#[test]
fn insert_before_a_fold_shifts_it() {
    let mut view = buf_view("alpha beta gamma");
    view.add_fold(6, 10);
    view.insert(0, "XX");
    assert_eq!(view.folds, vec![(8, 12)]);
}

#[test]
fn deleting_through_a_fold_opens_it() {
    let mut view = buf_view("alpha beta gamma");
    view.add_fold(6, 10);
    view.delete(4, 12);
    assert!(view.folds.is_empty());
}

#[test]
fn fold_candidates_transform_like_folds() {
    let mut view = buf_view("alpha beta gamma");
    view.set_fold_ranges(vec![(6, 10)]);
    view.insert(0, "XX");
    assert_eq!(view.fold_ranges, vec![(8, 12)]);
    assert!(view.undo());
    assert_eq!(view.fold_ranges, vec![(6, 10)]);
}

#[test]
fn a_fold_bridging_an_insert_stretches_with_it() {
    let mut view = buf_view("alpha beta gamma");
    view.add_fold(4, 12);
    view.insert(6, ">>");
    assert_eq!(view.folds, vec![(4, 14)]);
}
