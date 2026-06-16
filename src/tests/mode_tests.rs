use crate::kernel::input::handle_text_input;
use crate::tests::helpers;

fn visual_anchor(ed: &crate::kernel::state::Editor) -> Option<usize> {
    ed.selection.as_ref().map(|s| s.anchor)
}

fn command_input(ed: &crate::kernel::state::Editor) -> &str {
    ed.editor_mode.minibuffer.as_ref().map_or("", |mb| &mb.input)
}

// ── vim_mode_name ─────────────────────────────────────────────────────

#[test]
fn starts_in_normal_mode() {
    let ed = helpers::make_editor_with_buffer("");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
}

#[test]
fn mode_name_insert() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-insert-mode");
    assert_eq!(ed.vim_mode_name(), "INSERT");
}

#[test]
fn mode_name_visual() {
    let mut ed = helpers::make_editor_with_buffer("abc");
    helpers::run(&mut ed, "enter-visual-mode");
    assert_eq!(ed.vim_mode_name(), "VISUAL");
}

#[test]
fn mode_name_replace() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-replace-mode");
    assert_eq!(ed.vim_mode_name(), "REPLACE");
}

#[test]
fn mode_name_command() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-command-mode");
    assert_eq!(ed.vim_mode_name(), "COMMAND");
}

// ── Visual mode ───────────────────────────────────────────────────────

#[test]
fn visual_anchor_set_at_cursor() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(2);
    helpers::run(&mut ed, "enter-visual-mode");
    assert_eq!(visual_anchor(&ed), Some(2));
}

#[test]
fn exit_visual_clears_anchor() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    helpers::run(&mut ed, "enter-visual-mode");
    helpers::run(&mut ed, "exit-visual-mode");
    assert_eq!(visual_anchor(&ed), None);
    assert!(!ed.keymaps.is_layer_active("visual"));
}

#[test]
fn delete_selection_removes_chars() {
    let mut ed = helpers::make_editor_with_buffer("hello world");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    helpers::run(&mut ed, "enter-visual-mode");
    // select "hello" (5 chars)
    for _ in 0..4 { helpers::run(&mut ed, "cursor-right"); }
    helpers::run(&mut ed, "delete-selection");
    // "hello" should be gone; leading space + "world" remain
    assert!(helpers::buf_text(&ed).starts_with(" world") || helpers::buf_text(&ed) == " world");
    assert!(!ed.keymaps.is_layer_active("visual"));
    assert_eq!(visual_anchor(&ed), None);
}

#[test]
fn change_selection_deletes_and_enters_insert() {
    let mut ed = helpers::make_editor_with_buffer("hi there");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    helpers::run(&mut ed, "enter-visual-mode");
    helpers::run(&mut ed, "cursor-right"); // select "hi" (2 chars)
    helpers::run(&mut ed, "change-selection");
    assert!(ed.keymaps.is_layer_active("insert"));
    assert!(!ed.keymaps.is_layer_active("visual"));
}

// ── Replace mode ──────────────────────────────────────────────────────

#[test]
fn replace_mode_inserts_char() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    helpers::run(&mut ed, "enter-replace-mode");
    crate::kernel::input::dispatch_key(&mut ed, "H");
    assert!(helpers::buf_text(&ed).starts_with('H'));
}

#[test]
fn replace_mode_advances_cursor() {
    let mut ed = helpers::make_editor_with_buffer("abc");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    helpers::run(&mut ed, "enter-replace-mode");
    handle_text_input(&mut ed, "X");
    assert_eq!(helpers::cursor(&ed), 1); // advanced past replaced char
}

#[test]
fn exit_replace_mode() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-replace-mode");
    assert_eq!(ed.vim_mode_name(), "REPLACE");
    helpers::run(&mut ed, "exit-replace-mode");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
}

// ── Command mode ──────────────────────────────────────────────────────

#[test]
fn command_mode_captures_input() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-command-mode");
    handle_text_input(&mut ed, "w");
    handle_text_input(&mut ed, "q");
    assert_eq!(command_input(&ed), "wq");
}

#[test]
fn command_mode_backspace_removes_char() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-command-mode");
    handle_text_input(&mut ed, "w");
    handle_text_input(&mut ed, "q");
    helpers::run(&mut ed, "command-backspace");
    assert_eq!(command_input(&ed), "w");
}

#[test]
fn command_mode_backspace_on_empty_exits() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-command-mode");
    helpers::run(&mut ed, "command-backspace");
    assert!(!ed.keymaps.is_layer_active("command"));
}

#[test]
fn command_quit_exits_editor() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-command-mode");
    handle_text_input(&mut ed, "q");
    helpers::run(&mut ed, "command-execute");
    assert!(!ed.running);
}

#[test]
fn exit_command_mode_returns_to_normal() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-command-mode");
    handle_text_input(&mut ed, "w");
    helpers::run(&mut ed, "exit-command-mode");
    assert!(ed.editor_mode.is_named("normal"));
    assert!(!ed.keymaps.is_layer_active("command"));
}

// ── Normal mode drops unbound keys (regression) ───────────────────────

#[test]
fn normal_mode_does_not_insert_unbound_key() {
    let mut ed = helpers::make_editor_with_buffer("");
    // In normal mode, typing 'z' (unbound) should NOT insert it.
    // Use dispatch_key so the mode check happens.
    crate::kernel::input::dispatch_key(&mut ed, "z");
    assert_eq!(helpers::buf_text(&ed), "");
}

#[test]
fn normal_mode_does_not_insert_arrow_key_name() {
    let mut ed = helpers::make_editor_with_buffer("");
    handle_text_input(&mut ed, "left");
    handle_text_input(&mut ed, "right");
    assert_eq!(helpers::buf_text(&ed), "");
}

#[test]
fn insert_mode_does_insert_text() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-insert-mode");
    handle_text_input(&mut ed, "h");
    handle_text_input(&mut ed, "i");
    assert_eq!(helpers::buf_text(&ed), "hi");
}

#[test]
fn insert_mode_does_not_insert_arrow_key_name() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-insert-mode");
    handle_text_input(&mut ed, "left");
    assert_eq!(helpers::buf_text(&ed), "");
}
