use std::collections::HashMap;
use crate::state::Editor;
use crate::state::id::BufferId;
use crate::buffer::Buffer;
use crate::command::{self, builtin};
use crate::fs::disk::DiskFileSystem;

fn make_editor(content: &str) -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);

    let id = ed.allocate_buffer_id();
    let buf = Buffer::from_string(BufferId(id), "test", content);
    let entry = ed.buffers.vacant_entry();
    let key = entry.key();
    entry.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

fn run(ed: &mut Editor, cmd: &str) {
    command::execute_command(ed, cmd, &HashMap::new()).unwrap();
}

fn text(ed: &Editor) -> String {
    let slab = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();
    ed.buffers.get(slab).unwrap().slice(0, ed.buffers.get(slab).unwrap().len())
}

fn cursor(ed: &Editor) -> usize {
    let slab = ed.windows.focused_window()
        .and_then(|wid| ed.windows.buffer(wid))
        .unwrap();
    ed.buffers.get(slab).unwrap().cursor()
}

// ── Cursor movement ───────────────────────────────────────────────────

#[test]
fn cursor_left_moves_back_one_char() {
    let mut ed = make_editor("hello");
    // from_string starts cursor at 0; move it to end first
    ed.buffers.get_mut(0).unwrap().set_cursor(5);
    assert_eq!(cursor(&ed), 5);
    run(&mut ed, "cursor-left");
    assert_eq!(cursor(&ed), 4);
}

#[test]
fn cursor_right_moves_forward_one_char() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "cursor-right");
    assert_eq!(cursor(&ed), 1);
}

#[test]
fn cursor_left_stops_at_zero() {
    let mut ed = make_editor("hi");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "cursor-left");
    assert_eq!(cursor(&ed), 0);
}

#[test]
fn cursor_right_stops_at_end() {
    let mut ed = make_editor("hi");
    // from_string starts cursor at 0; move to end then try to go further
    ed.buffers.get_mut(0).unwrap().set_cursor(2);
    assert_eq!(cursor(&ed), 2);
    run(&mut ed, "cursor-right");
    assert_eq!(cursor(&ed), 2);
}

#[test]
fn cursor_left_right_multibyte() {
    let mut ed = make_editor("héllo");
    // 'é' is 2 bytes; cursor starts at end (byte 6)
    ed.buffers.get_mut(0).unwrap().set_cursor(3); // after 'é'
    run(&mut ed, "cursor-left");
    assert_eq!(cursor(&ed), 1); // back to 'é' start
    run(&mut ed, "cursor-right");
    assert_eq!(cursor(&ed), 3); // past 'é'
}

#[test]
fn cursor_down_moves_to_next_line() {
    let mut ed = make_editor("first\nsecond");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "cursor-down");
    // should be on "second" line
    assert!(cursor(&ed) >= 6);
}

#[test]
fn cursor_up_moves_to_prev_line() {
    let mut ed = make_editor("first\nsecond");
    // start on second line
    ed.buffers.get_mut(0).unwrap().set_cursor(6);
    run(&mut ed, "cursor-up");
    assert!(cursor(&ed) < 6);
}

// ── Line-start / line-end ─────────────────────────────────────────────

#[test]
fn line_start_moves_to_bol() {
    let mut ed = make_editor("hello\nworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(8); // somewhere in "world"
    run(&mut ed, "line-start");
    assert_eq!(cursor(&ed), 6); // 'w' in "world"
}

#[test]
fn line_end_moves_to_eol() {
    let mut ed = make_editor("hello\nworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "line-end");
    assert_eq!(cursor(&ed), 5); // after 'o' in "hello"
}

// ── Delete / backspace with unicode ───────────────────────────────────

#[test]
fn delete_char_removes_ascii() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(1);
    run(&mut ed, "delete-char");
    assert_eq!(text(&ed), "hllo");
}

#[test]
fn delete_char_removes_full_multibyte() {
    let mut ed = make_editor("héllo");
    ed.buffers.get_mut(0).unwrap().set_cursor(1); // on 'é'
    run(&mut ed, "delete-char");
    assert_eq!(text(&ed), "hllo");
}

#[test]
fn backspace_removes_preceding_ascii() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(5);
    run(&mut ed, "backspace");
    assert_eq!(text(&ed), "hell");
}

#[test]
fn backspace_removes_preceding_multibyte() {
    let mut ed = make_editor("héllo");
    ed.buffers.get_mut(0).unwrap().set_cursor(3); // just after 'é'
    run(&mut ed, "backspace");
    assert_eq!(text(&ed), "hllo");
}

#[test]
fn backspace_at_start_does_nothing() {
    let mut ed = make_editor("hi");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "backspace");
    assert_eq!(text(&ed), "hi");
}

// ── Newline ───────────────────────────────────────────────────────────

#[test]
fn newline_inserts_at_cursor() {
    let mut ed = make_editor("helloworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(5);
    run(&mut ed, "newline");
    assert_eq!(text(&ed), "hello\nworld");
}

// ── Undo / redo ───────────────────────────────────────────────────────

#[test]
fn undo_command_reverts_insert() {
    let mut ed = make_editor("");
    {
        let slab = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid)).unwrap();
        ed.buffers.get_mut(slab).unwrap().insert(0, "hello");
    }
    run(&mut ed, "undo");
    assert_eq!(text(&ed), "");
}

// ── Modal editing (enter/exit insert mode) ────────────────────────────

#[test]
fn enter_insert_mode_activates_insert_layer() {
    let mut ed = make_editor("");
    assert!(!ed.keymaps.is_layer_active("insert"));
    run(&mut ed, "enter-insert-mode");
    assert!(ed.keymaps.is_layer_active("insert"));
}

#[test]
fn exit_insert_mode_deactivates_insert_layer() {
    let mut ed = make_editor("");
    run(&mut ed, "enter-insert-mode");
    run(&mut ed, "exit-insert-mode");
    assert!(!ed.keymaps.is_layer_active("insert"));
}

#[test]
fn toggle_insert_mode_multiple_times() {
    let mut ed = make_editor("");
    for _ in 0..5 {
        run(&mut ed, "enter-insert-mode");
        assert!(ed.keymaps.is_layer_active("insert"));
        run(&mut ed, "exit-insert-mode");
        assert!(!ed.keymaps.is_layer_active("insert"));
    }
}

// ── Append ───────────────────────────────────────────────────────────

#[test]
fn append_moves_cursor_right_and_enters_insert() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    run(&mut ed, "append");
    assert_eq!(cursor(&ed), 1);
    assert!(ed.keymaps.is_layer_active("insert"));
}

// ── Quit ─────────────────────────────────────────────────────────────

#[test]
fn quit_sets_running_false() {
    let mut ed = make_editor("");
    assert!(ed.running);
    run(&mut ed, "quit");
    assert!(!ed.running);
}
