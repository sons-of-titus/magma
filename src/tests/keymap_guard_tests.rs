//! Keymap/GUI contract guards.
//!
//! The GUI consumes a small set of chords in normal mode (see
//! `DialogManager::shortcut` and the ctrl-q/ctrl-\ handlers in gui.rs).
//! If a keymap layer ever binds them to something else, one of the two
//! silently dies — this suite makes that drift a test failure.

/// Keys the GUI consumes in normal mode; none may be keymap-bound there.
const GUI_RESERVED_NORMAL: &[&str] = &[
    "ctrl-shift-n", // go to file
    "ctrl-shift-f", // search dialog
    "ctrl-shift-o", // go to symbol
    "ctrl-alt-s",   // settings (JetBrains)
    "ctrl-comma",   // settings (macOS)
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
    // silently vanished.  It is restored; the GUI takes no ctrl-p.
    janet_test!(ed, {
        enter_normal_mode(&mut ed);
        assert_eq!(ed.keymaps.resolve("ctrl-o").as_deref(), Some("mark-ring-jump-back"));
        assert_eq!(ed.keymaps.resolve("ctrl-i").as_deref(), Some("mark-ring-jump-forward"));
    });
}

#[test]
fn horizontal_scroll_chords_resolve_in_normal_mode() {
    janet_test!(ed, {
        enter_normal_mode(&mut ed);
        assert_eq!(ed.keymaps.resolve("ctrl-shift-left").as_deref(),
            Some("scroll-column-left"));
        assert_eq!(ed.keymaps.resolve("ctrl-shift-right").as_deref(),
            Some("scroll-column-right"));
        // ctrl/meta + arrow keep their pre-prefix cursor behavior.
        assert_eq!(ed.keymaps.resolve("ctrl-left").as_deref(), Some("cursor-left"));
        assert_eq!(ed.keymaps.resolve("meta-right").as_deref(), Some("cursor-right"));
    });
}

#[test]
fn debug_keys_follow_intellij_defaults() {
    janet_test!(ed, {
        enter_normal_mode(&mut ed);
        assert_eq!(ed.keymaps.resolve("f7").as_deref(),       Some("dbg-step-in"));
        assert_eq!(ed.keymaps.resolve("f8").as_deref(),       Some("dbg-step-over"));
        assert_eq!(ed.keymaps.resolve("shift-f8").as_deref(), Some("dbg-step-out"));
        assert_eq!(ed.keymaps.resolve("f9").as_deref(),       Some("dbg-continue"));
        assert_eq!(ed.keymaps.resolve("meta-f8").as_deref(),  Some("dbg-bp"));
    });
}

// ── Variadic command dispatch must not fabricate arguments ──────────────────

#[test]
fn variadic_janet_command_from_keymap_gets_zero_args() {
    use crate::kernel::scripting;
    // The regression: `(fn [& args] …)` commands received one empty-string
    // argument, so `(length args)` was 1 with `(args 0)` a string — and
    // commands reading a count from it (indent-line's `(for i 0 count)`)
    // looped forever on every keymap press of tab.
    janet_test!(ed, {
        let r = scripting::eval_result(r#"
            (do
              (var arg-len nil)
              (command/define "bisect-variadic" (fn [& args]
                (set arg-len (length args))))
              (editor/run-command "bisect-variadic")
              (string arg-len))
        "#).unwrap_or_default();
        assert_eq!(r, "\"0\"",
            "a variadic command invoked without arguments must see an empty args tuple");
    });
}
