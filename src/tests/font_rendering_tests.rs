use crate::render::frame::cell_width;
use crate::state::{Editor, FontConfig};
use crate::tests::helpers;

#[test]
fn default_font_family_is_monospace() {
    let ed = helpers::make_editor();
    assert_eq!(ed.font_config.family, "Monospace");
}

#[test]
fn default_font_size_is_15() {
    let ed = helpers::make_editor();
    assert!((ed.font_config.size - 15.0).abs() < f32::EPSILON);
}

#[test]
fn default_ligatures_is_false() {
    let ed = helpers::make_editor();
    assert!(!ed.font_config.ligatures);
}

#[test]
fn default_fallback_is_empty() {
    let ed = helpers::make_editor();
    assert!(ed.font_config.fallback.is_empty());
}

#[test]
fn default_glyph_widths_is_empty() {
    let ed = helpers::make_editor();
    assert!(ed.font_config.glyph_widths.is_empty());
}

#[test]
fn font_config_size_can_be_set() {
    let mut ed = helpers::make_editor();
    ed.font_config.size = 20.0;
    assert!((ed.font_config.size - 20.0).abs() < f32::EPSILON);
}

#[test]
fn cell_width_ascii_returns_1() {
    let widths = std::collections::HashMap::new();
    assert_eq!(cell_width('a', &widths), 1);
    assert_eq!(cell_width('Z', &widths), 1);
    assert_eq!(cell_width('5', &widths), 1);
}

#[test]
fn cell_width_override_is_respected() {
    let mut widths = std::collections::HashMap::new();
    widths.insert('\u{E000}', 2u8);
    assert_eq!(cell_width('\u{E000}', &widths), 2);
}

#[test]
fn cell_width_non_override_char_uses_unicode_width() {
    let widths = std::collections::HashMap::new();
    // Full-width CJK character should be 2 via unicode_width
    let cjk = '中';
    let expected = unicode_width::UnicodeWidthChar::width(cjk).unwrap_or(1);
    assert_eq!(cell_width(cjk, &widths), expected);
}

#[test]
fn cell_width_override_wins_over_unicode_width() {
    let mut widths = std::collections::HashMap::new();
    // '中' is normally width 2; override to 1
    widths.insert('中', 1u8);
    assert_eq!(cell_width('中', &widths), 1);
}

#[test]
fn glyph_widths_range_population() {
    let mut fc = FontConfig::default();
    // Simulate what set-nerd-font does
    for cp in 0xE000u32..=0xE010u32 {
        if let Some(c) = char::from_u32(cp) {
            fc.glyph_widths.insert(c, 2);
        }
    }
    assert_eq!(fc.glyph_widths.get(&'\u{E000}'), Some(&2));
    assert_eq!(fc.glyph_widths.get(&'\u{E010}'), Some(&2));
    assert_eq!(fc.glyph_widths.get(&'\u{E011}'), None);
}

#[test]
fn context_overrides_stored_on_font_config() {
    let mut fc = FontConfig::default();
    fc.context_overrides.insert(
        "prose".to_string(),
        crate::state::ContextFont { family: Some("Georgia".to_string()), size: Some(18.0) },
    );
    let ctx = fc.context_overrides.get("prose").unwrap();
    assert_eq!(ctx.family.as_deref(), Some("Georgia"));
    assert_eq!(ctx.size, Some(18.0));
}

#[test]
fn font_changed_flag_starts_false() {
    let ed = helpers::make_editor();
    assert!(!ed.font_changed);
}
