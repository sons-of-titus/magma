//! Welcome-window actions — every greeter click lands in one
//! `ShellRuntime::handle_welcome_action` call; these tests pin what each
//! action does to observable editor state.

#![cfg(feature = "gui")]

use crate::kernel::render::gui_shell::ShellRuntime;
use crate::kernel::render::gui_theme::GuiTheme;
use crate::kernel::render::gui_welcome::WelcomeAction;
use crate::tests::helpers::make_editor;

fn shell_and_editor() -> (ShellRuntime, crate::kernel::state::Editor) {
    let mut editor = make_editor();
    let shell = ShellRuntime::new(&mut editor);
    (shell, editor)
}

#[test]
fn welcome_open_project_action_opens_through_the_registry() {
    let dir = std::env::temp_dir().join(format!("magma-welcome-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (mut shell, mut ed) = shell_and_editor();

    shell.handle_welcome_action(&mut ed,
        WelcomeAction::OpenProject(dir.to_string_lossy().into_owned()));

    assert_eq!(ed.project_manager.project.root.as_deref(), Some(dir.as_path()),
        "OpenProject must go through the open-project command");
    assert_eq!(ed.project_manager.recent_projects.first().map(String::as_str),
        Some(dir.to_str().unwrap()), "an opened project becomes the MRU recent");
}

#[test]
fn welcome_remove_recent_action_forgets_a_project() {
    let (mut shell, mut ed) = shell_and_editor();
    ed.project_manager.recent_projects = vec!["/a/keep".to_string(), "/b/forget".to_string()];

    shell.handle_welcome_action(&mut ed, WelcomeAction::RemoveRecent("/b/forget".to_string()));

    assert_eq!(ed.project_manager.recent_projects, vec!["/a/keep".to_string()],
        "the forgotten project must leave the list, the rest keep their order");
}

#[test]
fn welcome_set_theme_action_switches_the_palette() {
    let (mut shell, mut ed) = shell_and_editor();
    assert!(!GuiTheme::option_is_light(&ed.options), "intellij-dark is the default preset");

    shell.handle_welcome_action(&mut ed, WelcomeAction::SetTheme("latte"));

    assert_eq!(ed.options.get("gui-theme").map(String::as_str), Some("latte"));
    assert!(GuiTheme::from_options(&ed.options).light,
        "the option drives the palette on the next sync_theme pass");
}

#[test]
fn welcome_font_size_action_steps_and_clamps() {
    let (mut shell, mut ed) = shell_and_editor();
    ed.font_config.size = 13.0;
    ed.font_changed = false;

    shell.handle_welcome_action(&mut ed, WelcomeAction::FontSize(2.0));
    assert_eq!(ed.font_config.size, 15.0);
    assert!(ed.font_changed, "a size change must request a font rebuild");

    shell.handle_welcome_action(&mut ed, WelcomeAction::FontSize(-1000.0));
    assert_eq!(ed.font_config.size, 8.0, "the size must clamp, not run away");
}
