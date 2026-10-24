use crate::tests::helpers;

fn set_cursor(ed: &mut crate::kernel::state::Editor, offset: usize) {
    let key = helpers::focused_key(ed);
    ed.views.get_mut(&key).unwrap().set_cursor(offset);
}

// ── Cursor movement ───────────────────────────────────────────────────

#[test]
fn cursor_left_moves_back_one_char() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    // from_string starts cursor at 0; move it to end first
    set_cursor(&mut ed, 5);
    assert_eq!(helpers::cursor(&ed), 5);
    helpers::run(&mut ed, "cursor-left");
    assert_eq!(helpers::cursor(&ed), 4);
}

#[test]
fn cursor_right_moves_forward_one_char() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    set_cursor(&mut ed, 0);
    helpers::run(&mut ed, "cursor-right");
    assert_eq!(helpers::cursor(&ed), 1);
}

#[test]
fn cursor_left_stops_at_zero() {
    let mut ed = helpers::make_editor_with_buffer("hi");
    set_cursor(&mut ed, 0);
    helpers::run(&mut ed, "cursor-left");
    assert_eq!(helpers::cursor(&ed), 0);
}

#[test]
fn cursor_right_stops_at_end() {
    let mut ed = helpers::make_editor_with_buffer("hi");
    // from_string starts cursor at 0; move to end then try to go further
    set_cursor(&mut ed, 2);
    assert_eq!(helpers::cursor(&ed), 2);
    helpers::run(&mut ed, "cursor-right");
    assert_eq!(helpers::cursor(&ed), 2);
}

#[test]
fn cursor_left_right_multibyte() {
    let mut ed = helpers::make_editor_with_buffer("héllo");
    // 'é' is 2 bytes; cursor starts at end (byte 6)
    set_cursor(&mut ed, 3); // after 'é'
    helpers::run(&mut ed, "cursor-left");
    assert_eq!(helpers::cursor(&ed), 1); // back to 'é' start
    helpers::run(&mut ed, "cursor-right");
    assert_eq!(helpers::cursor(&ed), 3); // past 'é'
}

#[test]
fn cursor_down_moves_to_next_line() {
    let mut ed = helpers::make_editor_with_buffer("first\nsecond");
    set_cursor(&mut ed, 0);
    helpers::run(&mut ed, "cursor-down");
    // should be on "second" line
    assert!(helpers::cursor(&ed) >= 6);
}

#[test]
fn cursor_up_moves_to_prev_line() {
    let mut ed = helpers::make_editor_with_buffer("first\nsecond");
    // start on second line
    set_cursor(&mut ed, 6);
    helpers::run(&mut ed, "cursor-up");
    assert!(helpers::cursor(&ed) < 6);
}

// ── Line-start / line-end ─────────────────────────────────────────────

#[test]
fn line_start_moves_to_bol() {
    let mut ed = helpers::make_editor_with_buffer("hello\nworld");
    set_cursor(&mut ed, 8); // somewhere in "world"
    helpers::run(&mut ed, "line-start");
    assert_eq!(helpers::cursor(&ed), 6); // 'w' in "world"
}

#[test]
fn line_end_moves_to_eol() {
    let mut ed = helpers::make_editor_with_buffer("hello\nworld");
    set_cursor(&mut ed, 0);
    helpers::run(&mut ed, "line-end");
    assert_eq!(helpers::cursor(&ed), 5); // after 'o' in "hello"
}

// ── Delete / backspace with unicode ───────────────────────────────────

#[test]
fn delete_char_removes_ascii() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    set_cursor(&mut ed, 1);
    helpers::run(&mut ed, "delete-char");
    assert_eq!(helpers::buf_text(&ed), "hllo");
}

#[test]
fn delete_char_removes_full_multibyte() {
    let mut ed = helpers::make_editor_with_buffer("héllo");
    set_cursor(&mut ed, 1); // on 'é'
    helpers::run(&mut ed, "delete-char");
    assert_eq!(helpers::buf_text(&ed), "hllo");
}

#[test]
fn backspace_removes_preceding_ascii() {
    let mut ed = helpers::make_editor_with_buffer("hello");
    set_cursor(&mut ed, 5);
    helpers::run(&mut ed, "backspace");
    assert_eq!(helpers::buf_text(&ed), "hell");
}

#[test]
fn backspace_removes_preceding_multibyte() {
    let mut ed = helpers::make_editor_with_buffer("héllo");
    set_cursor(&mut ed, 3); // just after 'é'
    helpers::run(&mut ed, "backspace");
    assert_eq!(helpers::buf_text(&ed), "hllo");
}

#[test]
fn backspace_at_start_does_nothing() {
    let mut ed = helpers::make_editor_with_buffer("hi");
    set_cursor(&mut ed, 0);
    helpers::run(&mut ed, "backspace");
    assert_eq!(helpers::buf_text(&ed), "hi");
}

// ── Newline ───────────────────────────────────────────────────────────

#[test]
fn newline_inserts_at_cursor() {
    let mut ed = helpers::make_editor_with_buffer("helloworld");
    set_cursor(&mut ed, 5);
    helpers::run(&mut ed, "newline");
    assert_eq!(helpers::buf_text(&ed), "hello\nworld");
}

// ── Undo / redo ───────────────────────────────────────────────────────

#[test]
fn undo_command_reverts_insert() {
    let mut ed = helpers::make_editor_with_buffer("");
    {
        let slab = ed.view_tree.focused_window()
            .and_then(|wid| ed.view_tree.buffer(wid)).unwrap();
        ed.views.get_mut(&slab).unwrap().insert(0, "hello");
    }
    helpers::run(&mut ed, "undo");
    assert_eq!(helpers::buf_text(&ed), "");
}

// ── Non-modal editing ─────────────────────────────────────────────────

#[test]
fn typing_inserts_without_entering_any_mode() {
    let mut ed = helpers::make_editor_with_buffer("");
    assert!(ed.editor_mode.accepts_text);
    crate::kernel::input::dispatch_key(&mut ed, "h");
    crate::kernel::input::dispatch_key(&mut ed, "i");
    assert_eq!(helpers::buf_text(&ed), "hi");
    assert!(ed.editor_mode.is_named("editor"), "still the one mode");
}

// ── Quit ─────────────────────────────────────────────────────────────

#[test]
fn quit_sets_running_false() {
    let mut ed = helpers::make_editor_with_buffer("");
    assert!(ed.running);
    helpers::run(&mut ed, "quit");
    assert!(!ed.running);
}

// ── goto-matching-brace ──────────────────────────────────────────────

#[test]
fn goto_matching_brace_survives_non_boundary_cursor() {
    // é is two bytes; a cursor landed mid-codepoint (possible after a
    // UTF-16 → byte conversion) used to panic the slice.  Snapping down
    // lands on é itself — not a bracket — so the cursor must stay put.
    let mut ed = helpers::make_editor_with_buffer("(println \"héllo\")");
    let key = helpers::focused_key(&ed);
    let mid_char = "(println \"h".len() + 1; // inside é
    ed.views.get_mut(&key).unwrap().set_cursor(mid_char);
    helpers::run(&mut ed, "goto-matching-brace");
    assert_eq!(helpers::cursor(&ed), mid_char, "no bracket at the snapped char: cursor stays");
}

#[test]
fn goto_matching_brace_from_mid_char_after_opener() {
    let mut ed = helpers::make_editor_with_buffer("(é)");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(2); // second byte of é
    helpers::run(&mut ed, "goto-matching-brace");
    assert_eq!(helpers::cursor(&ed), 2, "snap lands on é, not a bracket: cursor stays");
}

#[test]
fn goto_matching_brace_backward_from_end_of_buffer() {
    // The bracket candidate behind a cursor at end-of-buffer used to slice
    // past the last byte.
    let mut ed = helpers::make_editor_with_buffer("(é)");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor("(é)".len());
    helpers::run(&mut ed, "goto-matching-brace");
    assert_eq!(helpers::cursor(&ed), 0, "closing paren at EOB jumps to its opener");
}

// ── Word motions on the chunk-scan path ──────────────────────────────

#[test]
fn word_motions_walk_boundaries() {
    let mut ed = helpers::make_editor_with_buffer("foo bar  baz qux");
    let key = helpers::focused_key(&ed);

    // w from 0 → start of "bar" (4)
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    helpers::run(&mut ed, "move-word-forward");
    assert_eq!(helpers::cursor(&ed), 4);

    // w again → start of "baz" (9, past the double space)
    helpers::run(&mut ed, "move-word-forward");
    assert_eq!(helpers::cursor(&ed), 9);

    // b → back to "bar"
    helpers::run(&mut ed, "move-word-back");
    assert_eq!(helpers::cursor(&ed), 4);

    // e on "foo" → its end (3)
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    helpers::run(&mut ed, "move-word-end");
    assert_eq!(helpers::cursor(&ed), 3);
}

#[test]
fn word_motions_handle_multibyte_words() {
    let mut ed = helpers::make_editor_with_buffer("héllo wörld");
    let key = helpers::focused_key(&ed);
    ed.views.get_mut(&key).unwrap().set_cursor(0);
    helpers::run(&mut ed, "move-word-forward");
    assert_eq!(helpers::cursor(&ed), 7, "wörld starts after 'héllo ' (é is 2 bytes)");
    helpers::run(&mut ed, "move-word-back");
    assert_eq!(helpers::cursor(&ed), 0);
}

#[test]
fn search_repeat_wraps_in_both_directions() {
    let mut ed = helpers::make_editor_with_buffer("x ay x ay x");
    let key = helpers::focused_key(&ed);

    ed.search.pattern = Some("ay".to_string());
    ed.search.forward = true;
    helpers::run(&mut ed, "repeat-search");
    assert_eq!(helpers::cursor(&ed), 2);
    helpers::run(&mut ed, "repeat-search");
    assert_eq!(helpers::cursor(&ed), 7);
    helpers::run(&mut ed, "repeat-search");
    assert_eq!(helpers::cursor(&ed), 2, "forward search wraps to the first match");

    helpers::run(&mut ed, "reverse-search");
    assert_eq!(helpers::cursor(&ed), 7, "backward wraps to the last match");

    let highlights = {
        let v = ed.views.get(&key).unwrap();
        v.highlights.len()
    };
    assert_eq!(highlights, 2, "all occurrences highlight");
    let _ = &key;
}

// ── Regex + replace ──────────────────────────────────────────────────

#[test]
fn replace_all_is_one_undo_step() {
    let mut ed = helpers::make_editor_with_buffer("cat hat bat");
    let key = helpers::focused_key(&ed);
    let before_undo = ed.buffers.get(key).unwrap().lock().unwrap().undo_len();

    crate::kernel::command::builtin::search_query::replace_all(
        &mut ed, "at", "op", false).unwrap();
    assert_eq!(helpers::buf_text(&ed), "cop hop bop");

    // One undo STEP: the three replacements share a group even though
    // each is its own transaction.
    let after_undo = ed.buffers.get(key).unwrap().lock().unwrap().undo_len();
    assert_eq!(after_undo, before_undo + 3, "three transactions");
    helpers::run(&mut ed, "undo");
    assert_eq!(helpers::buf_text(&ed), "cat hat bat");
}

#[test]
fn replace_all_regex_uses_patterns() {
    let mut ed = helpers::make_editor_with_buffer("fn a() fn b()");
    crate::kernel::command::builtin::search_query::replace_all(
        &mut ed, r"fn (\w)", "fn m_$1", true).unwrap();
    assert_eq!(helpers::buf_text(&ed), "fn m_a() fn m_b()");
}

#[test]
fn replace_next_walks_matches() {
    let mut ed = helpers::make_editor_with_buffer("aa aa aa");
    use crate::kernel::command::builtin::search_query as q;
    assert!(q::replace_next(&mut ed, "aa", "bb", false).unwrap());
    assert_eq!(helpers::cursor(&ed), 2);
    assert!(q::replace_next(&mut ed, "aa", "bb", false).unwrap());
    assert_eq!(helpers::buf_text(&ed), "bb bb aa");
    // All replaced: the final call wraps, finds nothing, reports false.
    assert!(q::replace_next(&mut ed, "aa", "bb", false).unwrap());
    assert!(!q::replace_next(&mut ed, "aa", "bb", false).unwrap());
}

#[test]
fn smart_case_replaces_insensitively() {
    let mut ed = helpers::make_editor_with_buffer("Hello hello");
    crate::kernel::command::builtin::search_query::replace_all(
        &mut ed, "hello", "x", false).unwrap();
    assert_eq!(helpers::buf_text(&ed), "x x");
}
