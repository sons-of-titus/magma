use std::collections::HashMap;
use crate::state::Editor;
use crate::state::id::BufferId;
use crate::state::mode::Minibuffer;
use crate::buffer::Buffer;
use crate::command::{self, builtin};
use crate::fs::disk::DiskFileSystem;
use crate::input::handle_text_input;

fn make_editor(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", content);
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() { win.buffer_id = Some(key); }
    ed
}

fn run(ed: &mut Editor, cmd: &str) {
    command::execute_command(ed, cmd, &HashMap::new()).unwrap();
}

fn buf_text(ed: &Editor) -> String {
    let slab = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid)).unwrap();
    ed.buffers.get(slab).unwrap().slice(0, ed.buffers.get(slab).unwrap().len())
}

fn cursor(ed: &Editor) -> usize {
    let slab = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid)).unwrap();
    ed.buffers.get(slab).unwrap().cursor()
}

fn command_input(ed: &Editor) -> &str {
    ed.editor_mode.minibuffer.as_ref().map_or("", |mb| &mb.input)
}

fn visual_anchor(ed: &Editor) -> Option<usize> {
    ed.selection.as_ref().map(|s| s.anchor)
}

// ── vim_mode_name ─────────────────────────────────────────────────────

#[test]
fn starts_in_normal_mode() {
    let ed = make_editor("");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
}

#[test]
fn mode_name_insert() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-insert-mode");
    assert_eq!(ed.vim_mode_name(), "INSERT");
}

#[test]
fn mode_name_visual() {
    let mut ed = make_editor("abc");
    run(&mut ed, "enter-visual-mode");
    assert_eq!(ed.vim_mode_name(), "VISUAL");
}

#[test]
fn mode_name_replace() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-replace-mode");
    assert_eq!(ed.vim_mode_name(), "REPLACE");
}

#[test]
fn mode_name_command() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-command-mode");
    assert_eq!(ed.vim_mode_name(), "COMMAND");
}

// ── Visual mode ───────────────────────────────────────────────────────

#[test]
fn visual_anchor_set_at_cursor() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(2);
    run(&mut ed, "enter-visual-mode");
    assert_eq!(visual_anchor(&ed), Some(2));
}

#[test]
fn exit_visual_clears_anchor() {
    let mut ed = make_editor("hello");
    run(&mut ed, "enter-visual-mode");
    run(&mut ed, "exit-visual-mode");
    assert_eq!(visual_anchor(&ed), None);
    assert!(!ed.keymaps.is_layer_active("visual"));
}

#[test]
fn delete_selection_removes_chars() {
    let mut ed = make_editor("hello world");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "enter-visual-mode");
    // select "hello" (5 chars)
    for _ in 0..4 { run(&mut ed, "cursor-right"); }
    run(&mut ed, "delete-selection");
    // "hello" should be gone; leading space + "world" remain
    assert!(buf_text(&ed).starts_with(" world") || buf_text(&ed) == " world");
    assert!(!ed.keymaps.is_layer_active("visual"));
    assert_eq!(visual_anchor(&ed), None);
}

#[test]
fn change_selection_deletes_and_enters_insert() {
    let mut ed = make_editor("hi there");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "enter-visual-mode");
    run(&mut ed, "cursor-right"); // select "hi" (2 chars)
    run(&mut ed, "change-selection");
    assert!(ed.keymaps.is_layer_active("insert"));
    assert!(!ed.keymaps.is_layer_active("visual"));
}

// ── Replace mode ──────────────────────────────────────────────────────

#[test]
fn replace_mode_inserts_char() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "enter-replace-mode");
    crate::input::dispatch_key(&mut ed, "H");
    assert!(buf_text(&ed).starts_with('H'));
}

#[test]
fn replace_mode_advances_cursor() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "enter-replace-mode");
    handle_text_input(&mut ed, "X");
    assert_eq!(cursor(&ed), 1); // advanced past replaced char
}

#[test]
fn exit_replace_mode() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-replace-mode");
    assert_eq!(ed.vim_mode_name(), "REPLACE");
    run(&mut ed, "exit-replace-mode");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
}

// ── Command mode ──────────────────────────────────────────────────────

#[test]
fn command_mode_captures_input() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-command-mode");
    handle_text_input(&mut ed, "w");
    handle_text_input(&mut ed, "q");
    assert_eq!(command_input(&ed), "wq");
}

#[test]
fn command_mode_backspace_removes_char() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-command-mode");
    handle_text_input(&mut ed, "w");
    handle_text_input(&mut ed, "q");
    run(&mut ed, "command-backspace");
    assert_eq!(command_input(&ed), "w");
}

#[test]
fn command_mode_backspace_on_empty_exits() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-command-mode");
    run(&mut ed, "command-backspace");
    assert!(!ed.keymaps.is_layer_active("command"));
}

#[test]
fn command_quit_exits_editor() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-command-mode");
    handle_text_input(&mut ed, "q");
    run(&mut ed, "command-execute");
    assert!(!ed.running);
}

#[test]
fn exit_command_mode_returns_to_normal() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-command-mode");
    handle_text_input(&mut ed, "w");
    run(&mut ed, "exit-command-mode");
    assert!(ed.editor_mode.is_named("normal"));
    assert!(!ed.keymaps.is_layer_active("command"));
}

// ── Normal mode drops unbound keys (regression) ───────────────────────

#[test]
fn normal_mode_does_not_insert_unbound_key() {
    let mut ed = make_editor("");
    // In normal mode, typing 'z' (unbound) should NOT insert it.
    // Use dispatch_key so the mode check happens.
    crate::input::dispatch_key(&mut ed, "z");
    assert_eq!(buf_text(&ed), "");
}

#[test]
fn normal_mode_does_not_insert_arrow_key_name() {
    let mut ed = make_editor("");
    handle_text_input(&mut ed, "left");
    handle_text_input(&mut ed, "right");
    assert_eq!(buf_text(&ed), "");
}

#[test]
fn insert_mode_does_insert_text() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-insert-mode");
    handle_text_input(&mut ed, "h");
    handle_text_input(&mut ed, "i");
    assert_eq!(buf_text(&ed), "hi");
}

#[test]
fn insert_mode_does_not_insert_arrow_key_name() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-insert-mode");
    handle_text_input(&mut ed, "left");
    assert_eq!(buf_text(&ed), "");
}
