//! Tests for the unified input pipeline.
//! Pure-Rust tests: layout snapshot, on-input hook.

use crate::kernel::state::LayoutSnapshot;
use crate::tests::helpers;

// ── LayoutSnapshot ────────────────────────────────────────────────────────

#[test]
fn layout_snapshot_default() {
    let snap = LayoutSnapshot::default();
    assert_eq!(snap.gutter_width, 0);
    assert_eq!(snap.content_x, 0);
    assert_eq!(snap.row_offset, 0);
    assert_eq!(snap.scroll_top, 0);
    assert_eq!(snap.visible_lines, 0);
}

#[test]
fn editor_has_last_layout_field() {
    let ed = helpers::make_editor_with_buffer("hello");
    let _ = &ed.last_layout;
}

// ── on_input_fn ───────────────────────────────────────────────────────────

#[test]
fn editor_on_input_fn_starts_none() {
    let ed = helpers::make_editor();
    assert!(ed.on_input_fn.is_none());
}

#[test]
fn editor_on_input_fn_can_be_set() {
    let mut ed = helpers::make_editor();
    ed.on_input_fn = Some("my-interceptor".to_string());
    assert_eq!(ed.on_input_fn.as_deref(), Some("my-interceptor"));
}
