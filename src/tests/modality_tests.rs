//! Pure-Rust tests for the modality abstraction (Sprint 11f).
//! Tests EditorMode, Minibuffer, Selection, and plugin_state.

use crate::kernel::state::Editor;
use crate::kernel::state::mode::{EditorMode, Minibuffer, Selection};
use crate::tests::helpers;

// ── EditorMode ────────────────────────────────────────────────────────────

#[test]
fn editor_mode_starts_normal() {
    let ed = helpers::make_editor();
    assert_eq!(ed.editor_mode.name, "normal");
    assert!(!ed.editor_mode.accepts_text);
    assert!(ed.editor_mode.minibuffer.is_none());
}

#[test]
fn editor_mode_set() {
    let mut ed = helpers::make_editor();
    ed.editor_mode = EditorMode::new("insert", true);
    assert_eq!(ed.editor_mode.name, "insert");
    assert!(ed.editor_mode.accepts_text);
}

#[test]
fn editor_mode_is_named() {
    let mut ed = helpers::make_editor();
    ed.editor_mode.name = "visual".to_string();
    assert!(ed.editor_mode.is_named("visual"));
    assert!(!ed.editor_mode.is_named("insert"));
}

// ── Minibuffer ────────────────────────────────────────────────────────────

#[test]
fn minibuffer_open_and_close() {
    let mut ed = helpers::make_editor();
    ed.editor_mode.minibuffer = Some(Minibuffer { prompt: ":".to_string(), input: String::new() });
    assert!(ed.editor_mode.minibuffer.is_some());
    ed.editor_mode.minibuffer = None;
    assert!(ed.editor_mode.minibuffer.is_none());
}

#[test]
fn minibuffer_input_accumulates() {
    let mut ed = helpers::make_editor();
    ed.editor_mode.minibuffer = Some(Minibuffer { prompt: ":".to_string(), input: "he".to_string() });
    if let Some(ref mut mb) = ed.editor_mode.minibuffer {
        mb.input.push_str("llo");
    }
    assert_eq!(ed.editor_mode.minibuffer.as_ref().unwrap().input, "hello");
}

// ── Selection ─────────────────────────────────────────────────────────────

#[test]
fn selection_starts_none() {
    let ed = helpers::make_editor();
    assert!(ed.selection.is_none());
}

#[test]
fn selection_char() {
    let mut ed = helpers::make_editor();
    ed.selection = Some(Selection::char(10));
    let sel = ed.selection.as_ref().unwrap();
    assert_eq!(sel.anchor, 10);
    assert_eq!(sel.kind, "char");
    assert!(!sel.is_line());
}

#[test]
fn selection_line() {
    let mut ed = helpers::make_editor();
    ed.selection = Some(Selection::line(5));
    let sel = ed.selection.as_ref().unwrap();
    assert_eq!(sel.anchor, 5);
    assert!(sel.is_line());
}

#[test]
fn selection_clear() {
    let mut ed = helpers::make_editor();
    ed.selection = Some(Selection::char(3));
    ed.selection = None;
    assert!(ed.selection.is_none());
}

#[test]
fn selection_range_uses_new_model() {
    let mut ed = helpers::make_editor();
    let buf_key = ed.view_tree.focused_window().and_then(|w| ed.view_tree.buffer(w)).unwrap();
    ed.views.get_mut(&buf_key).unwrap().insert(0, "hello");
    ed.views.get_mut(&buf_key).unwrap().set_cursor(3);
    ed.selection = Some(Selection::char(1));
    let range = ed.selection_range(3);
    assert_eq!(range, Some((1, 4)));
}

// ── Plugin state ──────────────────────────────────────────────────────────

#[test]
fn plugin_state_starts_empty() {
    let ed = helpers::make_editor();
    assert!(ed.plugin_state.is_empty());
}

#[test]
fn plugin_state_set_and_get() {
    let mut ed = helpers::make_editor();
    ed.plugin_state.insert("vim.pending-operator".to_string(), "d".to_string());
    assert_eq!(ed.plugin_state.get("vim.pending-operator").map(|s| s.as_str()), Some("d"));
}

#[test]
fn plugin_state_delete() {
    let mut ed = helpers::make_editor();
    ed.plugin_state.insert("k".to_string(), "v".to_string());
    ed.plugin_state.remove("k");
    assert!(ed.plugin_state.get("k").is_none());
}

// ── vim_mode_name reflects editor_mode ────────────────────────────────────

#[test]
fn vim_mode_name_default_is_normal() {
    let ed = helpers::make_editor();
    assert_eq!(ed.vim_mode_name(), "NORMAL");
}

#[test]
fn vim_mode_name_follows_editor_mode() {
    let mut ed = helpers::make_editor();
    ed.editor_mode.name = "insert".to_string();
    assert_eq!(ed.vim_mode_name(), "INSERT");
}
