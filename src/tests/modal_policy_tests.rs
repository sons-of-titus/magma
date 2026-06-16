//! Pure-Rust tests for the modal input policy registries (Sprint 11e).
//! Tests that vim_operators / vim_prefixes / vim_motions / vim_char_captures
//! are consulted by dispatch_key instead of hardcoded constants.

use crate::kernel::state::Editor;
use crate::kernel::input::dispatch_key;
use crate::tests::helpers;

fn make_editor(content: &str) -> Editor {
    let mut ed = helpers::make_editor_with_buffer(content);
    ed.keymaps.push_layer("vim");
    ed.keymaps.set_layer("vim", "w",  "move-word-forward");
    ed.keymaps.set_layer("vim", "l",  "cursor-right");
    ed.keymaps.set_layer("vim", "h",  "cursor-left");
    ed.keymaps.set_layer("insert", "esc", "exit-insert-mode");
    ed
}

fn text(ed: &Editor) -> String {
    let slab = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid)).unwrap();
    ed.buffers.get(slab).unwrap().slice(0, ed.buffers.get(slab).unwrap().len())
}

fn cursor(ed: &Editor) -> usize {
    let slab = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid)).unwrap();
    ed.buffers.get(slab).unwrap().cursor()
}

// ── Registry fields exist ─────────────────────────────────────────────────

#[test]
fn vim_operators_starts_empty() {
    let ed = make_editor("");
    assert!(ed.vim_operators.is_empty());
}

#[test]
fn vim_prefixes_starts_empty() {
    let ed = make_editor("");
    assert!(ed.vim_prefixes.is_empty());
}

#[test]
fn vim_motions_starts_empty() {
    let ed = make_editor("");
    assert!(ed.vim_motions.is_empty());
}

#[test]
fn vim_char_captures_starts_empty() {
    let ed = make_editor("");
    assert!(ed.vim_char_captures.is_empty());
}

// ── Bare dispatcher with no registries ───────────────────────────────────

#[test]
fn d_key_without_operator_registration_inserts_nothing() {
    // With no operators registered, 'd' should not enter operator-pending.
    let mut ed = make_editor("hello");
    dispatch_key(&mut ed, "d");
    // Not entering operator mode → key falls through to text input (not normal mode insert)
    assert_eq!(text(&ed), "hello");  // no deletion
}

// ── Operator registration ─────────────────────────────────────────────────

#[test]
fn registered_operator_dd_deletes_line() {
    let mut ed = make_editor("hello\nworld");
    ed.vim_operators.insert("d".to_string(), "delete-line".to_string());
    dispatch_key(&mut ed, "d");
    dispatch_key(&mut ed, "d");
    assert_eq!(text(&ed), "world");
}

#[test]
fn registered_operator_dw_deletes_word() {
    let mut ed = make_editor("hello world");
    ed.vim_operators.insert("d".to_string(), "delete-line".to_string());
    ed.vim_motions.insert("w".to_string(), "move-word-forward".to_string());
    ed.buffers.iter_mut().next().unwrap().1.set_cursor(0);
    dispatch_key(&mut ed, "d");
    dispatch_key(&mut ed, "w");
    // dw removes "hello "
    assert!(text(&ed).starts_with("world"));
}

#[test]
fn operator_without_motion_cancels_on_unknown_key() {
    let mut ed = make_editor("hello");
    ed.vim_operators.insert("d".to_string(), "delete-line".to_string());
    dispatch_key(&mut ed, "d");  // enter operator-pending
    dispatch_key(&mut ed, "q");  // unknown key — cancels
    assert_eq!(text(&ed), "hello");  // nothing deleted
}

// ── Prefix registration ───────────────────────────────────────────────────

#[test]
fn registered_prefix_gg_runs_goto_start() {
    let mut ed = make_editor("hello\nworld");
    ed.vim_prefixes.insert("g".to_string());
    ed.keymaps.set_layer("vim", "gg", "goto-buffer-start");
    // Move to end first
    let slab = ed.view_tree.focused_window().and_then(|wid| ed.view_tree.buffer(wid)).unwrap();
    ed.buffers.get_mut(slab).unwrap().set_cursor(6);
    dispatch_key(&mut ed, "g");
    dispatch_key(&mut ed, "g");
    assert_eq!(cursor(&ed), 0);
}

#[test]
fn unregistered_key_is_not_treated_as_prefix() {
    let mut ed = make_editor("hello");
    // 'g' NOT registered as prefix
    ed.keymaps.set_layer("vim", "gg", "goto-buffer-start");
    dispatch_key(&mut ed, "g");  // no prefix-waiting state entered
    dispatch_key(&mut ed, "g");  // second 'g' runs independently
    // Neither 'g' runs goto-buffer-start since the prefix wasn't registered
    // Both 'g' keys fall through to text input (ignored in normal mode since not insertable)
    assert_eq!(text(&ed), "hello");
}

// ── Char-capture registration ─────────────────────────────────────────────

#[test]
fn char_capture_r_replaces_char() {
    let mut ed = make_editor("hello");
    ed.vim_char_captures.insert("r".to_string(), "replace-char".to_string());
    let slab = ed.view_tree.focused_window().and_then(|wid| ed.view_tree.buffer(wid)).unwrap();
    ed.buffers.get_mut(slab).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "r");  // enter char-capture
    dispatch_key(&mut ed, "H");  // replace 'h' with 'H'
    assert!(text(&ed).starts_with('H'));
}

#[test]
fn char_capture_m_sets_mark() {
    let mut ed = make_editor("hello");
    ed.vim_char_captures.insert("m".to_string(), "set-mark".to_string());
    dispatch_key(&mut ed, "m");  // enter char-capture
    dispatch_key(&mut ed, "a");  // set mark 'a'
    assert!(ed.marks.contains_key(&'a'));
}

// ── Motion registry ───────────────────────────────────────────────────────

#[test]
fn dl_deletes_char_right() {
    let mut ed = make_editor("hello");
    ed.vim_operators.insert("d".to_string(), "delete-line".to_string());
    ed.vim_motions.insert("l".to_string(), "cursor-right".to_string());
    let slab = ed.view_tree.focused_window().and_then(|wid| ed.view_tree.buffer(wid)).unwrap();
    ed.buffers.get_mut(slab).unwrap().set_cursor(0);
    dispatch_key(&mut ed, "d");
    dispatch_key(&mut ed, "l");
    // dl deletes chars from cursor to one right
    assert!(!text(&ed).starts_with('h'));
}
