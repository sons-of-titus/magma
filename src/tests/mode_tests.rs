//! The non-modal editing model: text is always accepted, selections come
//! from shift+movement or the mouse, and escape only clears selection.

use crate::kernel::input::handle_text_input;
use crate::tests::helpers;

fn visual_anchor(ed: &crate::kernel::state::Editor) -> Option<usize> {
    ed.selection.as_ref().map(|s| s.anchor)
}

// ── The one shipped mode ──────────────────────────────────────────────────────

#[test]
fn starts_in_editor_mode_with_text_accepted() {
    let ed = helpers::make_editor_with_buffer("");
    assert_eq!(ed.mode_name(), "EDITOR");
    assert!(ed.editor_mode.accepts_text, "buffers always accept text");
}

#[test]
fn unbound_printable_keys_are_inserted() {
    let mut ed = helpers::make_editor_with_buffer("");
    crate::kernel::input::dispatch_key(&mut ed, "z");
    assert_eq!(helpers::buf_text(&ed), "z");
}

#[test]
fn named_arrow_keys_are_never_inserted() {
    let mut ed = helpers::make_editor_with_buffer("");
    handle_text_input(&mut ed, "left");
    handle_text_input(&mut ed, "right");
    assert_eq!(helpers::buf_text(&ed), "");
}

// ── Selections (mouse / shift+movement — no mode entry) ───────────────────────

#[test]
fn delete_selection_removes_chars() {
    let mut ed = helpers::make_editor_with_buffer("hello world");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    ed.selection = Some(crate::kernel::state::mode::Selection::char(0));
    // select "hello" by walking the cursor right
    for _ in 0..4 { helpers::run(&mut ed, "cursor-right"); }
    helpers::run(&mut ed, "delete-selection");
    assert!(helpers::buf_text(&ed).starts_with(" world"),
        "got: {}", helpers::buf_text(&ed));
    assert_eq!(visual_anchor(&ed), None, "selection is consumed");
}

#[test]
fn yank_selection_copies_and_clears() {
    let mut ed = helpers::make_editor_with_buffer("hello world");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    ed.selection = Some(crate::kernel::state::mode::Selection::char(0));
    helpers::run(&mut ed, "cursor-right");
    helpers::run(&mut ed, "yank-selection");
    assert_eq!(ed.yanked_text.as_deref(), Some("he"));
    assert_eq!(visual_anchor(&ed), None);
    assert_eq!(helpers::buf_text(&ed), "hello world", "yank does not delete");
}

#[test]
fn cut_selection_copies_then_deletes() {
    let mut ed = helpers::make_editor_with_buffer("hello world");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    ed.selection = Some(crate::kernel::state::mode::Selection::char(0));
    helpers::run(&mut ed, "cursor-right");
    helpers::run(&mut ed, "cut-selection");
    assert_eq!(ed.yanked_text.as_deref(), Some("he"));
    assert_eq!(helpers::buf_text(&ed), "llo world");
}

#[test]
fn escape_clears_selection_without_entering_anything() {
    let mut ed = helpers::make_editor_with_buffer("text");
    ed.keymaps.push_layer("editor");
    ed.keymaps.set_layer("editor", "esc", "editor-escape");
    ed.selection = Some(crate::kernel::state::mode::Selection::char(0));
    crate::kernel::input::dispatch_key(&mut ed, "esc");
    assert_eq!(visual_anchor(&ed), None);
    assert!(ed.editor_mode.is_named("editor"), "still the same mode");
}
