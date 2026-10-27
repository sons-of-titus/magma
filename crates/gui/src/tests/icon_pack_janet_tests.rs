//! Icon pack Janet API — default pack installation, overrides, and
//! VS Code pack loading through the `icons/` C functions.


use kernel::icons::{self, IconKind};
use scripting;
use crate::tests::icon_pack_tests::glyph_pack;

fn lookup_glyph(ed: &kernel::state::Editor, name: &str) -> char {
    match ed.icons.lookup(name) {
        Some(IconKind::Glyph { glyph, .. }) => *glyph,
        other => panic!("expected glyph for {name}, got {other:?}"),
    }
}

// ── default pack installed at init ───────────────────────────────────────────

#[test]
fn default_pack_is_installed_by_the_janet_builtin() {
    janet_test!(ed, {
        assert_eq!(ed.icons.name, "default");
        assert!(ed.icons.default_file.is_some(), "icons.janet installed the pack at init");
        assert!(ed.icons.folder.is_some());
        assert_eq!(ed.default_icons.name, "default",
            "the default pack is snapshotted for the icon-pack option");
        assert!(ed.default_icons.default_file.is_some());
    });
}

#[test]
fn default_pack_maps_common_extensions() {
    janet_test!(ed, {
        assert_eq!(lookup_glyph(&ed, "main.rs"), '\u{e7a8}');
        assert_eq!(lookup_glyph(&ed, "app.py"), '\u{e606}');
        assert_eq!(lookup_glyph(&ed, "notes.md"), '\u{e609}');
        assert_eq!(lookup_glyph(&ed, "Cargo.toml"), '\u{e615}');
        assert_eq!(lookup_glyph(&ed, "Makefile"), '\u{e673}');
        assert_eq!(lookup_glyph(&ed, "Dockerfile"), '\u{f21f}');
    });
}

#[test]
fn every_default_pack_glyph_is_covered_by_the_bundled_icon_font() {
    use eframe::egui::epaint::text::{FontId, Fonts, TextOptions};

    janet_test!(ed, {
        let mut fonts = Fonts::new(
            TextOptions { max_texture_side: 4096, ..Default::default() },
            crate::render::gui_fonts::font_definitions(&ed.font_config),
        );
        let names = ["main.rs", "lib.janet", "app.py", "Cargo.toml", "README.md",
                     "data.json", "conf.yaml", "conf.yml", "run.sh", "Cargo.lock",
                     "main.go", "Main.java", "index.js", "app.ts", "page.html",
                     "style.css", "config.xml", "notes.txt", "image.png", "app.vue",
                     "Makefile", "Dockerfile", "LICENSE", ".gitignore"];
        for name in names {
            let covered = match ed.icons.lookup(name) {
                Some(IconKind::Glyph { glyph, .. }) => {
                    fonts.has_glyph(&FontId::proportional(13.0), *glyph)
                }
                _ => false,
            };
            assert!(covered, "{name} has no pack entry or its glyph renders as tofu");
        }
        // And the whole pack is tofu-free, not just the sampled names.
        for kind in ed.icons.extensions.values() {
            if let IconKind::Glyph { glyph, .. } = kind {
                assert!(
                    fonts.has_glyph(&FontId::proportional(13.0), *glyph),
                    "pack glyph {glyph:?} (U+{:04X}) is missing from the bundled icon font",
                    *glyph as u32,
                );
            }
        }
    });
}

// ── overrides ────────────────────────────────────────────────────────────────

#[test]
fn override_changes_and_clears_one_extension() {
    janet_test!(ed, {
        assert_eq!(scripting::eval(r#"(icons/override "rs" 0xF00D [1 2 3])"#), "ok");
        assert_eq!(lookup_glyph(&ed, "main.rs"), '\u{f00d}');
        // Dotted keys normalize onto the same override slot.
        assert_eq!(scripting::eval(r#"(icons/override ".rs" nil)"#), "ok");
        assert_eq!(lookup_glyph(&ed, "main.rs"), '\u{e7a8}');
    });
}

#[test]
fn use_default_drops_all_overrides() {
    janet_test!(ed, {
        scripting::eval(r#"(icons/override "rs" 0xF00D)"#);
        scripting::eval(r#"(icons/override "py" 0xF00E)"#);
        scripting::eval("(icons/use-default)");
        assert_eq!(lookup_glyph(&ed, "main.rs"), '\u{e7a8}');
        assert_eq!(lookup_glyph(&ed, "app.py"), '\u{e606}');
        assert_eq!(ed.options.get("icon-pack").map(String::as_str), Some("default"),
            "use-default keeps the persisted option consistent");
    });
}

// ── custom packs / disk loading ──────────────────────────────────────────────

#[test]
fn set_pack_installs_a_custom_glyph_pack() {
    janet_test!(ed, {
        let result = scripting::eval(
            r#"(icons/set-pack "custom" @{:files @{"zz" 0x41} :default-file 0x42 :folder 0x43})"#);
        assert_eq!(result, "ok");
        assert_eq!(lookup_glyph(&ed, "x.zz"), 'A');
        assert_eq!(lookup_glyph(&ed, "anything.bin"), 'B');
        // A pack named something other than "default" is not snapshotted.
        assert_eq!(ed.default_icons.name, "default");
    });
}

#[test]
fn load_pack_reads_a_vscode_theme_from_disk() {
    janet_test!(ed, {
        let dir = std::env::temp_dir().join(format!("magma-icon-janet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("icon-theme.json"),
            r#"{"name": "janet-fixture", "iconDefinitions": {"_": {"iconPath": "i.svg"}},
                "file": "_", "fileExtensions": {".rs": "_"}}"#).unwrap();
        std::fs::write(dir.join("i.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>"#).unwrap();

        let result = scripting::eval(
            &format!(r#"(icons/load-pack "{}")"#, dir.display()));
        assert_eq!(result, "ok", "load-pack failed: {result}");
        assert_eq!(ed.icons.name, "janet-fixture");
        assert_eq!(ed.options.get("icon-pack").map(String::as_str), Some("janet-fixture"));
        assert!(matches!(
            ed.icons.extensions.get("rs"), Some(IconKind::Svg { .. })));

        // And back to the default, in the way the option follower expects.
        scripting::eval("(icons/use-default)");
        assert_eq!(ed.options.get("icon-pack").map(String::as_str), Some("default"));
        assert_eq!(ed.icons.name, "default");
        std::fs::remove_dir_all(&dir).ok();
    });
}

#[test]
fn sync_option_follows_the_persisted_option() {
    janet_test!(ed, {
        ed.default_icons = glyph_pack();
        ed.icons.name = "elsewhere".into();
        ed.options.insert("icon-pack".into(), "default".into());
        icons::sync_option(&mut ed);
        assert_eq!(ed.icons.name, "test");
    });
}
