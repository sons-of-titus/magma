//! Pure-Rust tests for Phase 10 — Plugin Capability System.

use crate::kernel::extension::{Capability, ExtensionManifest, ExtensionRegistry};
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;

fn bare_editor() -> Editor {
    Editor::new(Box::new(DiskFileSystem::new()))
}

// ── Capability ────────────────────────────────────────────────────────────────

#[test]
fn capability_as_str_round_trips() {
    for (cap, expected) in [
        (Capability::Filesystem,       "filesystem"),
        (Capability::Network,          "network"),
        (Capability::ProcessExecution, "process-execution"),
        (Capability::EditorAccess,     "editor-access"),
    ] {
        assert_eq!(cap.as_str(), expected);
        assert_eq!(Capability::from_str(expected), Some(cap));
    }
}

#[test]
fn capability_from_str_unknown_returns_none() {
    assert_eq!(Capability::from_str("unknown-cap"), None);
    assert_eq!(Capability::from_str(""), None);
}

#[test]
fn capability_equality() {
    assert_eq!(Capability::Filesystem, Capability::Filesystem);
    assert_ne!(Capability::Filesystem, Capability::Network);
}

// ── ExtensionManifest ─────────────────────────────────────────────────────────

#[test]
fn manifest_has_capability_returns_true_when_declared() {
    let m = ExtensionManifest::new(
        "ext", "1.0",
        vec![Capability::Network, Capability::EditorAccess],
        "ext.janet",
    );
    assert!(m.has_capability(&Capability::Network));
    assert!(m.has_capability(&Capability::EditorAccess));
    assert!(!m.has_capability(&Capability::Filesystem));
    assert!(!m.has_capability(&Capability::ProcessExecution));
}

#[test]
fn manifest_empty_capabilities() {
    let m = ExtensionManifest::new("bare", "0.1", vec![], "bare.janet");
    assert!(!m.has_capability(&Capability::EditorAccess));
}

// ── ExtensionRegistry ─────────────────────────────────────────────────────────

#[test]
fn new_registry_is_empty() {
    let reg = ExtensionRegistry::new();
    assert_eq!(reg.count(), 0);
    assert!(reg.list().is_empty());
}

#[test]
fn register_single_extension() {
    let mut reg = ExtensionRegistry::new();
    reg.register(ExtensionManifest::new("my-ext", "1.0", vec![Capability::Filesystem], "my.janet"));
    assert_eq!(reg.count(), 1);
    assert!(reg.get("my-ext").is_some());
}

#[test]
fn register_replaces_existing() {
    let mut reg = ExtensionRegistry::new();
    reg.register(ExtensionManifest::new("ext", "1.0", vec![Capability::Filesystem], "e.janet"));
    reg.register(ExtensionManifest::new("ext", "2.0", vec![Capability::Network], "e.janet"));
    assert_eq!(reg.count(), 1);
    let m = reg.get("ext").unwrap();
    assert_eq!(m.version, "2.0");
    assert!(m.has_capability(&Capability::Network));
    assert!(!m.has_capability(&Capability::Filesystem));
}

#[test]
fn get_unknown_returns_none() {
    let reg = ExtensionRegistry::new();
    assert!(reg.get("ghost").is_none());
}

#[test]
fn unregister_removes_extension() {
    let mut reg = ExtensionRegistry::new();
    reg.register(ExtensionManifest::new("ext", "1.0", vec![], "e.janet"));
    assert_eq!(reg.count(), 1);
    reg.unregister("ext");
    assert_eq!(reg.count(), 0);
    assert!(reg.get("ext").is_none());
}

#[test]
fn unregister_unknown_is_noop() {
    let mut reg = ExtensionRegistry::new();
    reg.unregister("nothing"); // must not panic
    assert_eq!(reg.count(), 0);
}

#[test]
fn has_capability_true_when_declared() {
    let mut reg = ExtensionRegistry::new();
    reg.register(ExtensionManifest::new(
        "ext", "1.0",
        vec![Capability::Network, Capability::ProcessExecution],
        "e.janet",
    ));
    assert!(reg.has_capability("ext", &Capability::Network));
    assert!(reg.has_capability("ext", &Capability::ProcessExecution));
    assert!(!reg.has_capability("ext", &Capability::Filesystem));
    assert!(!reg.has_capability("ext", &Capability::EditorAccess));
}

#[test]
fn has_capability_false_for_unregistered() {
    let reg = ExtensionRegistry::new();
    assert!(!reg.has_capability("unknown", &Capability::EditorAccess));
    assert!(!reg.has_capability("unknown", &Capability::Filesystem));
}

#[test]
fn list_returns_sorted_by_name() {
    let mut reg = ExtensionRegistry::new();
    reg.register(ExtensionManifest::new("zebra", "1.0", vec![], "z.janet"));
    reg.register(ExtensionManifest::new("alpha", "1.0", vec![], "a.janet"));
    reg.register(ExtensionManifest::new("mango", "1.0", vec![], "m.janet"));
    let names: Vec<&str> = reg.list().iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mango", "zebra"]);
}

#[test]
fn list_multiple_extensions() {
    let mut reg = ExtensionRegistry::new();
    for i in 0..5 {
        reg.register(ExtensionManifest::new(
            format!("ext-{i}"), "1.0", vec![], "e.janet",
        ));
    }
    assert_eq!(reg.list().len(), 5);
}

// ── Editor integration ────────────────────────────────────────────────────────

#[test]
fn editor_has_extension_registry() {
    let ed = bare_editor();
    assert_eq!(ed.extension_registry.count(), 0);
}

#[test]
fn editor_extension_registry_register() {
    let mut ed = bare_editor();
    ed.extension_registry.register(ExtensionManifest::new(
        "my-plugin", "1.0",
        vec![Capability::EditorAccess],
        "plugin.janet",
    ));
    assert_eq!(ed.extension_registry.count(), 1);
    assert!(ed.extension_registry.has_capability("my-plugin", &Capability::EditorAccess));
    assert!(!ed.extension_registry.has_capability("my-plugin", &Capability::Network));
}

#[test]
fn editor_extension_registry_multiple_capabilities() {
    let mut ed = bare_editor();
    ed.extension_registry.register(ExtensionManifest::new(
        "power-ext", "2.0",
        vec![Capability::Filesystem, Capability::Network, Capability::ProcessExecution, Capability::EditorAccess],
        "power.janet",
    ));
    for cap in [Capability::Filesystem, Capability::Network, Capability::ProcessExecution, Capability::EditorAccess] {
        assert!(ed.extension_registry.has_capability("power-ext", &cap));
    }
}

#[test]
fn editor_extension_registry_unregister() {
    let mut ed = bare_editor();
    ed.extension_registry.register(ExtensionManifest::new("temp", "1.0", vec![], "t.janet"));
    assert_eq!(ed.extension_registry.count(), 1);
    ed.extension_registry.unregister("temp");
    assert_eq!(ed.extension_registry.count(), 0);
}

// ── Capability all_variants coverage ─────────────────────────────────────────

#[test]
fn all_capabilities_are_distinct() {
    let caps = [
        Capability::Filesystem,
        Capability::Network,
        Capability::ProcessExecution,
        Capability::EditorAccess,
    ];
    for i in 0..caps.len() {
        for j in 0..caps.len() {
            if i == j {
                assert_eq!(caps[i], caps[j]);
            } else {
                assert_ne!(caps[i], caps[j]);
            }
        }
    }
}

#[test]
fn manifest_with_all_capabilities() {
    let m = ExtensionManifest::new(
        "all", "1.0",
        vec![
            Capability::Filesystem,
            Capability::Network,
            Capability::ProcessExecution,
            Capability::EditorAccess,
        ],
        "all.janet",
    );
    assert_eq!(m.capabilities.len(), 4);
    assert!(m.has_capability(&Capability::Filesystem));
    assert!(m.has_capability(&Capability::Network));
    assert!(m.has_capability(&Capability::ProcessExecution));
    assert!(m.has_capability(&Capability::EditorAccess));
}
