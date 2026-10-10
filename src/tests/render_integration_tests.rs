#[test]
fn gui_app_init_bootstraps_bundled_font_and_atlas() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();
    crate::kernel::scripting::init(&mut ed);

    // Construction must not panic and must leave the editor with real font
    // bytes + a changed-font flag so the first frame applies everything.
    let app = crate::kernel::render::gui::GuiApp::new(ed);
    let ed = &app.editor;
    assert!(
        !ed.font_config.loaded_fonts.is_empty(),
        "init installs the bundled font"
    );
    assert!(ed.font_changed, "first frame re-applies fonts to egui + atlas");
    assert_eq!(ed.font_config.family, "magma-mono");
}
