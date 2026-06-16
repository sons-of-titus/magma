use crate::tests::helpers;

// ── Cursor movement ───────────────────────────────────────────────────

#[test]
fn cursor_left_moves_back_one_char() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    // from_string starts cursor at 0; move it to end first
    ed.buffers.get_mut(0).unwrap().set_cursor(5);
    assert_eq!(helpers::cursor(&ed), 5);
    helpers::run(&mut ed, "cursor-left");
    assert_eq!(helpers::cursor(&ed), 4);
}

#[test]
fn cursor_right_moves_forward_one_char() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    helpers::run(&mut ed, "cursor-right");
    assert_eq!(helpers::cursor(&ed), 1);
}

#[test]
fn cursor_left_stops_at_zero() {
    let mut ed = helpers::make_editor_with_buffer("hi");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    helpers::run(&mut ed, "cursor-left");
    assert_eq!(helpers::cursor(&ed), 0);
}

#[test]
fn cursor_right_stops_at_end() {
    let mut ed = helpers::make_editor_with_buffer("hi");
    // from_string starts cursor at 0; move to end then try to go further
    ed.buffers.get_mut(0).unwrap().set_cursor(2);
    assert_eq!(helpers::cursor(&ed), 2);
    helpers::run(&mut ed, "cursor-right");
    assert_eq!(helpers::cursor(&ed), 2);
}

#[test]
fn cursor_left_right_multibyte() {
    let mut ed = helpers::make_editor_with_buffer("héllo");
    // 'é' is 2 bytes; cursor starts at end (byte 6)
    ed.buffers.get_mut(0).unwrap().set_cursor(3); // after 'é'
    helpers::run(&mut ed, "cursor-left");
    assert_eq!(helpers::cursor(&ed), 1); // back to 'é' start
    helpers::run(&mut ed, "cursor-right");
    assert_eq!(helpers::cursor(&ed), 3); // past 'é'
}

#[test]
fn cursor_down_moves_to_next_line() {
    let mut ed = helpers::make_editor_with_buffer("first\nsecond");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    helpers::run(&mut ed, "cursor-down");
    // should be on "second" line
    assert!(helpers::cursor(&ed) >= 6);
}

#[test]
fn cursor_up_moves_to_prev_line() {
    let mut ed = helpers::make_editor_with_buffer("first\nsecond");
    // start on second line
    ed.buffers.get_mut(0).unwrap().set_cursor(6);
    helpers::run(&mut ed, "cursor-up");
    assert!(helpers::cursor(&ed) < 6);
}

// ── Line-start / line-end ─────────────────────────────────────────────

#[test]
fn line_start_moves_to_bol() {
    let mut ed = helpers::make_editor_with_buffer("hello\nworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(8); // somewhere in "world"
    helpers::run(&mut ed, "line-start");
    assert_eq!(helpers::cursor(&ed), 6); // 'w' in "world"
}

#[test]
fn line_end_moves_to_eol() {
    let mut ed = helpers::make_editor_with_buffer("hello\nworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    helpers::run(&mut ed, "line-end");
    assert_eq!(helpers::cursor(&ed), 5); // after 'o' in "hello"
}

// ── Delete / backspace with unicode ───────────────────────────────────

#[test]
fn delete_char_removes_ascii() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(1);
    helpers::run(&mut ed, "delete-char");
    assert_eq!(helpers::buf_text(&ed), "hllo");
}

#[test]
fn delete_char_removes_full_multibyte() {
    let mut ed = helpers::make_editor_with_buffer("héllo");
    ed.buffers.get_mut(0).unwrap().set_cursor(1); // on 'é'
    helpers::run(&mut ed, "delete-char");
    assert_eq!(helpers::buf_text(&ed), "hllo");
}

#[test]
fn backspace_removes_preceding_ascii() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(5);
    helpers::run(&mut ed, "backspace");
    assert_eq!(helpers::buf_text(&ed), "hell");
}

#[test]
fn backspace_removes_preceding_multibyte() {
    let mut ed = helpers::make_editor_with_buffer("héllo");
    ed.buffers.get_mut(0).unwrap().set_cursor(3); // just after 'é'
    helpers::run(&mut ed, "backspace");
    assert_eq!(helpers::buf_text(&ed), "hllo");
}

#[test]
fn backspace_at_start_does_nothing() {
    let mut ed = helpers::make_editor_with_buffer("hi");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    helpers::run(&mut ed, "backspace");
    assert_eq!(helpers::buf_text(&ed), "hi");
}

// ── Newline ───────────────────────────────────────────────────────────

#[test]
fn newline_inserts_at_cursor() {
    let mut ed = helpers::make_editor_with_buffer("helloworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(5);
    helpers::run(&mut ed, "newline");
    assert_eq!(helpers::buf_text(&ed), "hello\nworld");
}

// ── Undo / redo ───────────────────────────────────────────────────────

#[test]
fn undo_command_reverts_insert() {
    let mut ed = helpers::make_editor_with_buffer("");
    {
        let slab = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid)).unwrap();
        ed.buffers.get_mut(slab).unwrap().insert(0, "hello");
    }
    helpers::run(&mut ed, "undo");
    assert_eq!(helpers::buf_text(&ed), "");
}

// ── Modal editing (enter/exit insert mode) ────────────────────────────

#[test]
fn enter_insert_mode_activates_insert_layer() {
    let mut ed = helpers::make_editor_with_buffer("");
    assert!(!ed.keymaps.is_layer_active("insert"));
    helpers::run(&mut ed, "enter-insert-mode");
    assert!(ed.keymaps.is_layer_active("insert"));
}

#[test]
fn exit_insert_mode_deactivates_insert_layer() {
    let mut ed = helpers::make_editor_with_buffer("");
    helpers::run(&mut ed, "enter-insert-mode");
    helpers::run(&mut ed, "exit-insert-mode");
    assert!(!ed.keymaps.is_layer_active("insert"));
}

#[test]
fn toggle_insert_mode_multiple_times() {
    let mut ed = helpers::make_editor_with_buffer("");
    for _ in 0..5 {
        helpers::run(&mut ed, "enter-insert-mode");
        assert!(ed.keymaps.is_layer_active("insert"));
        helpers::run(&mut ed, "exit-insert-mode");
        assert!(!ed.keymaps.is_layer_active("insert"));
    }
}

// ── Append ───────────────────────────────────────────────────────────

#[test]
fn append_moves_cursor_right_and_enters_insert() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    helpers::run(&mut ed, "append");
    assert_eq!(helpers::cursor(&ed), 1);
    assert!(ed.keymaps.is_layer_active("insert"));
}

// ── Quit ─────────────────────────────────────────────────────────────

#[test]
fn quit_sets_running_false() {
    let mut ed = helpers::make_editor_with_buffer("");
    assert!(ed.running);
    helpers::run(&mut ed, "quit");
    assert!(!ed.running);
}
