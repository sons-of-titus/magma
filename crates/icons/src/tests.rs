use super::*;

fn glyph_pack() -> IconPack {
    let mut pack = IconPack::empty("test");
    pack.extensions.insert("ts".into(), IconKind::glyph('A'));
    pack.extensions
        .insert("spec.ts".into(), IconKind::glyph('B'));
    pack.extensions.insert("md".into(), IconKind::glyph('C'));
    pack.file_names
        .insert("Cargo.toml".into(), IconKind::glyph('D'));
    pack.default_file = Some(IconKind::glyph('?'));
    pack.folder = Some(IconKind::glyph('F'));
    pack.folder_open = Some(IconKind::glyph('O'));
    pack
}

fn glyph_of(kind: &IconKind) -> char {
    match kind {
        IconKind::Glyph { glyph, .. } => *glyph,
        other => panic!("expected glyph, got {other:?}"),
    }
}

// ── Lookup order ─────────────────────────────────────────────────────────────

#[test]
fn exact_file_name_beats_extension() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(pack.lookup("Cargo.toml").unwrap()), 'D');
}

#[test]
fn longest_extension_suffix_wins() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(pack.lookup("app.spec.ts").unwrap()), 'B');
    assert_eq!(glyph_of(pack.lookup("main.ts").unwrap()), 'A');
}

#[test]
fn extensions_match_case_insensitively() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(pack.lookup("README.MD").unwrap()), 'C');
}

#[test]
fn file_names_keep_their_case() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(pack.lookup("cargo.toml").unwrap()), '?');
}

#[test]
fn unknown_files_fall_back_to_the_pack_default() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(pack.lookup("mystery.bin").unwrap()), '?');

    let sparse = IconPack::empty("sparse");
    assert!(sparse.lookup("mystery.bin").is_none());
}

#[test]
fn folders_use_the_open_icon_only_when_expanded() {
    let pack = glyph_pack();
    assert_eq!(glyph_of(pack.folder_icon(false).unwrap()), 'F');
    assert_eq!(glyph_of(pack.folder_icon(true).unwrap()), 'O');
}

#[test]
fn a_pack_without_folder_open_reuses_the_closed_icon() {
    let mut sparse = IconPack::empty("sparse");
    sparse.folder = Some(IconKind::glyph('F'));
    assert_eq!(glyph_of(sparse.folder_icon(true).unwrap()), 'F');
    assert!(sparse.folder_icon(false).is_some());
}

#[test]
fn empty_pack_is_named_and_carries_no_icons() {
    let pack = IconPack::empty("blank");
    assert_eq!(pack.name, "blank");
    assert!(pack.file_names.is_empty() && pack.extensions.is_empty());
    assert!(pack.lookup("anything.ts").is_none());
    assert!(pack.folder_icon(true).is_none());
}

// ── VS Code manifest loading ─────────────────────────────────────────────────

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("icons-{tag}-{}-{nanos}", std::process::id()))
}

/// Write a minimal pack directory: a manifest + one SVG, VS Code style.
fn write_vscode_pack(dir: &Path, font_theme: bool) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let (icon_def, file_key) = if font_theme {
        (
            r##"{"fontCharacter": "\uE001", "fontColor": "#AABBCC"}"##.to_string(),
            "rs".to_string(),
        )
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
fn manifest_loads_an_svg_pack_from_a_directory() {
    let dir = temp_dir("svg");
    write_vscode_pack(&dir, false);

    let pack = find_and_load(&dir).expect("pack loads");
    assert_eq!(pack.name, "fixture");
    assert!(matches!(pack.default_file, Some(IconKind::Svg { .. })));
    let ext = pack.extensions.get("rs").expect("leading dot stripped");
    assert!(matches!(ext, IconKind::Svg { path } if path.ends_with("icon.svg")));
    assert!(pack.file_names.contains_key("Makefile"));
    assert!(pack.folder.is_some() && pack.folder_open.is_some());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn manifest_loads_a_font_character_theme() {
    let dir = temp_dir("font");
    write_vscode_pack(&dir, true);

    let pack = vscode::load_pack(&dir).expect("pack loads");
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
fn a_manifest_file_can_be_loaded_directly() {
    let dir = temp_dir("direct");
    let manifest = write_vscode_pack(&dir, true);

    let pack = vscode::load_pack(&manifest).expect("pack loads");
    assert_eq!(pack.name, "fixture");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn codepoint_forms_are_all_accepted() {
    let dir = temp_dir("codepoints");
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = r##"{
        "iconDefinitions": {
            "a": {"fontCharacter": "0xE001"},
            "b": {"fontCharacter": "&#xE002;"},
            "c": {"fontCharacter": "#"}
        },
        "fileExtensions": {"a": "a", "b": "b", "c": "c"}
    }"##;
    std::fs::write(dir.join("icon-theme.json"), manifest).unwrap();

    let pack = vscode::load_pack(&dir).expect("pack loads");
    assert_eq!(glyph_of(&pack.extensions["a"]), '\u{E001}');
    assert_eq!(glyph_of(&pack.extensions["b"]), '\u{E002}');
    assert_eq!(glyph_of(&pack.extensions["c"]), '#');

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn eight_digit_colors_drop_the_alpha_channel() {
    let dir = temp_dir("color8");
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = r##"{
        "iconDefinitions": {"a": {"fontCharacter": "x", "fontColor": "#80AABBCC"}},
        "fileExtensions": {"a": "a"}
    }"##;
    std::fs::write(dir.join("icon-theme.json"), manifest).unwrap();

    let pack = vscode::load_pack(&dir).expect("pack loads");
    match &pack.extensions["a"] {
        IconKind::Glyph { color, .. } => assert_eq!(*color, Some((0xAA, 0xBB, 0xCC))),
        other => panic!("expected glyph, got {other:?}"),
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_manifest_is_found_by_its_icon_definitions_key() {
    let dir = temp_dir("scan");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("other.json"), r#"{"unrelated": true}"#).unwrap();
    std::fs::write(
        dir.join("zzz.json"),
        r#"{"iconDefinitions": {"a": {"fontCharacter": "x"}}, "file": "a"}"#,
    )
    .unwrap();

    let pack = vscode::load_pack(&dir).expect("pack loads");
    assert!(pack.default_file.is_some());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_directory_without_a_manifest_is_an_error() {
    let dir = temp_dir("empty");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(vscode::load_pack(&dir).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_missing_pack_is_an_error_not_a_panic() {
    let missing = std::env::temp_dir().join("icons-no-such-pack-directory");
    assert!(find_and_load(&missing).is_err());
}

#[test]
fn pack_search_dirs_prefers_the_user_config_then_the_project() {
    let dirs = pack_search_dirs();
    assert!(
        dirs.last()
            .is_some_and(|d| d.ends_with(".magma/icon-packs"))
    );
    if std::env::var("HOME").is_ok_and(|h| !h.is_empty()) {
        assert!(
            dirs.first()
                .is_some_and(|d| d.ends_with(".config/magma/icon-packs"))
        );
    }
}
