//! Pure-Rust tests for the Project Model (Phase 5).

use crate::kernel::project::{
    BuildKind, BuildTarget, Config, Dependency, DependencyKind,
    Module, Project, ProjectManager,
};
use crate::kernel::task::TaskScheduler;

// ── Project struct defaults ───────────────────────────────────────────────────

#[test]
fn project_default_has_empty_phase5_fields() {
    let p = Project::default();
    assert!(p.language.is_empty());
    assert!(p.modules.is_empty());
    assert!(p.dependencies.is_empty());
    assert!(p.tasks.is_empty());
    assert!(p.build_targets.is_empty());
    assert!(p.runtime_config.env.is_empty());
}

#[test]
fn project_retains_legacy_fields() {
    let p = Project::default();
    assert!(p.root.is_none());
    assert!(p.name.is_none());
    assert!(p.workspace.is_none());
    assert!(p.files.is_empty());
    assert!(p.options.is_empty());
    assert!(!p.file_index_dirty);
}

// ── BuildTarget ───────────────────────────────────────────────────────────────

#[test]
fn build_target_kind_variants() {
    let debug_t = BuildTarget { name: "build".into(), kind: BuildKind::Debug, task_id: Some(1), artifacts: vec![] };
    let release_t = BuildTarget { name: "release".into(), kind: BuildKind::Release, task_id: None, artifacts: vec![] };
    assert_eq!(debug_t.kind, BuildKind::Debug);
    assert_eq!(release_t.kind, BuildKind::Release);
    assert!(release_t.task_id.is_none());
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

// ── Config ────────────────────────────────────────────────────────────────────

#[test]
fn config_default_empty() {
    let c = Config::default();
    assert!(c.env.is_empty());
}

#[test]
fn config_stores_env_vars() {
    let mut c = Config::default();
    c.env.insert("RUST_LOG".into(), "debug".into());
    assert_eq!(c.env.get("RUST_LOG").map(|s| s.as_str()), Some("debug"));
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
}

// ── ProjectManager::detect ────────────────────────────────────────────────────

#[test]
fn detect_rust_project_sets_language() {
    let tmp = std::env::temp_dir().join("phase5_detect_rust");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

    let mut pm = ProjectManager::default();
    let mut sched = TaskScheduler::new();
    pm.detect(&tmp.to_string_lossy(), &mut sched);

    assert_eq!(pm.project.language, "rust");
    assert_eq!(pm.project.build_targets.len(), 3);
    assert!(pm.project.tasks.len() == 3);

    let build_tgt = pm.project.build_targets.iter().find(|t| t.name == "build").unwrap();
    assert_eq!(build_tgt.kind, BuildKind::Debug);
    assert!(build_tgt.task_id.is_some());

    assert!(sched.task_by_name("build").is_some());
    assert!(sched.task_by_name("test").is_some());
    assert!(sched.task_by_name("run").is_some());

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_node_project_sets_language() {
    let tmp = std::env::temp_dir().join("phase5_detect_node");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("package.json"), r#"{"name":"test"}"#).unwrap();

    let mut pm = ProjectManager::default();
    let mut sched = TaskScheduler::new();
    pm.detect(&tmp.to_string_lossy(), &mut sched);

    assert_eq!(pm.project.language, "node");
    let build_task_id = sched.task_by_name("build").unwrap();
    let task = sched.tasks.get(&build_task_id).unwrap();
    assert_eq!(task.command, "npm run build");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_make_project_sets_language() {
    let tmp = std::env::temp_dir().join("phase5_detect_make");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("Makefile"), "all:").unwrap();

    let mut pm = ProjectManager::default();
    let mut sched = TaskScheduler::new();
    pm.detect(&tmp.to_string_lossy(), &mut sched);

    assert_eq!(pm.project.language, "make");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_unknown_project_does_nothing() {
    let tmp = std::env::temp_dir().join("phase5_detect_unknown");
    std::fs::create_dir_all(&tmp).unwrap();

    let mut pm = ProjectManager::default();
    let mut sched = TaskScheduler::new();
    pm.detect(&tmp.to_string_lossy(), &mut sched);

    assert!(pm.project.language.is_empty(), "unknown project must leave language empty");
    assert!(sched.tasks.is_empty(), "no tasks must be defined for unknown project");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn detect_redefines_tasks_idempotently() {
    let tmp = std::env::temp_dir().join("phase5_detect_idem");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

    let mut pm = ProjectManager::default();
    let mut sched = TaskScheduler::new();
    pm.detect(&tmp.to_string_lossy(), &mut sched);
    let id_first = sched.task_by_name("build").unwrap();
    pm.detect(&tmp.to_string_lossy(), &mut sched);
    let id_second = sched.task_by_name("build").unwrap();
    assert_eq!(id_first, id_second, "detect twice must not create duplicate tasks");
    assert_eq!(sched.tasks.len(), 3);

    std::fs::remove_dir_all(&tmp).ok();
}

// ── ProjectManager build/test/run preconditions ──────────────────────────────

#[test]
fn no_build_targets_before_detect() {
    let pm = ProjectManager::default();
    let sched = TaskScheduler::new();
    assert!(pm.project.build_targets.is_empty(), "no build targets before detect");
    assert!(sched.task_by_name("build").is_none(), "no tasks before detect");
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
