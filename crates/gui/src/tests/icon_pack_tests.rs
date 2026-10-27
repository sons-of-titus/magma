//! Icon pack state — lookup order, VS Code manifest loading, and the
//! `icon-pack` option follower.  No Janet VM involved.


use kernel::icons::{self, IconKind, IconPack};
use crate::tests::helpers::make_editor;

pub(crate) fn glyph_pack() -> IconPack {
    let mut pack = IconPack::empty("test");
    pack.extensions.insert("ts".into(), IconKind::glyph('A'));
    pack.extensions.insert("spec.ts".into(), IconKind::glyph('B'));
    pack.extensions.insert("md".into(), IconKind::glyph('C'));
    pack.file_names.insert("Cargo.toml".into(), IconKind::glyph('D'));
    pack.default_file = Some(IconKind::glyph('?'));
    pack.folder = Some(IconKind::glyph('F'));
    pack.folder_open = Some(IconKind::glyph('O'));
    pack
}

fn glyph_of(pack: &IconPack, name: &str) -> char {
    match pack.lookup(name) {
        Some(IconKind::Glyph { glyph, .. }) => *glyph,
        other => panic!("expected glyph for {name}, got {other:?}"),
    }
}

#[test]
fn exact_file_name_beats_extension() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(&pack, "Cargo.toml"), 'D');
}

#[test]
fn longest_extension_suffix_wins() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(&pack, "app.spec.ts"), 'B');
    assert_eq!(glyph_of(&pack, "main.ts"), 'A');
}

#[test]
fn unknown_files_fall_back_to_default() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(&pack, "data.bin"), '?');
}

#[test]
fn extension_lookup_is_case_insensitive() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(&pack, "README.MD"), 'C');
}

#[test]
fn open_folder_falls_back_to_closed_folder_icon() {
    let pack = glyph_pack();
    let glyph = |k: &IconKind| match k {
        IconKind::Glyph { glyph, .. } => *glyph,
        other => panic!("expected glyph, got {other:?}"),
    };
    assert_eq!(glyph(pack.folder_icon(false).unwrap()), 'F');
    assert_eq!(glyph(pack.folder_icon(true).unwrap()), 'O');
    let mut sparse = IconPack::empty("sparse");
    sparse.folder = Some(IconKind::glyph('F'));
    assert_eq!(glyph(sparse.folder_icon(true).unwrap()), 'F');
}

// ── VS Code manifest loading ─────────────────────────────────────────────────

/// Write a minimal pack directory: a manifest + one SVG, VS Code style.
fn write_vscode_pack(dir: &std::path::Path, font_theme: bool) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let (icon_def, file_key) = if font_theme {
        (r##"{"fontCharacter": "\uE001", "fontColor": "#AABBCC"}"##.to_string(), "rs".to_string())
    } else {
        (r#"{"iconPath": "icon.svg"}"#.to_string(), ".rs".to_string())
    };
    let manifest = format!(
        r#"{{
            "name": "fixture",
            "iconDefinitions": {{ "_file": {icon_def} }},
            "file": "_file",
            "folder": "_file",
            "folderExpanded": "_file",
            "fileExtensions": {{ "{file_key}": "_file" }},
            "fileNames": {{ "Makefile": "_file" }}
        }}"#
    );
    let manifest_path = dir.join("icon-theme.json");
    std::fs::write(&manifest_path, manifest).unwrap();
    if !font_theme {
        std::fs::write(dir.join("icon.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="red"/></svg>"#)
            .unwrap();
    }
    manifest_path
}

#[test]
fn vscode_manifest_loads_svg_pack_from_directory() {
    let dir = std::env::temp_dir().join(format!("magma-icon-test-svg-{}", std::process::id()));
    write_vscode_pack(&dir, false);

    let pack = icons::find_and_load(&dir).expect("pack loads");
    assert_eq!(pack.name, "fixture");
    assert!(matches!(pack.default_file, Some(IconKind::Svg { .. })));
    let ext = pack.extensions.get("rs").expect("stripped leading dot");
    assert!(matches!(ext, IconKind::Svg { path } if path.ends_with("icon.svg")));
    assert!(pack.file_names.contains_key("Makefile"));
    assert!(pack.folder.is_some() && pack.folder_open.is_some());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn vscode_manifest_loads_font_character_themes() {
    let dir = std::env::temp_dir().join(format!("magma-icon-test-font-{}", std::process::id()));
    write_vscode_pack(&dir, true);

    let pack = icons::vscode::load_pack(&dir).expect("pack loads");
    match pack.extensions.get("rs") {
        Some(IconKind::Glyph { glyph, color }) => {
            assert_eq!(*glyph, '\u{E001}');
            assert_eq!(*color, Some((0xAA, 0xBB, 0xCC)));
        }
        other => panic!("expected colored glyph, got {other:?}"),
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_pack_is_an_error_not_a_panic() {
    let missing = std::env::temp_dir().join("magma-icon-no-such-pack");
    assert!(icons::find_and_load(&missing).is_err());
}

// ── icon-pack option follower ────────────────────────────────────────────────

#[test]
fn sync_option_restores_the_default_pack() {
    let mut ed = make_editor();
    ed.default_icons = glyph_pack();
    ed.icons = IconPack::empty("custom");

    ed.options.insert("icon-pack".into(), "default".into());
    icons::sync_option(&mut ed);

    assert_eq!(ed.icons.name, "test");
}

#[test]
fn sync_option_drops_an_option_that_fails_to_load() {
    let mut ed = make_editor();
    let before = ed.icons.name.clone();

    ed.options.insert("icon-pack".into(), "definitely-not-a-pack-42".into());
    icons::sync_option(&mut ed);

    assert_eq!(ed.icons.name, before, "broken option leaves the active pack alone");
    assert!(!ed.options.contains_key("icon-pack"), "the bad option is cleared");
    assert!(ed.status_message.is_some(), "the failure is surfaced, not swallowed");
}

#[test]
fn sync_option_ignores_matched_packs() {
    let mut ed = make_editor();
    ed.icons.name = "fixture".into();
    ed.options.insert("icon-pack".into(), "fixture".into());
    icons::sync_option(&mut ed);
    assert_eq!(ed.icons.name, "fixture");
}
