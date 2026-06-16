//! Pure-Rust tests for the Task System (Phase 4).

use std::collections::HashMap;
use crate::kernel::task::{TaskOutput, TaskScheduler, TaskStatus};

// ── TaskScheduler basics ─────────────────────────────────────────────────────

#[test]
fn scheduler_starts_empty() {
    let s = TaskScheduler::new();
    assert!(s.tasks.is_empty());
    assert!(s.named.is_empty());
    assert_eq!(s.next_id, 1);
}

#[test]
fn define_creates_task() {
    let mut s = TaskScheduler::new();
    let id = s.define("build".into(), "cargo build".into(), HashMap::new(), vec![]);
    assert_eq!(s.tasks.len(), 1);
    assert_eq!(s.named.get("build"), Some(&id));
    let task = s.tasks.get(&id).unwrap();
    assert_eq!(task.name, "build");
    assert_eq!(task.command, "cargo build");
    assert_eq!(task.status, TaskStatus::Pending);
}

#[test]
fn define_same_name_updates_task_and_returns_same_id() {
    let mut s = TaskScheduler::new();
    let id1 = s.define("build".into(), "cargo build".into(), HashMap::new(), vec![]);
    let id2 = s.define("build".into(), "make".into(), HashMap::new(), vec![]);
    assert_eq!(id1, id2, "same name must yield the same id");
    assert_eq!(s.tasks.len(), 1);
    assert_eq!(s.tasks.get(&id1).unwrap().command, "make");
}

#[test]
fn define_preserves_running_status() {
    let mut s = TaskScheduler::new();
    let id = s.define("build".into(), "cargo build".into(), HashMap::new(), vec![]);
    s.tasks.get_mut(&id).unwrap().status = TaskStatus::Running;
    // Redefine while Running — status must not be reset
    s.define("build".into(), "make".into(), HashMap::new(), vec![]);
    assert_eq!(s.tasks.get(&id).unwrap().status, TaskStatus::Running);
}

#[test]
fn task_by_name_found_and_not_found() {
    let mut s = TaskScheduler::new();
    let id = s.define("test".into(), "cargo test".into(), HashMap::new(), vec![]);
    assert_eq!(s.task_by_name("test"), Some(id));
    assert_eq!(s.task_by_name("run"), None);
}

// ── Status ───────────────────────────────────────────────────────────────────

#[test]
fn status_pending_initially() {
    let mut s = TaskScheduler::new();
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![]);
    assert_eq!(s.status(id), Some(&TaskStatus::Pending));
}

#[test]
fn status_not_found_for_unknown_id() {
    let s = TaskScheduler::new();
    assert_eq!(s.status(9999), None);
}

// ── Cancellation ─────────────────────────────────────────────────────────────

#[test]
fn cancel_pending_task() {
    let mut s = TaskScheduler::new();
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![]);
    s.cancel(id);
    assert_eq!(s.status(id), Some(&TaskStatus::Cancelled));
}

#[test]
fn cancel_running_task() {
    let mut s = TaskScheduler::new();
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![]);
    s.tasks.get_mut(&id).unwrap().status = TaskStatus::Running;
    s.cancel(id);
    assert_eq!(s.status(id), Some(&TaskStatus::Cancelled));
}

#[test]
fn cancel_unknown_id_is_noop() {
    let mut s = TaskScheduler::new();
    s.cancel(9999); // must not panic
}

// ── Completion ───────────────────────────────────────────────────────────────

fn make_output(exit_code: i32) -> TaskOutput {
    TaskOutput { stdout: "out".into(), stderr: String::new(), exit_code, duration_ms: 10 }
}

#[test]
fn mark_completed_updates_status_and_output() {
    let mut s = TaskScheduler::new();
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![]);
    s.tasks.get_mut(&id).unwrap().status = TaskStatus::Running;
    s.mark_completed(id, make_output(0));
    assert_eq!(s.status(id), Some(&TaskStatus::Completed));
    assert!(s.output(id).is_some());
    assert_eq!(s.output(id).unwrap().exit_code, 0);
}

#[test]
fn mark_failed_updates_status() {
    let mut s = TaskScheduler::new();
    let id = s.define("x".into(), "false".into(), HashMap::new(), vec![]);
    s.tasks.get_mut(&id).unwrap().status = TaskStatus::Running;
    s.mark_failed(id, "exit code 1".into(), make_output(1));
    assert!(matches!(s.status(id), Some(TaskStatus::Failed(_))));
}

#[test]
fn mark_completed_ignores_cancelled_task() {
    let mut s = TaskScheduler::new();
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![]);
    s.cancel(id);
    s.mark_completed(id, make_output(0));
    assert_eq!(s.status(id), Some(&TaskStatus::Cancelled), "cancelled must not flip to completed");
}

#[test]
fn output_nil_when_pending() {
    let mut s = TaskScheduler::new();
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![]);
    assert!(s.output(id).is_none());
}

// ── Dependency resolution ─────────────────────────────────────────────────────

#[test]
fn dependencies_satisfied_with_no_deps() {
    let mut s = TaskScheduler::new();
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![]);
    assert!(s.dependencies_satisfied(id));
}

#[test]
fn dependencies_satisfied_with_completed_dep() {
    let mut s = TaskScheduler::new();
    let dep = s.define("dep".into(), "echo dep".into(), HashMap::new(), vec![]);
    s.tasks.get_mut(&dep).unwrap().status = TaskStatus::Completed;
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![dep]);
    assert!(s.dependencies_satisfied(id));
}

#[test]
fn dependencies_not_satisfied_with_pending_dep() {
    let mut s = TaskScheduler::new();
    let dep = s.define("dep".into(), "echo dep".into(), HashMap::new(), vec![]);
    let id = s.define("x".into(), "echo x".into(), HashMap::new(), vec![dep]);
    // dep is still Pending → x is not ready
    assert!(!s.dependencies_satisfied(id));
}

#[test]
fn ready_to_run_only_returns_pending_with_no_unsatisfied_deps() {
    let mut s = TaskScheduler::new();
    let id_a = s.define("a".into(), "echo a".into(), HashMap::new(), vec![]);
    let id_b = s.define("b".into(), "echo b".into(), HashMap::new(), vec![id_a]);
    let ready = s.ready_to_run();
    assert!(ready.contains(&id_a), "a has no deps → should be ready");
    assert!(!ready.contains(&id_b), "b depends on pending a → not ready");
}

#[test]
fn ready_to_run_includes_b_after_a_completes() {
    let mut s = TaskScheduler::new();
    let id_a = s.define("a".into(), "echo a".into(), HashMap::new(), vec![]);
    let id_b = s.define("b".into(), "echo b".into(), HashMap::new(), vec![id_a]);
    s.tasks.get_mut(&id_a).unwrap().status = TaskStatus::Completed;
    let ready = s.ready_to_run();
    assert!(ready.contains(&id_b));
}

// ── Auto-detection (promoted to ProjectManager::detect in Phase 5) ───────────

#[test]
fn auto_detect_cargo_project() {
    let dir = std::env::temp_dir().join("magma_test_detect_cargo");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname=\"x\"").ok();
    let _ = std::fs::remove_file(dir.join("package.json"));
    let _ = std::fs::remove_file(dir.join("Makefile"));

    let mut pm = crate::kernel::project::ProjectManager::default();
    let mut s = TaskScheduler::new();
    pm.detect(&dir.to_string_lossy(), &mut s);
    assert_eq!(pm.project.language, "rust");
    assert!(s.task_by_name("build").is_some(), "build task should be defined");
    assert_eq!(s.tasks.get(&s.task_by_name("build").unwrap()).unwrap().command, "cargo build");
    assert!(s.task_by_name("test").is_some());
    assert!(s.task_by_name("run").is_some());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn auto_detect_no_markers_defines_nothing() {
    let dir = std::env::temp_dir().join("magma_test_detect_empty");
    std::fs::create_dir_all(&dir).ok();
    let _ = std::fs::remove_file(dir.join("Cargo.toml"));
    let _ = std::fs::remove_file(dir.join("package.json"));
    let _ = std::fs::remove_file(dir.join("Makefile"));
    let _ = std::fs::remove_file(dir.join("setup.py"));
    let _ = std::fs::remove_file(dir.join("pyproject.toml"));

    let mut pm = crate::kernel::project::ProjectManager::default();
    let mut s = TaskScheduler::new();
    pm.detect(&dir.to_string_lossy(), &mut s);
    assert!(s.tasks.is_empty(), "no markers → no tasks defined");
    assert!(pm.project.language.is_empty(), "no markers → no language detected");

    std::fs::remove_dir_all(&dir).ok();
}
