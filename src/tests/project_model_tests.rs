//! Pure-Rust tests for the Project Model (Phase 5).

use crate::kernel::project::{
    Dependency, DependencyKind, Module, Project, ProjectManager,
};

// ── Project struct defaults ───────────────────────────────────────────────────

#[test]
fn project_default_has_empty_fields() {
    let p = Project::default();
    assert!(p.language.is_empty());
    assert!(p.modules.is_empty());
    assert!(p.dependencies.is_empty());
}

#[test]
fn project_retains_identity_fields() {
    let p = Project::default();
    assert!(p.root.is_none());
    assert!(p.name.is_none());
    assert!(p.workspace.is_none());
    assert!(p.files.is_empty());
    assert!(p.options.is_empty());
    assert!(!p.file_index_dirty);
}

// ── Module & Dependency ───────────────────────────────────────────────────────

#[test]
fn module_default_is_empty() {
    let m = Module::default();
    assert!(m.name.is_empty());
    assert!(m.files.is_empty());
    assert!(m.language.is_empty());
}

#[test]
fn dependency_kind_variants() {
    let ext = Dependency { name: "serde".into(), version: "1.0".into(), kind: DependencyKind::External };
    let int = Dependency { name: "core".into(), version: "0.1".into(), kind: DependencyKind::Internal };
    assert_eq!(ext.kind, DependencyKind::External);
    assert_eq!(int.kind, DependencyKind::Internal);
}

// ── ProjectManager ────────────────────────────────────────────────────────────

#[test]
fn project_manager_starts_empty() {
    let pm = ProjectManager::default();
    assert!(pm.project.root.is_none());
    assert!(pm.projects.is_empty());
    assert!(pm.current_project.is_none());
    assert!(pm.buffer_projects.is_empty());
    assert!(pm.recent_projects.is_empty());
    assert!(pm.run_configs.configs.is_empty());
    assert!(pm.run_configs.active.is_none());
}

// ── ProjectManager::detect ────────────────────────────────────────────────────

#[test]
fn detect_rust_project_seeds_run_configs() {
    let tmp = std::env::temp_dir().join("phase5_detect_rust");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

    let mut pm = ProjectManager::default();
    pm.detect(&tmp.to_string_lossy());

    assert_eq!(pm.project.language, "rust");
    assert_eq!(pm.run_configs.configs.len(), 3);
    assert_eq!(pm.run_configs.active.as_deref(), Some("build"));

    let build = pm.run_configs.named("build").unwrap();
    assert_eq!(build.command, "cargo");
    assert_eq!(build.args, vec!["build"]);
    let run = pm.run_configs.named("run").unwrap();
    assert_eq!(run.command, "cargo");
    assert_eq!(run.args, vec!["run"]);
    assert!(run.debuggable);

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_node_project_seeds_npm_configs() {
    let tmp = std::env::temp_dir().join("phase5_detect_node");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("package.json"), r#"{"name":"test"}"#).unwrap();

    let mut pm = ProjectManager::default();
    pm.detect(&tmp.to_string_lossy());

    assert_eq!(pm.project.language, "node");
    let build = pm.run_configs.named("build").unwrap();
    assert_eq!(build.command, "npm");
    assert_eq!(build.args, vec!["run", "build"]);

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_make_project_sets_language() {
    let tmp = std::env::temp_dir().join("phase5_detect_make");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("Makefile"), "all:").unwrap();

    let mut pm = ProjectManager::default();
    pm.detect(&tmp.to_string_lossy());

    assert_eq!(pm.project.language, "make");
    assert_eq!(pm.run_configs.configs.len(), 3);

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_unknown_project_does_nothing() {
    let tmp = std::env::temp_dir().join("phase5_detect_unknown");
    std::fs::create_dir_all(&tmp).unwrap();

    let mut pm = ProjectManager::default();
    pm.detect(&tmp.to_string_lossy());

    assert!(pm.project.language.is_empty(), "unknown project must leave language empty");
    assert!(pm.run_configs.configs.is_empty(), "unknown project must not seed configs");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_is_idempotent() {
    let tmp = std::env::temp_dir().join("phase5_detect_idem");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

    let mut pm = ProjectManager::default();
    pm.detect(&tmp.to_string_lossy());
    pm.detect(&tmp.to_string_lossy());

    assert_eq!(pm.run_configs.configs.len(), 3, "detect twice must not duplicate configs");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_prefers_saved_configs_over_defaults() {
    let tmp = std::env::temp_dir().join("phase5_detect_saved");
    let mag_dir = tmp.join(".magma");
    std::fs::create_dir_all(&mag_dir).unwrap();
    std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();
    std::fs::write(mag_dir.join("run-configs.json"), r#"{
        "configs": [{"name": "bench", "command": "cargo", "args": ["bench"]}],
        "active": "bench"
    }"#).unwrap();

    let mut pm = ProjectManager::default();
    pm.detect(&tmp.to_string_lossy());

    assert_eq!(pm.run_configs.configs.len(), 1, "saved configs must win over defaults");
    assert_eq!(pm.run_configs.active.as_deref(), Some("bench"));
    assert!(pm.run_configs.named("build").is_none());

    std::fs::remove_dir_all(&tmp).ok();
}

// ── sync_to_project_graph ─────────────────────────────────────────────────────

#[test]
fn sync_adds_files_to_project_graph() {
    let mut pm = ProjectManager::default();
    pm.project.language = "rust".to_string();
    pm.project.files = vec![
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
    ];

    let mut graph = crate::kernel::semantic::project_graph::ProjectGraph::new();
    pm.sync_to_project_graph(&mut graph);

    assert_eq!(graph.file_count(), 2);
    let node = graph.files.get("src/main.rs").unwrap();
    assert_eq!(node.language, "rust");
}

#[test]
fn sync_does_not_duplicate_files() {
    let mut pm = ProjectManager::default();
    pm.project.language = "rust".to_string();
    pm.project.files = vec!["src/main.rs".to_string()];

    let mut graph = crate::kernel::semantic::project_graph::ProjectGraph::new();
    pm.sync_to_project_graph(&mut graph);
    pm.sync_to_project_graph(&mut graph);

    assert_eq!(graph.file_count(), 1, "sync must not add duplicate entries");
}

#[test]
fn sync_without_language_uses_empty_string() {
    let mut pm = ProjectManager::default();
    pm.project.files = vec!["README.md".to_string()];

    let mut graph = crate::kernel::semantic::project_graph::ProjectGraph::new();
    pm.sync_to_project_graph(&mut graph);

    let node = graph.files.get("README.md").unwrap();
    assert_eq!(node.language, "");
}

// ── FileChanged → index dirty decision ────────────────────────────────────────
//
// The watcher reports every create/modify under the project root; only
// list-relevant changes may mark `file_index_dirty`, or any build/VCS churn
// re-indexes the whole project on the GUI's debounce.

#[test]
fn ignored_dir_churn_never_marks_the_index_dirty() {
    use crate::kernel::project::index_change_marks_dirty;

    let root = std::path::Path::new("/proj");
    let files = vec!["src/main.rs".to_string()];
    for path in [
        "/proj/target/debug/libmagma.rlib",
        "/proj/.git/index",
        "/proj/node_modules/pkg/index.js",
        "/proj/.zcode/session.json",
        "/proj/src/.hidden/state.bin",
    ] {
        for kind in ["modified", "created", "removed"] {
            assert!(!index_change_marks_dirty(root, path, kind, &std::collections::HashSet::from_iter(files.clone())),
                "{path} ({kind}) sits under an ignored dir");
        }
    }
}

#[test]
fn writing_an_indexed_file_does_not_dirty_the_index() {
    use crate::kernel::project::index_change_marks_dirty;

    let root = std::path::Path::new("/proj");
    let files = vec!["src/main.rs".to_string(), ".gitignore".to_string()];
    assert!(!index_change_marks_dirty(root, "/proj/src/main.rs", "modified", &std::collections::HashSet::from_iter(files.clone())),
        "a data write to an indexed file leaves the list unchanged");
    assert!(!index_change_marks_dirty(root, "/proj/.gitignore", "modified", &std::collections::HashSet::from_iter(files.clone())),
        "dotfiles at indexed levels are members too");
}

#[test]
fn creates_removals_and_unknown_paths_dirty_the_index() {
    use crate::kernel::project::index_change_marks_dirty;

    let root = std::path::Path::new("/proj");
    let files = vec!["src/main.rs".to_string()];
    assert!(index_change_marks_dirty(root, "/proj/src/extra.rs", "modified", &std::collections::HashSet::from_iter(files.clone())),
        "an unknown path reported as modified is a creation whose event was lost");
    assert!(index_change_marks_dirty(root, "/proj/src/main.rs", "removed", &std::collections::HashSet::from_iter(files.clone())));
    assert!(index_change_marks_dirty(root, "/proj/src/new.rs", "created", &std::collections::HashSet::from_iter(files.clone())));
    assert!(!index_change_marks_dirty(root, "/elsewhere/file.txt", "created", &std::collections::HashSet::from_iter(files.clone())),
        "paths outside the root are not the index's business");
}
