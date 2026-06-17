use crate::tests::helpers::make_editor;
use crate::kernel::semantic::language_registry::{LanguageInfo, LanguageRegistry};

// ── LanguageRegistry unit tests ───────────────────────────────────────────────

#[test]
fn new_registry_empty() {
    let r = LanguageRegistry::new();
    assert_eq!(r.count(), 0);
    assert!(r.list().is_empty());
}

#[test]
fn register_single_language() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(
        Some("/langs/rust/provider.janet".into()),
        None, None,
    ));
    assert_eq!(r.count(), 1);
    assert!(r.get("rust").is_some());
}

#[test]
fn register_multiple_languages() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(None, None, None));
    r.register(LanguageInfo::new("janet").with_paths(None, None, None));
    r.register(LanguageInfo::new("python").with_paths(None, None, None));
    assert_eq!(r.count(), 3);
}

#[test]
fn list_is_alphabetically_sorted() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("python").with_paths(None, None, None));
    r.register(LanguageInfo::new("rust").with_paths(None, None, None));
    r.register(LanguageInfo::new("janet").with_paths(None, None, None));
    let names: Vec<&str> = r.list().iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["janet", "python", "rust"]);
}

#[test]
fn get_returns_correct_paths() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(
        Some("/a/provider.janet".into()),
        Some("/a/syntax.scm".into()),
        Some("/a/config.janet".into()),
    ));
    let info = r.get("rust").unwrap();
    assert_eq!(info.provider_path.as_deref(), Some("/a/provider.janet"));
    assert_eq!(info.syntax_path.as_deref(), Some("/a/syntax.scm"));
    assert_eq!(info.config_path.as_deref(), Some("/a/config.janet"));
}

#[test]
fn get_missing_returns_none() {
    let r = LanguageRegistry::new();
    assert!(r.get("go").is_none());
}

#[test]
fn unregister_existing_returns_true() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(None, None, None));
    assert!(r.unregister("rust"));
    assert_eq!(r.count(), 0);
}

#[test]
fn unregister_missing_returns_false() {
    let mut r = LanguageRegistry::new();
    assert!(!r.unregister("nonexistent"));
}

#[test]
fn register_replaces_existing_entry() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(
        Some("/old/provider.janet".into()), None, None,
    ));
    r.register(LanguageInfo::new("rust").with_paths(
        Some("/new/provider.janet".into()), None, None,
    ));
    assert_eq!(r.count(), 1);
    assert_eq!(
        r.get("rust").unwrap().provider_path.as_deref(),
        Some("/new/provider.janet")
    );
}

#[test]
fn mark_loaded_sets_flag() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(None, None, None));
    assert!(!r.is_loaded("rust"));
    r.mark_loaded("rust");
    assert!(r.is_loaded("rust"));
}

#[test]
fn mark_loaded_unknown_does_not_panic() {
    let mut r = LanguageRegistry::new();
    r.mark_loaded("doesnotexist"); // must not panic
}

#[test]
fn is_loaded_unknown_returns_false() {
    let r = LanguageRegistry::new();
    assert!(!r.is_loaded("unknown"));
}

#[test]
fn newly_registered_language_is_not_loaded() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("janet").with_paths(None, None, None));
    assert!(!r.is_loaded("janet"));
    assert!(!r.get("janet").unwrap().loaded);
}

#[test]
fn language_info_partial_paths() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("janet").with_paths(
        Some("/langs/janet/provider.janet".into()),
        None,
        None,
    ));
    let info = r.get("janet").unwrap();
    assert!(info.provider_path.is_some());
    assert!(info.syntax_path.is_none());
    assert!(info.config_path.is_none());
}

#[test]
fn discover_nonexistent_dir_is_noop() {
    let mut r = LanguageRegistry::new();
    r.discover("/this/path/does/not/exist/at/all");
    assert_eq!(r.count(), 0);
}

#[test]
fn discover_empty_dir_registers_nothing() {
    use std::io::Write as _;
    let tmp = std::env::temp_dir().join(format!("magma_le_empty_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    let mut r = LanguageRegistry::new();
    r.discover(tmp.to_str().unwrap());
    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(r.count(), 0);
}

#[test]
fn discover_finds_language_with_all_files() {
    use std::io::Write as _;
    let tmp = std::env::temp_dir().join(format!("magma_le_full_{}", std::process::id()));
    let lang_dir = tmp.join("golangtest");
    std::fs::create_dir_all(&lang_dir).unwrap();
    for f in &["provider.janet", "syntax.scm", "config.janet"] {
        let mut file = std::fs::File::create(lang_dir.join(f)).unwrap();
        let _ = file.write_all(b"# placeholder");
    }

    let mut r = LanguageRegistry::new();
    r.discover(tmp.to_str().unwrap());
    std::fs::remove_dir_all(&tmp).ok();

    assert_eq!(r.count(), 1);
    let info = r.get("golangtest").unwrap();
    assert!(info.provider_path.is_some());
    assert!(info.syntax_path.is_some());
    assert!(info.config_path.is_some());
}

#[test]
fn discover_finds_language_with_provider_only() {
    use std::io::Write as _;
    let tmp = std::env::temp_dir().join(format!("magma_le_partial_{}", std::process::id()));
    let lang_dir = tmp.join("minimal");
    std::fs::create_dir_all(&lang_dir).unwrap();
    let mut f = std::fs::File::create(lang_dir.join("provider.janet")).unwrap();
    let _ = f.write_all(b"# provider");

    let mut r = LanguageRegistry::new();
    r.discover(tmp.to_str().unwrap());
    std::fs::remove_dir_all(&tmp).ok();

    let info = r.get("minimal").unwrap();
    assert!(info.provider_path.is_some());
    assert!(info.syntax_path.is_none());
    assert!(info.config_path.is_none());
}

// ── Editor integration ────────────────────────────────────────────────────────

#[test]
fn editor_has_empty_language_registry_on_new() {
    let ed = make_editor();
    assert_eq!(ed.language_registry.count(), 0);
}

#[test]
fn editor_language_registry_register_and_query() {
    let mut ed = make_editor();
    ed.language_registry.register(LanguageInfo::new("rust").with_paths(
        Some("/test/provider.janet".into()), None, None,
    ));
    assert_eq!(ed.language_registry.count(), 1);
    assert!(ed.language_registry.get("rust").is_some());
}

#[test]
fn editor_language_registry_mark_loaded() {
    let mut ed = make_editor();
    ed.language_registry.register(LanguageInfo::new("python").with_paths(None, None, None));
    assert!(!ed.language_registry.is_loaded("python"));
    ed.language_registry.mark_loaded("python");
    assert!(ed.language_registry.is_loaded("python"));
}

#[test]
fn editor_language_registry_unregister() {
    let mut ed = make_editor();
    ed.language_registry.register(LanguageInfo::new("janet").with_paths(None, None, None));
    assert!(ed.language_registry.unregister("janet"));
    assert_eq!(ed.language_registry.count(), 0);
}

#[test]
fn editor_language_registry_list_sorted() {
    let mut ed = make_editor();
    ed.language_registry.register(LanguageInfo::new("z-lang").with_paths(None, None, None));
    ed.language_registry.register(LanguageInfo::new("a-lang").with_paths(None, None, None));
    let names: Vec<&str> = ed.language_registry.list().iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names[0], "a-lang");
    assert_eq!(names[1], "z-lang");
}
