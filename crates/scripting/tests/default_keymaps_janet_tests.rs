//! The shipped default keymap: every binding must name a command that
//! actually exists, the `editor` layer must be active after boot, and a
//! sample of keys must produce their observable effects.

#[macro_use]
mod helpers;

use kernel::input::dispatch_key;
use kernel::state::Editor;
use scripting::EditorScope;

fn booted_editor() -> Editor {
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    ed
}

#[test]
fn editor_layer_is_active_after_boot() {
    janet_test!(ed, {
        assert!(
            ed.keymaps.is_layer_active("editor"),
            "the default keymap lives in the `editor` layer; it must be active"
        );
    });
}

#[test]
fn every_default_binding_names_a_registered_command() {
    janet_test!(ed, {
        let mut dangling: Vec<String> = Vec::new();
        for (layer, bindings) in ed.keymaps.list_all() {
            for (key, cmd) in bindings {
                if !ed.commands.exists(&cmd) {
                    dangling.push(format!("[{layer}] {key} -> {cmd}"));
                }
            }
        }
        assert!(
            dangling.is_empty(),
            "default keymap binds keys to commands that do not exist:\n  {}",
            dangling.join("\n  ")
        );
    });
}

#[test]
fn default_bindings_resolve() {
    janet_test!(ed, {
        for key in [
            "left", "right", "up", "down", "home", "end", "page-up", "page-down", "backspace",
            "delete", "return", "tab", "esc", "ctrl-c", "ctrl-x", "ctrl-v", "ctrl-a", "ctrl-f",
            "ctrl-h", "ctrl-s", "ctrl-z", "ctrl-w", "ctrl-q", "ctrl-p", "ctrl-n", "ctrl-space",
            "ctrl-]", "ctrl-o", "ctrl-i", "ctrl-l", "ctrl-=", "ctrl--", "ctrl-0", "ctrl-shift-f",
            "ctrl-shift-o", "ctrl-shift-p", "ctrl-shift-z", "shift-left", "shift-right",
            "shift-up", "shift-down", "shift-home", "shift-end",
        ] {
            assert_eq!(
                ed.keymaps.resolve_for_buffer(key, Some(0)),
                ed.keymaps.resolve(key),
                "{key} must resolve identically through the buffer-aware path"
            );
            assert!(
                ed.keymaps.resolve(key).is_some(),
                "{key} must be bound after boot"
            );
        }
    });
}

#[test]
fn arrow_keys_move_the_cursor() {
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("probe.txt", "hello\nworld");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }

        dispatch_key(&mut ed, "end");
        assert_eq!(helpers::cursor(&ed), 5);
        dispatch_key(&mut ed, "home");
        assert_eq!(helpers::cursor(&ed), 0);
        dispatch_key(&mut ed, "right");
        dispatch_key(&mut ed, "right");
        dispatch_key(&mut ed, "down");
        assert_eq!(helpers::cursor(&ed), 8, "down keeps the column (line 2 col 2)");
        dispatch_key(&mut ed, "up");
        assert_eq!(helpers::cursor(&ed), 2);
    });
}

#[test]
fn editing_keys_edit() {
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("probe.txt", "hello");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }

        dispatch_key(&mut ed, "end");
        dispatch_key(&mut ed, "return");
        dispatch_key(&mut ed, "x");
        assert_eq!(helpers::buf_text(&ed), "hello\nx");

        dispatch_key(&mut ed, "backspace");
        assert_eq!(helpers::buf_text(&ed), "hello\n");

        // Select the last two chars ("o\n") …
        dispatch_key(&mut ed, "shift-left");
        dispatch_key(&mut ed, "shift-left");
        dispatch_key(&mut ed, "ctrl-x");
        assert_eq!(helpers::buf_text(&ed), "hell", "cut removes the selection");
        // … and paste them back.
        dispatch_key(&mut ed, "ctrl-v");
        assert_eq!(helpers::buf_text(&ed), "hello\n", "paste restores the cut text");
        assert!(ed.selection.is_none(), "paste clears the selection");
    });
}

#[test]
fn escape_clears_the_selection() {
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("probe.txt", "hello");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }

        dispatch_key(&mut ed, "shift-right");
        assert!(ed.selection.is_some(), "shift+movement selects");
        dispatch_key(&mut ed, "esc");
        assert!(ed.selection.is_none(), "escape clears the selection");
    });
}

#[test]
fn ctrl_a_selects_all_and_copy_yanks_it() {
    if !helpers::clipboard_available() {
        return;
    }
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("probe.txt", "hello\nworld");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }

        let _scope = EditorScope::new(&mut ed);
        dispatch_key(&mut ed, "ctrl-a");
        assert!(ed.selection.is_some(), "ctrl-a selects everything");
        dispatch_key(&mut ed, "ctrl-c");
        assert_eq!(
            ed.clipboard.get_text().as_deref(),
            Some("hello\nworld"),
            "copy with a selection yanks the selection"
        );
    });
}
