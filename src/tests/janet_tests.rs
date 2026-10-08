//! Janet runtime integration — command surface, options, and the mode
//! substrate a community modal-editing plugin would build on.

use crate::kernel::scripting;

#[test]
fn quit_routes_through_the_guarded_command() {
    janet_test!(ed, {
        let r = scripting::eval("(editor/run-command \"quit\")");
        assert_eq!(r, "ok");
        assert!(!ed.running, "quit must end the session");
    });
}

#[test]
fn quit_refusal_surfaces_as_a_janet_error() {
    janet_test!(ed, {
        let key = ed.create_buffer_from_str("dirty.txt", "work");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        let view = ed.views.get_mut(&key).unwrap();
        view.insert(0, "x");

        let r = scripting::eval("(editor/run-command \"quit\")");
        assert_ne!(r, "ok", "quit with dirty buffers must fail");
        assert!(ed.running, "refusal keeps the editor alive");
    });
}

#[test]
fn set_mode_substrate_remains_for_community_plugins() {
    // Modality is out of the shipped product, but the extension point a
    // community vim plugin needs must keep working.
    janet_test!(ed, {
        let r = scripting::eval("(editor/set-mode \"vim-normal\" {:accepts-text false})");
        assert_eq!(r, "ok");
        assert_eq!(ed.editor_mode.name, "vim-normal");
        assert!(!ed.editor_mode.accepts_text);

        let r = scripting::eval("(editor/set-mode \"editor\")");
        assert_eq!(r, "ok");
        assert!(ed.editor_mode.accepts_text, "back to the default typing mode");
    });
}

#[test]
fn option_set_is_the_configuration_path() {
    janet_test!(ed, {
        let r = scripting::eval("(option/set \"tab-width\" \"8\")");
        assert_eq!(r, "ok");
        assert_eq!(ed.options.get("tab-width").map(|s| s.as_str()), Some("8"));
    });
}

#[test]
fn janet_defined_commands_execute_but_stay_off_the_palette() {
    janet_test!(ed, {
        let r = scripting::eval(
            "(command/define \"community-hello\" (fn [&] (editor/echo \"hi\")))");
        assert_eq!(r, "ok");
        assert!(ed.commands.exists("community-hello"));
        assert!(ed.commands.list_rust_commands().iter()
            .all(|(name, _, _)| name != "community-hello"),
            "Janet-defined commands are not palette surface");
    });
}
