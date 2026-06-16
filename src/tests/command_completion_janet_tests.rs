use crate::kernel::state::mode::{EditorMode, Minibuffer};
use crate::kernel::state::Editor;
use crate::kernel::scripting;
use crate::kernel::input::dispatch_key;

fn enter_command(ed: &mut Editor, input: &str) {
    ed.editor_mode = EditorMode::new("command", false);
    ed.editor_mode.minibuffer = Some(Minibuffer { prompt: ":".into(), input: input.into() });
    ed.keymaps.pop_layer("vim");
    ed.keymaps.push_layer("command");
}

// Helper: enter command mode with a given prefix and dispatch "tab".
fn complete_once(ed: &mut Editor, prefix: &str) {
    enter_command(ed, prefix);
    dispatch_key(ed, "tab");
}

// ── editor/command-input ──────────────────────────────────────────────────

#[test]
fn command_input_returns_nil_outside_command_mode() {
    janet_test!(ed, {
        let r = scripting::eval("(nil? (editor/command-input))");
        assert_eq!(r, "ok");
    });
}

#[test]
fn command_input_returns_current_input_string() {
    janet_test!(ed, {
        enter_command(&mut ed, "set nu");

        let r = scripting::eval(r#"(= (editor/command-input) "set nu")"#);
        assert_eq!(r, "ok");
    });
}

#[test]
fn command_input_returns_empty_string_when_no_input() {
    janet_test!(ed, {
        enter_command(&mut ed, "");

        let r = scripting::eval(r#"(= (editor/command-input) "")"#);
        assert_eq!(r, "ok");
    });
}

// ── editor/set-command-input ──────────────────────────────────────────────

#[test]
fn set_command_input_replaces_input() {
    janet_test!(ed, {
        enter_command(&mut ed, "w");

        let r = scripting::eval(r#"(editor/set-command-input "wq")"#);
        assert_eq!(r, "ok");

        if let Some(ref mb) = ed.editor_mode.minibuffer {
            assert_eq!(mb.input, "wq");
        } else {
            panic!("expected command mode");
        }
    });
}

#[test]
fn set_command_input_is_noop_outside_command_mode() {
    janet_test!(ed, {
        let r = scripting::eval(r#"(editor/set-command-input "anything")"#);
        assert_eq!(r, "ok");
        assert!(ed.editor_mode.is_named("normal"), "mode must remain Normal");
    });
}

// ── command-complete via dispatch_key (avoids nested-fiber constraint) ────

#[test]
fn tab_in_command_mode_completes_first_match() {
    janet_test!(ed, {
        complete_once(&mut ed, "w");

        if let Some(ref mb) = ed.editor_mode.minibuffer {
            assert!(
                mb.input.starts_with('w'),
                "completed verb should start with 'w', got '{}'", mb.input
            );
        } else {
            panic!("mode should still be Command after tab");
        }
    });
}

#[test]
fn tab_cycles_to_next_match_on_repeat() {
    janet_test!(ed, {
        complete_once(&mut ed, "w");
        let first = if let Some(ref mb) = ed.editor_mode.minibuffer {
            mb.input.clone()
        } else {
            panic!("expected Command mode");
        };

        dispatch_key(&mut ed, "tab");
        let second = if let Some(ref mb) = ed.editor_mode.minibuffer {
            mb.input.clone()
        } else {
            panic!("expected Command mode");
        };

        assert!(first.starts_with('w'), "first completion starts with 'w'");
        assert!(second.starts_with('w'), "second completion starts with 'w'");
    });
}

#[test]
fn tab_noop_when_no_match() {
    janet_test!(ed, {
        complete_once(&mut ed, "zzznomatch");

        if let Some(ref mb) = ed.editor_mode.minibuffer {
            assert_eq!(mb.input, "zzznomatch", "no match → input unchanged");
        } else {
            panic!("expected Command mode");
        }
    });
}

#[test]
fn tab_skips_completion_when_input_has_space() {
    janet_test!(ed, {
        complete_once(&mut ed, "set nu");

        if let Some(ref mb) = ed.editor_mode.minibuffer {
            assert_eq!(mb.input, "set nu", "input with space must not be completed");
        } else {
            panic!("expected Command mode");
        }
    });
}

#[test]
fn tab_resets_on_new_prefix() {
    janet_test!(ed, {
        complete_once(&mut ed, "w");

        if let Some(ref mut mb) = ed.editor_mode.minibuffer { mb.input = "q".to_string(); }
        dispatch_key(&mut ed, "tab");

        if let Some(ref mb) = ed.editor_mode.minibuffer {
            assert!(
                mb.input.starts_with('q'),
                "after prefix change, completion should match 'q', got '{}'", mb.input
            );
        } else {
            panic!("expected Command mode");
        }
    });
}

#[test]
fn tab_completes_empty_prefix_to_first_verb() {
    janet_test!(ed, {
        complete_once(&mut ed, "");

        if let Some(ref mb) = ed.editor_mode.minibuffer {
            assert!(!mb.input.is_empty(), "empty prefix should complete to some verb");
        } else {
            panic!("expected Command mode");
        }
    });
}
