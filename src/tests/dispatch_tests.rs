use crate::state::Editor;
use crate::state::id::BufferId;
use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;

/// Build an editor with keymaps for dispatch testing.
/// In production, these are loaded from Janet (builtins/vim.janet).
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

    // Mirror the keymaps from vim.janet
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

    // Sprint 11e: populate vim policy registries (Janet-owned at runtime)
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
    ed.vim_char_captures.insert("f".to_string(), "find-forward".to_string());
    ed.vim_char_captures.insert("t".to_string(), "find-till-forward".to_string());
    ed.vim_char_captures.insert("F".to_string(), "find-backward".to_string());
    ed.vim_char_captures.insert("T".to_string(), "find-till-backward".to_string());
    ed.vim_char_captures.insert("r".to_string(), "replace-char".to_string());
    ed.vim_char_captures.insert("m".to_string(), "set-mark".to_string());
    ed.vim_char_captures.insert("q".to_string(), "vim-macro-toggle".to_string());
    ed.vim_char_captures.insert("@".to_string(), "play-macro".to_string());
    ed.vim_char_captures.insert("'".to_string(), "jump-to-mark".to_string());
    ed.vim_char_captures.insert("`".to_string(), "jump-to-mark-char".to_string());
    ed.vim_prefixes.insert("g".to_string());
    ed.vim_prefixes.insert("z".to_string());
    ed.vim_prefixes.insert("Z".to_string());
    ed.vim_prefixes.insert("ctrl-w".to_string());
    ed.vim_prefixes.insert("[".to_string());
    ed.vim_prefixes.insert("]".to_string());
    // Two-key prefix bindings (formerly in handle_prefix Rust match)
    ed.keymaps.set_layer("vim", "gg", "goto-buffer-start");
    ed.keymaps.set_layer("vim", "gi", "go-to-last-insert-pos");
    ed.keymaps.set_layer("vim", "g~", "toggle-case");
    ed.keymaps.set_layer("vim", "gu", "lowercase-region");
    ed.keymaps.set_layer("vim", "gU", "uppercase-region");
    ed.keymaps.set_layer("vim", "gp", "put-after-cursor");
    ed.keymaps.set_layer("vim", "gP", "put-before-cursor");
    ed.keymaps.set_layer("vim", "gv", "reselect-last-visual");
    ed.keymaps.set_layer("vim", "gt", "buffer-next");
    ed.keymaps.set_layer("vim", "gT", "buffer-prev");
    ed.keymaps.set_layer("vim", "zt", "scroll-to-top");
    ed.keymaps.set_layer("vim", "zz", "scroll-to-middle");
    ed.keymaps.set_layer("vim", "zb", "scroll-to-bottom");
    ed.keymaps.set_layer("vim", "ZQ", "force-quit");
    ed.keymaps.set_layer("vim", "ZZ", "save-and-quit");
    ed.keymaps.set_layer("vim", "ctrl-ws", "window-split");
    ed.keymaps.set_layer("vim", "ctrl-wv", "window-vsplit");
    ed.keymaps.set_layer("vim", "ctrl-wq", "window-close");
    ed.keymaps.set_layer("vim", "ctrl-wh", "window-focus-left");
    ed.keymaps.set_layer("vim", "ctrl-wj", "window-focus-down");
    ed.keymaps.set_layer("vim", "ctrl-wk", "window-focus-up");
    ed.keymaps.set_layer("vim", "ctrl-wl", "window-focus-right");
    ed.keymaps.set_layer("vim", "ctrl-ww", "window-next");
    ed.keymaps.set_layer("vim", "[I", "keyword-context");
    ed.keymaps.set_layer("vim", "]I", "find-word-under-cursor");

    ed
}

/// Dispatch a single key exactly as the main loop does.
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

// ── Normal mode ───────────────────────────────────────────────────────

#[test]
fn normal_hjkl_move_cursor() {
    let mut ed = make_editor("abc\ndef");
    ed.buffers.get_mut(0).unwrap().set_cursor(3); // end of first line
    press(&mut ed, "h"); // cursor-left
    assert_eq!(cursor(&ed), 2, "h should move cursor left");
    press(&mut ed, "l"); // cursor-right
    assert_eq!(cursor(&ed), 3, "l should move cursor right");
}

#[test]
fn normal_dl_deletes_char_under_cursor() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(1); // on 'e'
    press(&mut ed, "d");
    press(&mut ed, "l"); // d + l = delete right
    assert_eq!(text(&ed), "hllo", "dl should delete the char under cursor");
}

#[test]
fn normal_dw_deletes_word_forward() {
    let mut ed = make_editor("hello world foo");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "d");
    press(&mut ed, "w"); // d + w = delete word forward
    assert_eq!(text(&ed), "world foo", "dw should delete the first word and trailing space");
}

#[test]
fn normal_dd_deletes_line() {
    let mut ed = make_editor("hello\nworld");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "d");
    press(&mut ed, "d"); // dd = delete line
    assert_eq!(text(&ed), "world", "dd should delete the current line");
}

#[test]
fn normal_df_deletes_until_char() {
    let mut ed = make_editor("hello.world");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "d");
    press(&mut ed, "f"); // d + f + char = delete until char
    press(&mut ed, "."); // target char
    assert_eq!(text(&ed), "world", "df. should delete up to and including the dot");
}

#[test]
fn normal_f_moves_to_char() {
    let mut ed = make_editor("hello.world");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "f");
    press(&mut ed, "."); // jump to '.'
    assert_eq!(cursor(&ed), 5, "f. should move cursor to the dot");
}

#[test]
fn normal_x_deletes_char_under_cursor() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "x");
    assert_eq!(text(&ed), "ello");
}

#[test]
fn normal_u_undoes_last_change() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "x"); // delete 'h' → "ello"
    assert_eq!(text(&ed), "ello");
    press(&mut ed, "u"); // undo
    assert_eq!(text(&ed), "hello");
}

// ── Insert mode: vim keys must insert characters, not execute commands ──

#[test]
fn insert_mode_h_inserts_not_moves_cursor() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(2);
    press(&mut ed, "i"); // enter insert mode
    assert_eq!(ed.vim_mode_name(), "INSERT");
    press(&mut ed, "h"); // must insert 'h', NOT run cursor-left
    assert_eq!(text(&ed), "abhc", "h must be inserted in insert mode");
    assert_eq!(cursor(&ed), 3, "cursor must advance after insert, not move left");
}

#[test]
fn insert_mode_j_inserts_not_moves_down() {
    let mut ed = make_editor("abc\ndef");
    ed.buffers.get_mut(0).unwrap().set_cursor(1);
    press(&mut ed, "i");
    press(&mut ed, "j");
    assert!(text(&ed).contains('j'), "j must be inserted as text in insert mode");
    assert_eq!(ed.vim_mode_name(), "INSERT");
}

#[test]
fn insert_mode_k_inserts_not_moves_up() {
    let mut ed = make_editor("abc\ndef");
    ed.buffers.get_mut(0).unwrap().set_cursor(5); // on 'd'
    press(&mut ed, "i");
    press(&mut ed, "k");
    assert!(text(&ed).contains('k'), "k must be inserted as text in insert mode");
}

#[test]
fn insert_mode_l_inserts_not_moves_right() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "i");
    press(&mut ed, "l");
    assert_eq!(text(&ed), "labc");
}

#[test]
fn insert_mode_u_inserts_not_undoes() {
    let mut ed = make_editor("hello");
    ed.buffers.get_mut(0).unwrap().set_cursor(5);
    press(&mut ed, "i");
    press(&mut ed, "u");
    // buffer should have grown by one char (u inserted), NOT shrunk (undo)
    assert_eq!(text(&ed).len(), 6, "u must insert in insert mode, not undo");
    assert!(text(&ed).ends_with('u'));
}

#[test]
fn insert_mode_i_inserts_not_enters_insert_again() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "i"); // enter insert
    press(&mut ed, "i"); // must insert 'i', NOT re-trigger enter-insert-mode
    assert_eq!(text(&ed), "iabc");
    assert_eq!(ed.vim_mode_name(), "INSERT");
}

#[test]
fn insert_mode_v_inserts_not_enters_visual() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(0);
    press(&mut ed, "i");
    press(&mut ed, "v");
    assert_eq!(text(&ed), "vabc");
    assert_eq!(ed.vim_mode_name(), "INSERT"); // stayed in INSERT, not VISUAL
}

#[test]
fn insert_mode_sequence_of_text() {
    let mut ed = make_editor("");
    press(&mut ed, "i");
    for ch in ["h", "e", "l", "l", "o"] {
        press(&mut ed, ch);
    }
    assert_eq!(text(&ed), "hello");
}

#[test]
fn insert_mode_esc_returns_to_normal() {
    let mut ed = make_editor("abc");
    ed.buffers.get_mut(0).unwrap().set_cursor(1);
    press(&mut ed, "i");
    assert_eq!(ed.vim_mode_name(), "INSERT");
    press(&mut ed, "esc");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    // vim bindings must be restored
    press(&mut ed, "h"); // cursor-left
    assert_eq!(cursor(&ed), 0);
}

#[test]
fn insert_then_normal_then_insert_roundtrip() {
    let mut ed = make_editor("x");
    // i → type → esc → i → type → esc
    press(&mut ed, "i");
    press(&mut ed, "a");
    press(&mut ed, "esc");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    press(&mut ed, "i");
    press(&mut ed, "b");
    press(&mut ed, "esc");
    assert_eq!(ed.vim_mode_name(), "NORMAL");
    // Both 'a' and 'b' were inserted; exact positions depend on cursor movement
    assert!(text(&ed).contains('a') && text(&ed).contains('b'));
}
