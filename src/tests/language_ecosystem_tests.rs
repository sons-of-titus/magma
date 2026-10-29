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
        Some("/langs/rust/syntax.scm".into()),
        None,
    ));
    assert_eq!(r.count(), 1);
    assert!(r.get("rust").is_some());
}

#[test]
fn register_multiple_languages() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(None, None));
    r.register(LanguageInfo::new("janet").with_paths(None, None));
    r.register(LanguageInfo::new("python").with_paths(None, None));
    assert_eq!(r.count(), 3);
}

#[test]
fn list_is_alphabetically_sorted() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("python").with_paths(None, None));
    r.register(LanguageInfo::new("rust").with_paths(None, None));
    r.register(LanguageInfo::new("janet").with_paths(None, None));
    let names: Vec<&str> = r.list().iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["janet", "python", "rust"]);
}

#[test]
fn get_returns_correct_paths() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(
        Some("/a/syntax.scm".into()),
        Some("/a/config.janet".into()),
    ));
    let info = r.get("rust").unwrap();
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
    r.register(LanguageInfo::new("rust").with_paths(None, None));
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
        Some("/old/syntax.scm".into()), None,
    ));
    r.register(LanguageInfo::new("rust").with_paths(
        Some("/new/syntax.scm".into()), None,
    ));
    assert_eq!(r.count(), 1);
    assert_eq!(
        r.get("rust").unwrap().syntax_path.as_deref(),
        Some("/new/syntax.scm")
    );
}

#[test]
fn mark_loaded_sets_flag() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("rust").with_paths(None, None));
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
    r.register(LanguageInfo::new("janet").with_paths(None, None));
    assert!(!r.is_loaded("janet"));
    assert!(!r.get("janet").unwrap().loaded);
}

#[test]
fn language_info_partial_paths() {
    let mut r = LanguageRegistry::new();
    r.register(LanguageInfo::new("janet").with_paths(
        Some("/langs/janet/syntax.scm".into()),
        None,
    ));
    let info = r.get("janet").unwrap();
    assert!(info.syntax_path.is_some());
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
        Some("/test/syntax.scm".into()), None,
    ));
    assert_eq!(ed.language_registry.count(), 1);
    assert!(ed.language_registry.get("rust").is_some());
}

#[test]
fn editor_language_registry_mark_loaded() {
    let mut ed = make_editor();
    ed.language_registry.register(LanguageInfo::new("python").with_paths(None, None));
    assert!(!ed.language_registry.is_loaded("python"));
    ed.language_registry.mark_loaded("python");
    assert!(ed.language_registry.is_loaded("python"));
}

#[test]
fn editor_language_registry_unregister() {
    let mut ed = make_editor();
    ed.language_registry.register(LanguageInfo::new("janet").with_paths(None, None));
    assert!(ed.language_registry.unregister("janet"));
    assert_eq!(ed.language_registry.count(), 0);
}

#[test]
fn editor_language_registry_list_sorted() {
    let mut ed = make_editor();
    ed.language_registry.register(LanguageInfo::new("z-lang").with_paths(None, None));
    ed.language_registry.register(LanguageInfo::new("a-lang").with_paths(None, None));
    let names: Vec<&str> = ed.language_registry.list().iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names[0], "a-lang");
    assert_eq!(names[1], "z-lang");
}
