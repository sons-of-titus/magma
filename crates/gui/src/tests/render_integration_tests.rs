#[test]
fn gui_app_init_bootstraps_bundled_font_and_atlas() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor();
    scripting::init(&mut ed);

    // Construction must not panic and must leave the editor with real font
    // bytes + a changed-font flag so the first frame applies everything.
    let app = crate::render::gui::GuiApp::new(ed);
    let ed = &app.editor;
    assert!(
        !ed.font_config.loaded_fonts.is_empty(),
        "init installs a default font (system monospace or bundled)"
    );
    assert!(
        ed.font_changed,
        "first frame re-applies fonts to egui + atlas"
    );
    assert!(
        ed.font_config.family == "system-mono" || ed.font_config.family == "magma-mono",
        "unexpected default family {:?}",
        ed.font_config.family
    );
    assert!(
        ed.font_config.primary_font().is_some(),
        "the primary family must resolve to loaded bytes"
    );
}
