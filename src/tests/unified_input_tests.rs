//! Tests for the unified input pipeline (Sprint 11d).
//! Pure-Rust tests: key normalisation, shift mapping, mouse dispatch helpers.

use crate::kernel::input::keys::{shift_char, apply_modifiers};
use crate::kernel::input::event::{InputEvent, MouseEvent, MouseEventKind, MouseButton};
use crate::kernel::state::{Editor, LayoutSnapshot};
use crate::tests::helpers;

// ── shift_char ────────────────────────────────────────────────────────────

#[test]
fn shift_char_digit_row() {
    assert_eq!(shift_char('1'), '!');
    assert_eq!(shift_char('2'), '@');
    assert_eq!(shift_char('3'), '#');
    assert_eq!(shift_char('4'), '$');
    assert_eq!(shift_char('5'), '%');
    assert_eq!(shift_char('6'), '^');
    assert_eq!(shift_char('7'), '&');
    assert_eq!(shift_char('8'), '*');
    assert_eq!(shift_char('9'), '(');
    assert_eq!(shift_char('0'), ')');
}

#[test]
fn shift_char_punctuation() {
    assert_eq!(shift_char('`'), '~');
    assert_eq!(shift_char('-'), '_');
    assert_eq!(shift_char('='), '+');
    assert_eq!(shift_char('['), '{');
    assert_eq!(shift_char(']'), '}');
    assert_eq!(shift_char('\\'), '|');
    assert_eq!(shift_char(';'), ':');
    assert_eq!(shift_char('\''), '"');
    assert_eq!(shift_char(','), '<');
    assert_eq!(shift_char('.'), '>');
    assert_eq!(shift_char('/'), '?');
}

#[test]
fn shift_char_letters() {
    assert_eq!(shift_char('a'), 'A');
    assert_eq!(shift_char('z'), 'Z');
    // Already-shifted chars are no-ops
    assert_eq!(shift_char('A'), 'A');
    assert_eq!(shift_char('Z'), 'Z');
}

// ── apply_modifiers ───────────────────────────────────────────────────────

#[test]
fn apply_modifiers_ctrl_lowercases() {
    assert_eq!(apply_modifiers('A', true, false, false), "ctrl-a");
    assert_eq!(apply_modifiers('s', true, false, false), "ctrl-s");
    assert_eq!(apply_modifiers('R', true, false, false), "ctrl-r");
}

#[test]
fn apply_modifiers_alt() {
    assert_eq!(apply_modifiers('x', false, true, false), "meta-x");
}

#[test]
fn apply_modifiers_shift_digit() {
    assert_eq!(apply_modifiers('9', false, false, true), "(");
    assert_eq!(apply_modifiers('1', false, false, true), "!");
}

#[test]
fn apply_modifiers_shift_letter() {
    assert_eq!(apply_modifiers('a', false, false, true), "A");
}

#[test]
fn apply_modifiers_no_modifier() {
    assert_eq!(apply_modifiers('a', false, false, false), "a");
    assert_eq!(apply_modifiers('Z', false, false, false), "Z");
    assert_eq!(apply_modifiers(' ', false, false, false), " ");
}

// ── InputEvent ────────────────────────────────────────────────────────────

#[test]
fn input_event_key_debug() {
    let ev = InputEvent::Key("ctrl-a".to_string());
    let s = format!("{:?}", ev);
    assert!(s.contains("ctrl-a"));
}

#[test]
fn input_event_mouse_debug() {
    let ev = InputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Click,
        x: 5,
        y: 10,
        button: MouseButton::Left,
        modifiers: String::new(),
    });
    let s = format!("{:?}", ev);
    assert!(s.contains("Mouse"));
}

#[test]
fn input_event_resize() {
    let ev = InputEvent::Resize(120, 40);
    if let InputEvent::Resize(w, h) = ev {
        assert_eq!(w, 120);
        assert_eq!(h, 40);
    } else {
        panic!("Expected Resize");
    }
}

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
