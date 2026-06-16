use crate::kernel::scripting;
use crate::tests::helpers;

// ── ViewTree through Janet API ─────────────────────────────────────────────

#[test]
fn view_tree_split_visible_via_window_list() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("test\n");
    scripting::init(&mut ed);

    let initial_count = ed.view_tree.len();
    let wid = ed.view_tree.focused_window().unwrap();
    ed.view_tree.split_horizontal(wid);
    assert_eq!(ed.view_tree.len(), initial_count + 1);

    let result = scripting::eval("(length (window/list))");
    assert_eq!(result, "ok");
}

#[test]
fn view_tree_focus_works_via_window_focus() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("test\n");
    scripting::init(&mut ed);

    let id = ed.view_tree.focused_window().unwrap();
    let new_id = ed.view_tree.split_vertical(id).unwrap();
    let before = ed.view_tree.focused_window();

    scripting::eval(&format!("(window/focus {})", new_id.0));
    assert_ne!(ed.view_tree.focused_window(), before);
    assert_eq!(ed.view_tree.focused_window(), Some(new_id));
}

#[test]
fn view_tree_layout_save_and_restore() {
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor_with_buffer("test\n");
    scripting::init(&mut ed);

    scripting::eval("(editor/save-layout \"phase2-test\")");
    assert!(ed.saved_layouts.contains_key("phase2-test"));

    let id = ed.view_tree.focused_window().unwrap();
    ed.view_tree.split_horizontal(id);
    assert_eq!(ed.view_tree.len(), 2);

    scripting::eval("(editor/restore-layout \"phase2-test\")");
    assert_eq!(ed.view_tree.len(), 1);
}
