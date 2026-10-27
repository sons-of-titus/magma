//! Frontend tests: shell behavior, the pane's input translation, and the
//! view-model logic that used to live behind the kernel's `gui` feature.

macro_rules! janet_test {
    ($ed:ident, $body:block) => {{
        let _lock = crate::tests::helpers::acquire_janet_lock();
        let mut $ed = crate::tests::helpers::make_editor();
        scripting::init(&mut $ed);
        let _editor_scope = scripting::EditorScope::new(&mut $ed);
        $body
    }};
}

mod helpers;

mod gui_chrome_tests;
mod gui_overhaul_janet_tests;
mod gui_overhaul_tests;
mod gui_repaint_tests;
mod gui_welcome_tests;
mod icon_pack_janet_tests;
mod icon_pack_tests;
mod ime_tests;
mod project_open_tests;
mod quickfix_tests;
mod render_integration_tests;
mod view_layout_tests;
