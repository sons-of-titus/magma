use crate::kernel::state::{mode::{EditorMode, Minibuffer}, Editor};
use crate::kernel::render::{frame::render_frame, surface::Surface};
use crate::tests::helpers;

fn enter_command(ed: &mut Editor, input: &str) {
    ed.editor_mode = EditorMode::new("command", false);
    ed.editor_mode.minibuffer = Some(Minibuffer { prompt: ":".into(), input: input.into() });
}

// ── Rust-side state for completions ───────────────────────────────────────

#[test]
fn completion_visible_starts_false() {
    let ed = helpers::make_editor();
    assert!(!ed.completion.visible);
    assert!(ed.completion.items.is_empty());
}

#[test]
fn completion_items_can_be_set() {
    let mut ed = helpers::make_editor();
    ed.completion.items = vec!["write".to_string(), "wq".to_string()];
    ed.completion.idx = 0;
    ed.completion.visible = true;
    assert_eq!(ed.completion.items.len(), 2);
    assert!(ed.completion.visible);
}

#[test]
fn completion_dismissed_by_backspace_command() {
    let mut ed = helpers::make_editor();
    enter_command(&mut ed, "w");
    ed.completion.items = vec!["w".to_string(), "write".to_string()];
    ed.completion.idx = 0;
    ed.completion.visible = true;

    let args = std::collections::HashMap::new();
    let _ = crate::kernel::command::execute_command(&mut ed, "command-backspace", &args);

    assert!(!ed.completion.visible, "backspace must dismiss completion popup");
    assert!(ed.completion.items.is_empty());
}

#[test]
fn completion_dismissed_by_text_input() {
    let mut ed = helpers::make_editor();
    enter_command(&mut ed, "w");
    ed.completion.items = vec!["w".to_string(), "write".to_string()];
    ed.completion.visible = true;

    crate::kernel::input::handle_text_input(&mut ed, "r");

    assert!(!ed.completion.visible, "typing a char must dismiss completion popup");
    assert!(ed.completion.items.is_empty());
    if let Some(ref mb) = ed.editor_mode.minibuffer {
        assert_eq!(mb.input, "wr");
    } else {
        panic!("expected Command mode");
    }
}

// ── Popup rendering in command mode ──────────────────────────────────────

#[test]
fn command_popup_renders_items_above_status_bar() {
    let mut ed = helpers::make_editor();
    enter_command(&mut ed, "w");
    ed.completion.items = vec!["w".to_string(), "write".to_string(), "wq".to_string()];
    ed.completion.idx = 1;
    ed.completion.visible = true;

    let mut surface = Surface::new(80, 10);
    render_frame(&ed, &mut surface);

    let popup_y = 10u16 - 1 - 3;
    let cell = surface.cell(0, popup_y).unwrap();
    let (bg_r, bg_g, bg_b) = ed.theme_color("bg");
    assert!(
        cell.style.bg != (bg_r, bg_g, bg_b) || cell.ch != ' ',
        "popup row {popup_y} must be painted by the popup renderer"
    );
}

#[test]
fn command_popup_not_rendered_outside_command_mode() {
    let mut ed = helpers::make_editor();
    ed.completion.items = vec!["write".to_string()];
    ed.completion.visible = true;

    let mut surface = Surface::new(80, 10);
    render_frame(&ed, &mut surface);

    let cell = surface.cell(0, 8).unwrap();
    let sel_bg = ed.theme_color("selection-bg");
    assert_ne!(cell.style.bg, sel_bg,
        "completion popup must not render outside command mode");
}
