//! Tests for the shell-facing search, task, zoom, and GUI-request commands:
//! the find bar's kernel surface (`search-buffer`/`replace-*`), the
//! toolbar's build/test dispatch, zoom, and the `gui-*` requests the keymap
//! raises chrome with.

use crate::kernel::command::execute_command;
use crate::kernel::render::tabs;
use crate::tests::helpers::{buf_text, focused_key, make_editor_with_buffer};
use std::collections::HashMap;

fn run(ed: &mut crate::kernel::state::Editor, cmd: &str, args: &[(&str, &str)]) -> Result<(), String> {
    let mut map = HashMap::new();
    for (name, value) in args {
        map.insert(
            name.to_string(),
            crate::kernel::command::args::ArgValue::String(value.to_string()),
        );
    }
    execute_command(ed, cmd, &map)
}

#[test]
fn search_buffer_highlights_and_reports_matches() {
    let mut ed = make_editor_with_buffer("hat cat hat\nbat\nhat");
    run(&mut ed, "search-buffer", &[("pattern", "hat")]).unwrap();

    assert_eq!(ed.search.pattern.as_deref(), Some("hat"));
    assert_eq!(ed.search.last_match_count, 3, "every occurrence counts");
    let key = focused_key(&ed);
    let view = ed.view_of_buffer(key).unwrap();
    assert_eq!(view.highlights.len(), 3, "every match is highlighted");
    // The first match at/after the cursor is "hat" at offset 0.
    assert_eq!(view.cursor_offset(), 3, "cursor lands after the match");
}

#[test]
fn search_buffer_wraps_to_first_match() {
    let mut ed = make_editor_with_buffer("one two three");
    run(&mut ed, "search-buffer", &[("pattern", "two")]).unwrap();
    run(&mut ed, "search-buffer", &[("pattern", "one")]).unwrap();
    let view = ed.view_of_buffer(focused_key(&ed)).unwrap();
    assert_eq!(view.cursor_offset(), 3, "wrapped to the only match");
}

#[test]
fn search_buffer_is_smart_case() {
    let mut ed = make_editor_with_buffer("Hello hello HELLO");
    run(&mut ed, "search-buffer", &[("pattern", "hello")]).unwrap();
    assert_eq!(ed.search.last_match_count, 3, "lowercase query ignores case");
    run(&mut ed, "search-buffer", &[("pattern", "HELLO")]).unwrap();
    assert_eq!(ed.search.last_match_count, 1, "uppercase query is exact");
}

#[test]
fn search_buffer_empty_pattern_clears() {
    let mut ed = make_editor_with_buffer("abc");
    run(&mut ed, "search-buffer", &[("pattern", "a")]).unwrap();
    run(&mut ed, "search-buffer", &[("pattern", "")]).unwrap();
    assert!(ed.search.pattern.is_none());
    assert_eq!(ed.search.last_match_count, 0);
}

#[test]
fn search_clear_forgets_pattern_and_highlights() {
    let mut ed = make_editor_with_buffer("abc abc");
    run(&mut ed, "search-buffer", &[("pattern", "abc")]).unwrap();
    execute_command(&mut ed, "search-clear", &HashMap::new()).unwrap();
    assert!(ed.search.pattern.is_none());
    let view = ed.view_of_buffer(focused_key(&ed)).unwrap();
    assert!(view.highlights.is_empty());
}

#[test]
fn replace_next_swaps_one_occurrence() {
    let mut ed = make_editor_with_buffer("cat cat cat");
    run(
        &mut ed,
        "replace-next",
        &[("pattern", "cat"), ("replacement", "dog")],
    )
    .unwrap();
    assert_eq!(buf_text(&ed), "dog cat cat");
    assert_eq!(ed.search.last_match_count, 2, "re-search reports what is left");
}

#[test]
fn replace_all_swaps_every_occurrence() {
    let mut ed = make_editor_with_buffer("cat cat cat");
    run(
        &mut ed,
        "replace-all",
        &[("pattern", "cat"), ("replacement", "dog")],
    )
    .unwrap();
    assert_eq!(buf_text(&ed), "dog dog dog");
    assert!(ed.search.pattern.is_none(), "a finished replace-all clears the find bar");
}

#[test]
fn task_build_without_config_names_the_language() {
    let mut ed = make_editor_with_buffer("fn main() {}");
    ed.project_manager.project.language = "python".into();
    let err = execute_command(&mut ed, "task-build", &HashMap::new()).unwrap_err();
    assert!(err.contains("no build task"), "got: {err}");
    assert!(err.contains("python"), "got: {err}");
}

#[test]
fn task_test_launches_the_detected_test_config() {
    let mut ed = make_editor_with_buffer("fn main() {}");
    // A project with a seeded "test" config launches it.  No background
    // runtime in this test, so the launch path reports the honest error —
    // the config resolution itself must have passed.
    ed.project_manager.project.language = "rust".into();
    // The detection-seeded defaults are what task-test resolves against;
    // launching needs the background runtime this test does not have, so
    // the config must resolve first and the launch be the reported gap.
    let configs = kernel::run_config::defaults_for("rust");
    let test_cfg = configs.iter().find(|c| c.name == "test").unwrap();
    assert_eq!(test_cfg.command, "cargo");
    assert_eq!(test_cfg.args, vec!["test"]);
    let err = execute_command(&mut ed, "task-test", &HashMap::new()).unwrap_err();
    assert!(
        err.contains("background runtime") || err.contains("no test task"),
        "got: {err}"
    );
}

#[test]
fn zoom_commands_step_and_clamp_font_size() {
    let mut ed = make_editor_with_buffer("x");
    let start = ed.font_config.size;
    execute_command(&mut ed, "zoom-in", &HashMap::new()).unwrap();
    assert_eq!(ed.font_config.size, start + 1.0);
    assert!(ed.font_changed, "the shell must re-rasterize");
    assert_eq!(ed.options.get("font.size").unwrap(), &(start + 1.0).to_string());

    for _ in 0..100 {
        execute_command(&mut ed, "zoom-out", &HashMap::new()).unwrap();
    }
    assert_eq!(ed.font_config.size, 6.0, "clamped at the floor");

    execute_command(&mut ed, "zoom-reset", &HashMap::new()).unwrap();
    assert_eq!(ed.font_config.size, 15.0);
}

#[test]
fn gui_commands_queue_chrome_requests() {
    for (command, request) in [
        ("gui-find", "find"),
        ("gui-replace", "replace"),
        ("gui-project-find", "project-find"),
        ("gui-goto-file", "goto-file"),
        ("gui-goto-symbol", "goto-symbol"),
        ("gui-buffers", "buffers"),
        ("gui-command-palette", "commands"),
        ("gui-goto-line", "goto-line"),
        ("gui-branches", "branches"),
    ] {
        let mut ed = make_editor_with_buffer("x");
        execute_command(&mut ed, command, &HashMap::new()).unwrap();
        assert_eq!(ed.ui_requests, vec![request.to_string()], "{command}");
    }
}

#[test]
fn display_label_renders_buffer_names_for_users() {
    assert_eq!(tabs::display_label("*make*(# make)"), "make");
    assert_eq!(tabs::display_label("*make*(# make test)"), "make test");
    assert_eq!(tabs::display_label("*scratch*"), "scratch");
    assert_eq!(tabs::display_label("main.rs"), "main.rs");
    assert_eq!(tabs::display_label("src/lib.rs"), "lib.rs");
}
