use crate::state::Editor;
use crate::tests::helpers;

fn make_editor(content: &str) -> Editor {
    let mut ed = helpers::make_editor_with_buffer(content);

    ed.keymaps.push_layer("vim");
    ed.keymaps.set_layer("vim", "h",  "cursor-left");
    ed.keymaps.set_layer("vim", "j",  "cursor-down");
    ed.keymaps.set_layer("vim", "k",  "cursor-up");
    ed.keymaps.set_layer("vim", "l",  "cursor-right");
    ed.keymaps.set_layer("vim", "i",  "enter-insert-mode");
    ed.keymaps.set_layer("vim", "a",  "append");
    ed.keymaps.set_layer("vim", "o",  "open-line-below");
    ed.keymaps.set_layer("vim", "v",  "enter-visual-mode");
    ed.keymaps.set_layer("vim", "R",  "enter-replace-mode");
    ed.keymaps.set_layer("vim", ":",  "enter-command-mode");
    ed.keymaps.set_layer("vim", "x",  "delete-char");
    ed.keymaps.set_layer("vim", "X",  "backspace");
    ed.keymaps.set_layer("vim", "u",  "undo");
    ed.keymaps.set_layer("vim", "0",  "line-start");
    ed.keymaps.set_layer("vim", "w",  "move-word-forward");
    ed.keymaps.set_layer("vim", "b",  "move-word-back");
    ed.keymaps.set_layer("vim", "e",  "move-word-end");
    ed.keymaps.set_layer("vim", "p",  "put");
    ed.keymaps.set_layer("vim", "P",  "put-before");
    ed.keymaps.set_layer("insert", "esc",       "exit-insert-mode");
    ed.keymaps.set_layer("insert", "backspace", "backspace");
    ed.keymaps.set_layer("insert", "return",    "newline");
    ed.keymaps.set_layer("insert", "left",      "cursor-left");
    ed.keymaps.set_layer("insert", "right",     "cursor-right");
    ed.keymaps.set_layer("visual", "esc",  "exit-visual-mode");
    ed.keymaps.set_layer("visual", "d",    "delete-selection");
    ed.keymaps.set_layer("visual", "h",    "cursor-left");
    ed.keymaps.set_layer("visual", "l",    "cursor-right");
    ed.keymaps.set_layer("visual", "y",    "yank-selection");
    ed.keymaps.set_layer("replace", "esc", "exit-replace-mode");
    ed.keymaps.set_layer("command", "esc",       "exit-command-mode");
    ed.keymaps.set_layer("command", "return",    "command-execute");
    ed.keymaps.set_layer("command", "backspace", "command-backspace");
    ed.keymaps.set("ctrl-q", "quit");
    ed.keymaps.set("ctrl-s", "save-buffer");

    // Sprint 11e: populate the vim policy registries so dispatch_key works
    // without Janet being loaded.  Janet populates these from vim.janet at
    // runtime; tests that don't load Janet must do it manually.
    ed.vim_operators.insert("d".to_string(), "delete-line".to_string());
    ed.vim_operators.insert("c".to_string(), "change-line".to_string());
    ed.vim_operators.insert("y".to_string(), "yank-line".to_string());
    ed.vim_operators.insert(">".to_string(), "indent-line".to_string());
    ed.vim_operators.insert("<".to_string(), "deindent-line".to_string());
    ed.vim_operators.insert("=".to_string(), "autoindent-line".to_string());

    ed.vim_motions.insert("h".to_string(), "cursor-left".to_string());
    ed.vim_motions.insert("j".to_string(), "cursor-down".to_string());
    ed.vim_motions.insert("k".to_string(), "cursor-up".to_string());
    ed.vim_motions.insert("l".to_string(), "cursor-right".to_string());
    ed.vim_motions.insert("w".to_string(), "move-word-forward".to_string());
    ed.vim_motions.insert("b".to_string(), "move-word-back".to_string());
    ed.vim_motions.insert("e".to_string(), "move-word-end".to_string());
    ed.vim_motions.insert("0".to_string(), "line-start".to_string());
    ed.vim_motions.insert("$".to_string(), "line-end".to_string());
    ed.vim_motions.insert("G".to_string(), "goto-buffer-end".to_string());

    ed
}

fn press(ed: &mut Editor, key: &str) {
    crate::input::dispatch_key(ed, key);
}

fn text(ed: &Editor) -> String {
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
    use crate::state::mode::Mode;
    if let Mode::Command { ref input } = ed.mode { input } else { "" }
}

// ── Replace mode ──────────────────────────────────────────────────────

#[test]
fn replace_mode_overwrites_char_via_key_dispatch() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "R"); // enter-replace-mode
    assert_eq!(ed.vim_mode_name(), "REPLACE");
    press(&mut ed, "H"); // should overwrite 'h' with 'H', not trigger vim
    assert!(text(&ed).starts_with('H'));
    assert_eq!(text(&ed).len(), 5);
}

#[test]
fn replace_mode_esc_restores_vim_bindings() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(3);
    press(&mut ed, "R");
    press(&mut ed, "esc");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    press(&mut ed, "h"); // cursor-left must work again
    assert_eq!(cursor(&ed), 2);
}

// ── Command mode ──────────────────────────────────────────────────────

#[test]
fn command_mode_letters_go_to_input_not_commands() {
    let mut ed = make_editor("hello");
    press(&mut ed, ":"); // enter-command-mode
    assert_eq!(ed.vim_mode_name(), "COMMAND");
    // h,j,k,l must append to command input, NOT run cursor-left etc.
    press(&mut ed, "h");
    press(&mut ed, "j");
    press(&mut ed, "w");
    assert_eq!(command_input(&ed), "hjw");
    // buffer must be unchanged
    assert_eq!(text(&ed), "hello");
}

#[test]
fn command_mode_esc_returns_to_normal_with_vim_bindings() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(2);
    press(&mut ed, ":");
    press(&mut ed, "esc");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    press(&mut ed, "h");
    assert_eq!(cursor(&ed), 1, "h must be cursor-left after exiting command mode");
}

// ── Visual mode ───────────────────────────────────────────────────────

#[test]
fn visual_mode_hjkl_move_cursor() {
    let mut ed = make_editor("hello world");
    ed.buffers.get_mut(0).unwrap().set_cursor(5);
    press(&mut ed, "v"); // enter-visual-mode
    assert_eq!(ed.vim_mode_name(), "VISUAL");
    press(&mut ed, "h"); // cursor-left (from visual layer)
    assert_eq!(cursor(&ed), 4);
}

#[test]
fn visual_mode_d_deletes_selection() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "v");
    press(&mut ed, "l"); // extend selection rightward
    press(&mut ed, "l");
    press(&mut ed, "d"); // delete-selection
    assert_eq!(ed.vim_mode_name(), "NORMAL");
}

#[test]
fn visual_mode_esc_returns_to_normal() {
    let mut ed = make_editor("abc");
    press(&mut ed, "v");
    press(&mut ed, "esc");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    // vim bindings must still work
    press(&mut ed, "h");
    assert_eq!(cursor(&ed), 0); // was already 0, cursor-left is clamped
}

// ── Operator-pending: c (change) ──────────────────────────────────────

#[test]
fn normal_cw_changes_word_forward() {
    let mut ed = make_editor("hello world");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "c");
    press(&mut ed, "w");
    assert_eq!(ed.vim_mode_name(), "INSERT", "cw enters insert mode");
    assert_eq!(text(&ed), "world", "cw deletes the first word and trailing space, enters insert");
}

#[test]
fn normal_cc_changes_line() {
    let mut ed = make_editor("hello\nworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "c");
    press(&mut ed, "c");
    assert_eq!(ed.vim_mode_name(), "INSERT", "cc enters insert mode");
    assert_eq!(text(&ed), "world", "cc deletes the current line");
}

#[test]
fn normal_cl_deletes_char_and_enters_insert() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "c");
    press(&mut ed, "l");
    assert_eq!(ed.vim_mode_name(), "INSERT");
    assert_eq!(text(&ed), "ello");
}

// ── Operator-pending: y (yank) ──────────────────────────────────────────

#[test]
fn normal_yw_yanks_word_forward() {
    let mut ed = make_editor("hello world");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "y");
    press(&mut ed, "w");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    assert_eq!(text(&ed), "hello world", "yw does not delete text");
    assert_eq!(ed.yanked_text.as_deref(), Some("hello "), "yw yanks word + space");
}

#[test]
fn normal_yy_yanks_line() {
    let mut ed = make_editor("hello\nworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "y");
    press(&mut ed, "y");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    assert_eq!(text(&ed), "hello\nworld");
    assert_eq!(ed.yanked_text.as_deref(), Some("hello\n"), "yy yanks line with newline");
}

// ── Put / paste ──────────────────────────────────────────────────────────

#[test]
fn normal_p_pastes_yanked_text() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    // Yank "a"
    press(&mut ed, "y");
    press(&mut ed, "l");
    assert_eq!(ed.yanked_text.as_deref(), Some("a"));
    // Paste after cursor
    press(&mut ed, "p");
    assert_eq!(text(&ed), "aabc", "p pastes yanked text after cursor");
}

#[test]
#[allow(non_snake_case)]
fn normal_P_pastes_before_cursor() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(1);
    press(&mut ed, "y");
    press(&mut ed, "l");
    assert_eq!(ed.yanked_text.as_deref(), Some("b"));
    press(&mut ed, "P");
    assert_eq!(text(&ed), "abbc", "P pastes yanked text before cursor");
}

#[test]
fn normal_dd_leaves_yanked_text_for_put() {
    let mut ed = make_editor("hello\nworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "d");
    press(&mut ed, "d");
    assert_eq!(text(&ed), "world");
    assert!(ed.yanked_text.as_deref().unwrap_or("").contains("hello"));
}

// ── Repeat (.) ───────────────────────────────────────────────────────────

#[test]
fn normal_dot_repeats_line_delete() {
    let mut ed = make_editor("a\nb\nc");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "d");
    press(&mut ed, "d"); // dd deletes first line
    assert_eq!(text(&ed), "b\nc");
    press(&mut ed, "."); // repeat dd on second line
    assert_eq!(text(&ed), "c");
}

#[test]
fn normal_dot_repeats_op_motion() {
    let mut ed = make_editor("hello world foo");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "d");
    press(&mut ed, "w"); // dw deletes "hello "
    assert_eq!(text(&ed), "world foo");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "."); // repeat dw → deletes "world "
    assert_eq!(text(&ed), "foo");
}

#[test]
fn normal_dot_repeats_with_count() {
    let mut ed = make_editor("aa bb cc dd");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "2");
    press(&mut ed, "d");
    press(&mut ed, "w"); // 2dw → delete 2 words
    assert_eq!(text(&ed), "cc dd", "2dw deletes two words and trailing space");
    // Move cursor to start
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "."); // repeat with last count → delete 2 more words
    assert_eq!(text(&ed), "", ". repeats 2dw");
}

// ── Visual yank ──────────────────────────────────────────────────────────

#[test]
fn visual_y_yanks_selection() {
    let mut ed = make_editor("hello world");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "v");
    press(&mut ed, "l");
    press(&mut ed, "l");
    press(&mut ed, "y"); // yank-selection → yanks "hel"
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    assert_eq!(ed.yanked_text.as_deref(), Some("hel"));
}
