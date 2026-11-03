use crate::kernel::render::frame::cell_width;
use crate::kernel::state::FontConfig;
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
        crate::kernel::state::ContextFont { family: Some("Georgia".to_string()), size: Some(18.0) },
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

// ── Bundled font bootstrap (GUI presentability) ─────────────────────────

#[test]
fn bundled_font_bootstrap_installs_jetbrains_mono() {
    let mut fc = crate::kernel::state::FontConfig::default();
    assert!(fc.loaded_fonts.is_empty(), "default config ships no fonts");

    fc.bootstrap_bundled();
    assert_eq!(fc.loaded_fonts.len(), 2, "regular + bold bundled");
    assert_eq!(fc.family, "magma-mono");
    let regular = fc.loaded_fonts.get("magma-mono").unwrap();
    assert!(regular.len() > 100_000, "real TTF bytes, got {}", regular.len());

    // Idempotent: an existing user font choice is never overwritten.
    fc.loaded_fonts.insert("user-font".to_string(), vec![1, 2, 3]);
    fc.family = "user-font".to_string();
    fc.bootstrap_bundled();
    assert_eq!(fc.family, "user-font");
    assert_eq!(fc.loaded_fonts.get("user-font").unwrap(), &vec![1, 2, 3]);
}

#[test]
fn default_bootstrap_installs_a_resolvable_primary_family() {
    let mut fc = FontConfig::default();
    fc.bootstrap_default_font();
    assert!(!fc.loaded_fonts.is_empty(), "a default font must be installed");
    // Either the system monospace (Monaco) or the bundled fallback wins,
    // but the family must always name a font we actually loaded.
    assert!(
        fc.family == "system-mono" || fc.family == "magma-mono",
        "unexpected default family {:?}", fc.family
    );
    assert!(fc.loaded_fonts.contains_key(&fc.family));
    assert!(fc.primary_font().is_some(), "primary font bytes must resolve");
}

#[test]
fn default_bootstrap_respects_an_existing_choice() {
    let mut fc = FontConfig::default();
    fc.loaded_fonts.insert("user-font".to_string(), vec![9u8; 2048]);
    fc.family = "user-font".to_string();
    fc.bootstrap_default_font();
    assert_eq!(fc.family, "user-font");
    assert_eq!(fc.primary_font().map(|b| b.len()), Some(2048));
}

// ── Nerd Font default (icons render in the buffer grid) ─────────────────

#[test]
fn nerd_font_default_covers_icon_glyphs_when_present() {
    use ab_glyph::{Font as _, FontRef, ScaleFont as _};

    let Some((path, bytes)) = crate::kernel::state::font::default_font_candidates()
        .into_iter()
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().contains("Nerd"))
                .unwrap_or(false)
        })
        .find_map(|p| std::fs::read(&p).ok().map(|b| (p, b)))
    else {
        eprintln!("no Nerd Font installed; skipping icon-coverage check");
        return;
    };

    let font = FontRef::try_from_slice(&bytes)
        .unwrap_or_else(|e| panic!("Nerd Font {path:?} must parse: {e}"));
    let scaled = font.as_scaled(ab_glyph::PxScale::from(15.0));
    // Powerline, devicons, Font-Awesome, and box-drawing must all be real
    // glyphs, not .notdef — that is what makes a Nerd Font worth defaulting to.
    for (name, cp) in [
        ("powerline", '\u{E0B0}'),
        ("devicons", '\u{E0A0}'),
        ("font-awesome", '\u{F015}'),
        ("box-drawing", '\u{2500}'),
    ] {
        assert_ne!(
            scaled.glyph_id(cp).0, 0,
            "Nerd Font {path:?} is missing {name} glyph U+{:04X}", cp as u32
        );
    }
}

#[test]
fn nerd_font_default_rasterizes_into_the_atlas_when_present() {
    let Some(bytes) = crate::kernel::state::font::default_font_candidates()
        .into_iter()
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().contains("Nerd"))
                .unwrap_or(false)
        })
        .find_map(|p| std::fs::read(&p).ok())
    else {
        eprintln!("no Nerd Font installed; skipping rasterization check");
        return;
    };
    let mut atlas = crate::kernel::render::gpu_atlas::GpuGlyphAtlas::new();
    atlas.rasterize(&bytes, 15.0);
    assert!(atlas.needs_rebuild());
    // A Nerd Font must still cover the ASCII the atlas is built from.
    for ch in ['A', 'z', '0', '~'] {
        assert!(atlas.contains(ch), "atlas must contain {ch:?}");
    }
    let px = atlas.generate_atlas_pixels();
    assert_eq!(px.len(), (atlas.atlas_width * atlas.atlas_height * 4) as usize);
}

#[test]
fn bundled_font_parses_and_rasterizes_with_extended_charset() {
    let bytes = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");
    let mut atlas = crate::kernel::render::gpu_atlas::GpuGlyphAtlas::new();
    atlas.rasterize(bytes, 15.0);
    assert!(atlas.needs_rebuild());
    // ASCII, Latin-1, box drawing, and arrows must all be real glyphs now.
    for ch in ['A', 'é', '│', '┌', '╔', '→'] {
        assert!(atlas.contains(ch), "atlas must contain {ch:?}");
    }
    assert!(atlas.atlas_width > 0 && atlas.atlas_height > 0);
    let px = atlas.generate_atlas_pixels();
    assert_eq!(
        px.len(),
        (atlas.atlas_width * atlas.atlas_height * 4) as usize
    );
    // The first cell must be solid white (rect sampling depends on it).
    let checksum: u64 = px.iter().map(|&b| b as u64).sum();
    eprintln!("CHECKSUM {}x{} = {}", atlas.atlas_width, atlas.atlas_height, checksum);
    let white_cell: u64 = (16 * 24 * 4 * 255) as u64;
    assert!(checksum > white_cell, "atlas must contain at least the white cell");
}






#[test]
fn rasterizer_draws_all_glyphs_not_one_cell() {
    // Regression: ab_glyph's OutlineGlyph::draw yields bounding-box-relative
    // u32 coordinates; treating them as absolute piled every glyph onto atlas
    // cell 0, leaving the atlas (and the whole GPU text path) blank.  The
    // fix composes bounds().min with the draw coordinates.
    let bytes = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");
    let chars: Vec<char> = crate::kernel::render::gpu_atlas::ATLAS_CHARS.chars().collect();
    let px = gpu::gpu_rasterizer::rasterize_font_atlas(bytes, 15.0, &chars, 16, 24, 16);

    // The reserved solid-white first cell is intact.
    assert_eq!(&px[0..4], &[255, 255, 255, 255]);

    // Glyph coverage spans far more than one cell (~60+ texels per glyph).
    let nonzero_texels = px.chunks(4).filter(|t| t[3] != 0).count();
    assert!(
        nonzero_texels > chars.len() * 20,
        "expected glyph coverage across the atlas, got {nonzero_texels} texels for {} chars",
        chars.len()
    );

    // A late-alphabet cell has content (proves glyphs land in their own cells).
    let z_pos = chars.iter().position(|&c| c == 'z').unwrap();
    let (cx, cy) = ((z_pos as u32) % 16, (z_pos as u32) / 16);
    let cell: Vec<u8> = (0..24u32)
        .flat_map(|y| (0..16u32).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            let i = (((cy * 24 + y) * 256 + (cx * 16 + x)) * 4) as usize;
            px[i + 3..i + 4].to_vec()
        })
        .collect();
    assert!(cell.iter().any(|&a| a != 0), "'z' cell must contain glyph coverage");
}
