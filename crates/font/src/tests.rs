use super::*;

#[test]
fn the_default_config_is_plain_monospace_15() {
    let fc = FontConfig::default();
    assert_eq!(fc.family, "Monospace");
    assert_eq!(fc.size, 15.0);
    assert_eq!(fc.weight, 400);
    assert!(!fc.ligatures);
    assert!(fc.loaded_fonts.is_empty());
    assert!(fc.fallback.is_empty());
    assert!(fc.glyph_widths.is_empty());
    assert!(fc.context_overrides.is_empty());
}

#[test]
fn the_primary_font_is_the_loaded_face_named_by_the_family() {
    let mut fc = FontConfig::default();
    assert!(fc.primary_font().is_none());
    fc.loaded_fonts.insert("custom".into(), vec![1, 2, 3]);
    assert!(fc.primary_font().is_none(), "family still says Monospace");
    fc.family = "custom".into();
    assert_eq!(fc.primary_font(), Some(&vec![1, 2, 3]));
}

#[test]
fn bundled_bootstrap_registers_the_regular_and_bold_faces() {
    let mut fc = FontConfig::default();
    fc.bootstrap_bundled();
    assert_eq!(fc.family, BUNDLED_ALIAS);
    assert!(
        fc.loaded_fonts
            .get(BUNDLED_ALIAS)
            .is_some_and(|b| !b.is_empty())
    );
    assert!(
        fc.loaded_fonts
            .get(&format!("{BUNDLED_ALIAS}-bold"))
            .is_some_and(|b| !b.is_empty())
    );
}

#[test]
fn an_already_loaded_font_is_never_replaced() {
    let mut fc = FontConfig::default();
    fc.loaded_fonts.insert("from-config".into(), vec![9]);
    fc.family = "from-config".into();

    fc.bootstrap_bundled();
    fc.bootstrap_default_font();

    assert_eq!(fc.family, "from-config", "an explicit choice wins");
    assert_eq!(fc.loaded_fonts.len(), 1);
}

#[test]
fn candidates_prefer_home_nerd_fonts_then_system_monospace() {
    let paths: Vec<String> = default_font_candidates()
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert!(
        paths
            .iter()
            .any(|p| p.ends_with("MonacoNerdFontMono-Regular.ttf"))
    );
    assert!(
        paths
            .iter()
            .any(|p| p == "/System/Library/Fonts/Monaco.ttf")
    );
    assert!(paths.iter().any(|p| p.contains("DejaVuSansMono.ttf")));
    assert!(
        paths.iter().any(|p| p.ends_with("consola.ttf")),
        "{paths:?}"
    );

    if let Some(home) = std::env::var_os("HOME") {
        let first = std::path::PathBuf::from(home).join("Library/Fonts");
        assert!(
            paths[0].starts_with(first.to_string_lossy().as_ref()),
            "a user font outranks the system copies"
        );
    }
}
