//! Keymap/GUI contract guards.
//!
//! The GUI consumes a small set of chords in normal mode (see
//! `DialogManager::shortcut` and the ctrl-q/ctrl-\ handlers in gui.rs).
//! If a keymap layer ever binds them to something else, one of the two
//! silently dies — this suite makes that drift a test failure.

/// Keys the GUI consumes in normal mode; none may be keymap-bound there.
const GUI_RESERVED_NORMAL: &[&str] = &[
    "ctrl-p",       // search-everywhere palette
    "ctrl-shift-n", // go to file
    "ctrl-shift-f", // search dialog
    "ctrl-shift-o", // go to symbol
    "ctrl-comma",   // settings
    "ctrl-q",       // quit guard
    "ctrl-\\",      // sidebar toggle
    "ctrl-shift-l", // theme toggle
];

fn enter_normal_mode(ed: &mut crate::kernel::state::Editor) {
    ed.keymaps.push_layer("editor");
}

#[test]
fn gui_reserved_keys_are_free_in_normal_mode() {
    janet_test!(ed, {
        enter_normal_mode(&mut ed);
        for key in GUI_RESERVED_NORMAL {
            assert!(
                ed.keymaps.resolve(key).is_none(),
                "{key} is reserved by the GUI in normal mode, but the keymap binds it to {:?}",
                ed.keymaps.resolve(key),
            );
        }
    });
}

#[test]
fn completion_bindings_work_while_typing() {
    // Non-modal: the completion chords live in the default layer, so they
    // are live whenever the user is typing — no mode entry required.
    janet_test!(ed, {
        enter_normal_mode(&mut ed);
        assert_eq!(ed.keymaps.resolve("ctrl-n").as_deref(), Some("completion-next"));
        assert_eq!(ed.keymaps.resolve("ctrl-space").as_deref(), Some("completion-trigger"));
        assert_eq!(ed.keymaps.resolve("esc").as_deref(), Some("editor-escape"));
    });
}

#[test]
fn ctrl_o_still_means_jump_back_in_normal_mode() {
    // The review's finding: the GUI hijacked ctrl-o and vim's jump-back
    // silently vanished.  It is restored; the palette owns ctrl-p.
    janet_test!(ed, {
        enter_normal_mode(&mut ed);
        assert_eq!(ed.keymaps.resolve("ctrl-o").as_deref(), Some("mark-ring-jump-back"));
        assert_eq!(ed.keymaps.resolve("ctrl-i").as_deref(), Some("mark-ring-jump-forward"));
    });
}
