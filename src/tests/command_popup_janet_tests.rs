use crate::kernel::state::mode::{EditorMode, Minibuffer};
use crate::kernel::state::Editor;
use crate::kernel::scripting;
use crate::kernel::input::dispatch_key;

fn enter_command_mode_with(ed: &mut Editor, prefix: &str) {
    ed.editor_mode = EditorMode::new("command", false);
    ed.editor_mode.minibuffer = Some(Minibuffer { prompt: ":".into(), input: prefix.into() });
    ed.keymaps.pop_layer("vim");
    ed.keymaps.push_layer("command");
}

// ── editor/set-completions ────────────────────────────────────────────────

#[test]
fn set_completions_populates_items_and_shows_popup() {
    janet_test!(ed, {
        let r = scripting::eval(
            r#"(editor/set-completions @["write" "wq" "w"] 1)"#,
        );
        assert_eq!(r, "ok");

        assert!(ed.completion.visible);
        assert_eq!(ed.completion.items, vec!["write", "wq", "w"]);
        assert_eq!(ed.completion.idx, 1);
    });
}

#[test]
fn set_completions_defaults_idx_to_zero() {
    janet_test!(ed, {
        let r = scripting::eval(
            r#"(editor/set-completions @["alpha" "beta"])"#,
        );
        assert_eq!(r, "ok");
        assert_eq!(ed.completion.idx, 0);
    });
}

#[test]
fn set_completions_with_empty_array_hides_popup() {
    janet_test!(ed, {
        scripting::eval(r#"(editor/set-completions @["x"])"#);
        assert!(ed.completion.visible);

        let r = scripting::eval(r#"(editor/set-completions @[])"#);
        assert_eq!(r, "ok");
        assert!(!ed.completion.visible);
    });
}

// ── editor/clear-completions ──────────────────────────────────────────────

#[test]
fn clear_completions_hides_popup() {
    janet_test!(ed, {
        scripting::eval(r#"(editor/set-completions @["w" "write"])"#);
        assert!(ed.completion.visible);

        let r = scripting::eval("(editor/clear-completions)");
        assert_eq!(r, "ok");
        assert!(!ed.completion.visible);
        assert!(ed.completion.items.is_empty());
    });
}

// ── tab shows popup via dispatch_key ─────────────────────────────────────

#[test]
fn tab_in_command_mode_shows_popup() {
    janet_test!(ed, {
        enter_command_mode_with(&mut ed, "w");
        dispatch_key(&mut ed, "tab");

        assert!(ed.completion.visible, "tab should show completion popup");
        assert!(!ed.completion.items.is_empty(), "popup must have candidates");
        for item in &ed.completion.items {
            assert!(
                item.starts_with('w'),
                "all candidates should start with 'w', got '{item}'"
            );
        }
    });
}

#[test]
fn tab_selects_first_match_and_shows_in_popup() {
    janet_test!(ed, {
        enter_command_mode_with(&mut ed, "w");
        dispatch_key(&mut ed, "tab");

        let selected_idx = ed.completion.idx;
        let selected_item = ed.completion.items.get(selected_idx).cloned().unwrap_or_default();
        let input = if let Some(ref mb) = ed.editor_mode.minibuffer {
            mb.input.clone()
        } else {
            panic!("expected Command mode");
        };
        assert_eq!(input, selected_item,
            "command input must equal the selected completion item");
    });
}

#[test]
fn popup_dismissed_when_typing_a_char() {
    janet_test!(ed, {
        enter_command_mode_with(&mut ed, "w");
        dispatch_key(&mut ed, "tab");
        assert!(ed.completion.visible);

        dispatch_key(&mut ed, "r");
        assert!(!ed.completion.visible, "typing should dismiss the popup");
    });
}

#[test]
fn ctrl_p_reverses_completion_cycle() {
    janet_test!(ed, {
        enter_command_mode_with(&mut ed, "w");
        dispatch_key(&mut ed, "tab");
        dispatch_key(&mut ed, "tab");
        let after_two_tabs = ed.completion.idx;

        dispatch_key(&mut ed, "ctrl-p");
        let after_back = ed.completion.idx;

        let n = ed.completion.items.len();
        let expected = if after_two_tabs == 0 { n - 1 } else { after_two_tabs - 1 };
        assert_eq!(after_back, expected,
            "ctrl-p must move selection one step backward");
    });
}

#[test]
fn popup_dismissed_when_backspace_pressed() {
    janet_test!(ed, {
        enter_command_mode_with(&mut ed, "write");
        ed.completion.items = vec!["write".to_string()];
        ed.completion.idx = 0;
        ed.completion.visible = true;

        let args = std::collections::HashMap::new();
        let _ = crate::kernel::command::execute_command(&mut ed, "command-backspace", &args);

        assert!(!ed.completion.visible, "command-backspace must dismiss the popup");
        assert!(ed.completion.items.is_empty());
        if let Some(ref mb) = ed.editor_mode.minibuffer {
            assert_eq!(mb.input, "writ");
        } else {
            panic!("expected Command mode");
        }
    });
}
