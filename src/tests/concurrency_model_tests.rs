//! Pure-Rust tests for Phase 9 — Concurrency Model.

use crate::kernel::scheduler::{
    CancellationToken, WorkId, WorkProgress, WorkScheduler, WorkStatus,
};
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;

fn bare_editor() -> Editor {
    Editor::new(Box::new(DiskFileSystem::new()))
}

// ── CancellationToken ─────────────────────────────────────────────────────────

#[test]
fn cancellation_token_starts_uncancelled() {
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());
}

#[test]
fn cancellation_token_cancel_sets_flag() {
    let token = CancellationToken::new();
    token.cancel();
    assert!(token.is_cancelled());
}

#[test]
fn cancellation_token_clone_shares_state() {
    let token = CancellationToken::new();
    let clone = token.clone();
    token.cancel();
    assert!(clone.is_cancelled());
}

// ── WorkScheduler basics ─────────────────────────────────────────────────────

#[test]
fn new_scheduler_is_empty() {
    let sched = WorkScheduler::new();
    assert_eq!(sched.list().len(), 0);
    assert_eq!(sched.active_count(), 0);
}

#[test]
fn register_task_returns_unique_ids() {
    let mut sched = WorkScheduler::new();
    let id1 = sched.register_task(1, "build");
    let id2 = sched.register_task(2, "test");
    assert_ne!(id1, id2);
}

#[test]
fn register_task_status_is_running() {
    let mut sched = WorkScheduler::new();
    let id = sched.register_task(42, "build");
    assert_eq!(sched.status(id), Some(&WorkStatus::Running));
}

#[test]
fn register_task_name_is_stored() {
    let mut sched = WorkScheduler::new();
    let id = sched.register_task(1, "my-task");
    assert_eq!(sched.name(id), Some("my-task"));
}

#[test]
fn complete_task_marks_completed() {
    let mut sched = WorkScheduler::new();
    let _ = sched.register_task(7, "build");
    sched.complete_task(7);
    // find the work item whose name is "build"
    let items = sched.list();
    let item = items.iter().find(|&&(_, n, _, _)| n == "build").unwrap();
    assert_eq!(*item.2, WorkStatus::Completed);
}

#[test]
fn fail_task_marks_failed() {
    let mut sched = WorkScheduler::new();
    let _ = sched.register_task(8, "flaky");
    sched.fail_task(8, "exit code 1".into());
    let items = sched.list();
    let item = items.iter().find(|&&(_, n, _, _)| n == "flaky").unwrap();
    assert!(matches!(item.2, WorkStatus::Failed(_)));
}

#[test]
fn complete_unknown_task_is_noop() {
    let mut sched = WorkScheduler::new();
    sched.complete_task(99999); // should not panic
    assert_eq!(sched.list().len(), 0);
}

// ── Progress tracking ─────────────────────────────────────────────────────────

#[test]
fn register_task_initial_progress_is_zero() {
    let mut sched = WorkScheduler::new();
    let id = sched.register_task(1, "parse");
    let p = sched.progress(id).unwrap();
    assert_eq!(p.done, 0);
    assert_eq!(p.total, 0);
}

#[test]
fn update_progress_stores_values() {
    let mut sched = WorkScheduler::new();
    let id = sched.register_task(1, "index");
    sched.update_progress(id, 50, 100);
    let p = sched.progress(id).unwrap();
    assert_eq!(p.done, 50);
    assert_eq!(p.total, 100);
}

#[test]
fn progress_on_unknown_id_returns_none() {
    let sched = WorkScheduler::new();
    assert!(sched.progress(999).is_none());
}

// ── Process tracking ─────────────────────────────────────────────────────────

#[test]
fn register_process_tracks_by_process_id() {
    let mut sched = WorkScheduler::new();
    let work_id = sched.register_process(42, "git status");
    assert_eq!(sched.status(work_id), Some(&WorkStatus::Running));
}

#[test]
fn complete_process_marks_item_completed() {
    let mut sched = WorkScheduler::new();
    let work_id = sched.register_process(5, "ls");
    sched.complete_process(5);
    assert_eq!(sched.status(work_id), Some(&WorkStatus::Completed));
}

#[test]
fn fail_process_marks_item_failed() {
    let mut sched = WorkScheduler::new();
    let work_id = sched.register_process(6, "bad-cmd");
    sched.fail_process(6, "exit code 127".into());
    assert!(matches!(sched.status(work_id), Some(WorkStatus::Failed(_))));
}

// ── Cancel ────────────────────────────────────────────────────────────────────

#[test]
fn cancel_running_item_marks_cancelled() {
    let mut sched = WorkScheduler::new();
    let id = sched.register_task(1, "long-build");
    sched.cancel(id);
    assert_eq!(sched.status(id), Some(&WorkStatus::Cancelled));
}

#[test]
fn cancel_unknown_id_is_noop() {
    let mut sched = WorkScheduler::new();
    sched.cancel(12345); // should not panic
}

// ── list() ────────────────────────────────────────────────────────────────────

#[test]
fn list_returns_all_items() {
    let mut sched = WorkScheduler::new();
    sched.register_task(1, "a");
    sched.register_task(2, "b");
    sched.register_process(3, "c");
    assert_eq!(sched.list().len(), 3);
}

#[test]
fn list_is_sorted_by_id() {
    let mut sched = WorkScheduler::new();
    sched.register_task(1, "first");
    sched.register_task(2, "second");
    let ids: Vec<WorkId> = sched.list().iter().map(|&(id, _, _, _)| id).collect();
    assert_eq!(ids, vec![ids[0], ids[1]]);
    assert!(ids[0] < ids[1]);
}

// ── Editor integration ────────────────────────────────────────────────────────

#[test]
fn editor_has_scheduler_field() {
    let ed = bare_editor();
    assert_eq!(ed.scheduler.list().len(), 0);
}

#[test]
fn editor_scheduler_tracks_tasks() {
    let mut ed = bare_editor();
    let _wid = ed.scheduler.register_task(1, "cargo-build");
    assert_eq!(ed.scheduler.active_count(), 1);
}
