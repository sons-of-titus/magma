//! GUI overhaul — Janet-side integration.
//!
//! The new GUI options (`gui-theme`, `minimap-enabled`,
//! `breadcrumb-enabled`) are plain editor options, so the Janet option API
//! must read and write them exactly like the settings dialog does.


use scripting;
use crate::tests::helpers::{acquire_janet_lock, make_editor};

#[test]
fn janet_option_set_writes_gui_options() {
    let _lock = acquire_janet_lock();
    let mut ed = make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let result = scripting::eval(r#"(option/set "minimap-enabled" "true")"#);
    assert_eq!(result, "ok");
    assert_eq!(ed.options.get("minimap-enabled").map(|s| s.as_str()), Some("true"));
}

#[test]
fn janet_option_theme_toggle_writes_options() {
    let _lock = acquire_janet_lock();
    let mut ed = make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    assert_eq!(scripting::eval(r#"(option/set "gui-theme" "latte")"#), "ok");
    assert_eq!(ed.options.get("gui-theme").map(|s| s.as_str()), Some("latte"));
    assert_eq!(scripting::eval(r#"(option/set "gui-theme" "mocha")"#), "ok");
    assert_eq!(ed.options.get("gui-theme").map(|s| s.as_str()), Some("mocha"));
}
