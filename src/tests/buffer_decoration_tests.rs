//! Pure Rust tests for Sprint 11b buffer decoration data model.

use crate::kernel::text_engine::{Buffer, BufferView, Decoration};
use crate::kernel::state::id::BufferId;
use std::sync::{Arc, Mutex};

fn view() -> BufferView {
    let buf = Buffer::new(BufferId(1), "test");
    BufferView::new(Arc::new(Mutex::new(buf)))
}

// ── InlineText ────────────────────────────────────────────────────────────────

#[test]
fn decor_set_inline_stores_decoration() {
    let mut b = view();
    b.decor_set_inline("layer1", 3, 5, "hint".to_string(), "type-face".to_string());
    let count = b.decor_count_layer("layer1");
    assert_eq!(count, 1);
}

#[test]
fn decor_set_inline_replaces_at_same_position() {
    let mut b = view();
    b.decor_set_inline("layer1", 3, 5, "first".to_string(), "type-face".to_string());
    b.decor_set_inline("layer1", 3, 5, "second".to_string(), "type-face".to_string());
    assert_eq!(b.decor_count_layer("layer1"), 1);
    if let Some(layer) = b.decoration_layers.get("layer1") {
        if let Some(Decoration::InlineText { text, .. }) = layer.first() {
            assert_eq!(text, "second");
        } else {
            panic!("wrong decoration type");
        }
    }
}

#[test]
fn decor_set_inline_different_positions_accumulate() {
    let mut b = view();
    b.decor_set_inline("layer1", 0, 0, "a".to_string(), "f".to_string());
    b.decor_set_inline("layer1", 1, 0, "b".to_string(), "f".to_string());
    assert_eq!(b.decor_count_layer("layer1"), 2);
}

// ── EndOfLine ─────────────────────────────────────────────────────────────────

#[test]
fn decor_set_eol_stores_decoration() {
    let mut b = view();
    b.decor_set_eol("blame", 0, "alice 2d ago".to_string(), "comment-face".to_string());
    assert_eq!(b.decor_count_layer("blame"), 1);
}

#[test]
fn decor_set_eol_replaces_on_same_line() {
    let mut b = view();
    b.decor_set_eol("blame", 5, "old".to_string(), "f".to_string());
    b.decor_set_eol("blame", 5, "new".to_string(), "f".to_string());
    assert_eq!(b.decor_count_layer("blame"), 1);
    if let Some(layer) = b.decoration_layers.get("blame") {
        if let Some(Decoration::EndOfLine { text, .. }) = layer.first() {
            assert_eq!(text, "new");
        } else {
            panic!("wrong decoration type");
        }
    }
}

// ── LinePrefix ────────────────────────────────────────────────────────────────

#[test]
fn decor_set_prefix_stores_decoration() {
    let mut b = view();
    b.decor_set_prefix("cov", 2, "42".to_string(), "keyword-face".to_string());
    assert_eq!(b.decor_count_layer("cov"), 1);
}

#[test]
fn decor_set_prefix_replaces_on_same_line() {
    let mut b = view();
    b.decor_set_prefix("cov", 2, "old".to_string(), "f".to_string());
    b.decor_set_prefix("cov", 2, "new".to_string(), "f".to_string());
    assert_eq!(b.decor_count_layer("cov"), 1);
}

// ── Clear ─────────────────────────────────────────────────────────────────────

#[test]
fn decor_clear_layer_removes_layer() {
    let mut b = view();
    b.decor_set_eol("x", 0, "a".to_string(), "f".to_string());
    b.decor_clear_layer("x");
    assert_eq!(b.decor_count_layer("x"), 0);
    assert!(!b.decoration_layers.contains_key("x"));
}

#[test]
fn decor_clear_removes_all_layers() {
    let mut b = view();
    b.decor_set_eol("x", 0, "a".to_string(), "f".to_string());
    b.decor_set_prefix("y", 1, "b".to_string(), "f".to_string());
    b.decor_clear();
    assert!(b.decoration_layers.is_empty());
}

// ── Count ─────────────────────────────────────────────────────────────────────

#[test]
fn decor_count_returns_zero_for_missing_layer() {
    let b = view();
    assert_eq!(b.decor_count_layer("nonexistent"), 0);
}

#[test]
fn decor_count_returns_correct_count() {
    let mut b = view();
    b.decor_set_eol("l", 0, "a".to_string(), "f".to_string());
    b.decor_set_eol("l", 1, "b".to_string(), "f".to_string());
    b.decor_set_eol("l", 2, "c".to_string(), "f".to_string());
    assert_eq!(b.decor_count_layer("l"), 3);
}

// ── Multiple layers ───────────────────────────────────────────────────────────

#[test]
fn multiple_layers_are_independent() {
    let mut b = view();
    b.decor_set_eol("blame", 0, "alice".to_string(), "f".to_string());
    b.decor_set_inline("hints", 0, 5, "i32".to_string(), "t".to_string());
    assert_eq!(b.decor_count_layer("blame"), 1);
    assert_eq!(b.decor_count_layer("hints"), 1);
    b.decor_clear_layer("blame");
    assert_eq!(b.decor_count_layer("blame"), 0);
    assert_eq!(b.decor_count_layer("hints"), 1);
}
