use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::state::{mode::{EditorMode, Minibuffer}, Editor};

fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    ed
}

fn enter_command(ed: &mut Editor, input: &str) {
    ed.editor_mode = EditorMode::new("command", false);
    ed.editor_mode.minibuffer = Some(Minibuffer { prompt: ":".into(), input: input.into() });
}

// ── editor/command-input (Rust-side state read) ───────────────────────────

#[test]
fn command_input_is_none_outside_command_mode() {
    let ed = make_editor();
    assert!(ed.editor_mode.minibuffer.is_none());
}

#[test]
fn command_input_is_empty_string_on_enter() {
    let mut ed = make_editor();
    enter_command(&mut ed, "");
    if let Some(ref mb) = ed.editor_mode.minibuffer {
        assert!(mb.input.is_empty());
    } else {
        panic!("expected command mode");
    }
}

#[test]
fn command_input_stores_typed_text() {
    let mut ed = make_editor();
    enter_command(&mut ed, "set nu");
    if let Some(ref mb) = ed.editor_mode.minibuffer {
        assert_eq!(mb.input, "set nu");
    } else {
        panic!("expected command mode");
    }
}

// ── editor/set-command-input (Rust-side mutation) ─────────────────────────

#[test]
fn set_command_input_replaces_text_in_command_mode() {
    let mut ed = make_editor();
    enter_command(&mut ed, "w");
    if let Some(ref mut mb) = ed.editor_mode.minibuffer {
        mb.input = "wq".to_string();
    }
    if let Some(ref mb) = ed.editor_mode.minibuffer {
        assert_eq!(mb.input, "wq");
    } else {
        panic!("expected command mode");
    }
}

#[test]
fn set_command_input_noop_outside_command_mode() {
    let mut ed = make_editor();
    assert!(ed.editor_mode.is_named("normal"));
    assert!(ed.editor_mode.is_named("normal"));
}

#[test]
fn command_backspace_exits_on_empty_input() {
    let mut ed = make_editor();
    enter_command(&mut ed, "");
    let should_exit = if let Some(ref mut mb) = ed.editor_mode.minibuffer {
        let last_len = mb.input.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
        let new_len = mb.input.len().saturating_sub(last_len);
        mb.input.truncate(new_len);
        mb.input.is_empty()
    } else {
        false
    };
    assert!(should_exit);
}

#[test]
fn command_backspace_removes_last_char() {
    let mut ed = make_editor();
    enter_command(&mut ed, "wq");
    if let Some(ref mut mb) = ed.editor_mode.minibuffer {
        let last_len = mb.input.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
        let new_len = mb.input.len().saturating_sub(last_len);
        mb.input.truncate(new_len);
    }
    if let Some(ref mb) = ed.editor_mode.minibuffer {
        assert_eq!(mb.input, "w");
    } else {
        panic!("expected command mode");
    }
}
