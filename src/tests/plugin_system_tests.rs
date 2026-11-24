use crate::kernel::extension::{PluginRecord, PluginRegistry};

fn record(name: &str, deps: &[&str], langs: &[&str]) -> PluginRecord {
    PluginRecord {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        root: format!("/plugins/{name}"),
        entry_point: "main.janet".to_string(),
        dependencies: deps.iter().map(|s| s.to_string()).collect(),
        languages: langs.iter().map(|s| s.to_string()).collect(),
        loaded: false,
        description: None,
        author: None,
        homepage: None,
        tags: Vec::new(),
    }
}

#[test]
fn editor_starts_with_empty_plugin_registry() {
    let ed = crate::tests::helpers::make_editor();
    assert_eq!(ed.plugin_registry.count(), 0);
}

#[test]
fn registry_register_and_status_round_trip() {
    let mut reg = PluginRegistry::new();
    reg.register(record("rust", &[], &["rust"]));
    let p = reg.get("rust").unwrap();
    assert_eq!(p.version, "0.1.0");
    assert_eq!(p.entry_point, "main.janet");
    assert_eq!(p.languages, vec!["rust".to_string()]);
    assert!(!p.loaded);
}

#[test]
fn registry_re_register_resets_loaded() {
    let mut reg = PluginRegistry::new();
    reg.register(record("rust", &[], &[]));
    reg.get_mut("rust").unwrap().loaded = true;
    assert!(reg.is_loaded("rust"));
    reg.register(record("rust", &[], &[]));
    assert!(!reg.is_loaded("rust"));
}

#[test]
fn registry_list_sorted_and_counted() {
    let mut reg = PluginRegistry::new();
    reg.register(record("zig", &[], &[]));
    reg.register(record("rust", &["zig"], &["rust"]));
    reg.register(record("toml", &[], &["toml"]));
    assert_eq!(reg.count(), 3);
    let names: Vec<&str> = reg.list().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["rust", "toml", "zig"]);
}

#[test]
fn registry_unregister() {
    let mut reg = PluginRegistry::new();
    reg.register(record("rust", &[], &[]));
    assert!(reg.unregister("rust"));
    assert!(!reg.unregister("rust"));
    assert!(reg.get("rust").is_none());
}

#[test]
fn registry_dependencies_preserved() {
    let mut reg = PluginRegistry::new();
    reg.register(record("rust-analyzer-ui", &["rust"], &[]));
    assert_eq!(
        reg.get("rust-analyzer-ui").unwrap().dependencies,
        vec!["rust".to_string()]
    );
}
