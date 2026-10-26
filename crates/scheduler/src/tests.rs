use super::*;

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Runs jobs inline and records every emitted event.
struct TestHost {
    events: Arc<Mutex<Vec<WorkEvent>>>,
    spawned: AtomicUsize,
}

impl TestHost {
    fn new() -> Self {
        Self { events: Arc::new(Mutex::new(Vec::new())), spawned: AtomicUsize::new(0) }
    }

    fn events(&self) -> Vec<WorkEvent> {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl WorkHost for TestHost {
    fn spawn(&self, job: Box<dyn FnOnce() + Send + 'static>) {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        job();
    }

    fn sink(&self) -> WorkSink {
        let events = self.events.clone();
        WorkSink::new(move |event| {
            events.lock().unwrap_or_else(|e| e.into_inner()).push(event);
        })
    }
}

// ── CancellationToken ────────────────────────────────────────────────────────

#[test]
fn token_starts_uncancelled() {
    assert!(!CancellationToken::new().is_cancelled());
}

#[test]
fn token_clone_observes_cancellation() {
    let token = CancellationToken::new();
    let clone = token.clone();
    token.cancel();
    assert!(clone.is_cancelled());
    assert!(token.is_cancelled());
}

// ── Bookkeeping ──────────────────────────────────────────────────────────────

#[test]
fn new_scheduler_is_empty() {
    let sched = WorkScheduler::new();
    assert!(sched.list().is_empty());
    assert_eq!(sched.active_count(), 0);
}

#[test]
fn ids_are_unique_and_status_starts_running() {
    let mut sched = WorkScheduler::new();
    let a = sched.register_task(1, "build");
    let b = sched.register_task(2, "test");
    assert_ne!(a, b);
    assert_eq!(sched.status(a), Some(&WorkStatus::Running));
    assert_eq!(sched.name(b), Some("test"));
    assert_eq!(sched.active_count(), 2);
}

#[test]
fn complete_and_fail_are_forwarded_from_task_ids() {
    let mut sched = WorkScheduler::new();
    let done = sched.register_task(7, "build");
    let broken = sched.register_task(8, "flaky");
    sched.complete_task(7);
    sched.fail_task(8, "exit code 1".into());
    assert_eq!(sched.status(done), Some(&WorkStatus::Completed));
    assert_eq!(sched.status(broken), Some(&WorkStatus::Failed("exit code 1".into())));
}

#[test]
fn complete_and_fail_are_forwarded_from_process_ids() {
    let mut sched = WorkScheduler::new();
    let done = sched.register_process(5, "ls");
    let broken = sched.register_process(6, "bad-cmd");
    sched.complete_process(5);
    sched.fail_process(6, "exit code 127".into());
    assert_eq!(sched.status(done), Some(&WorkStatus::Completed));
    assert!(matches!(sched.status(broken), Some(WorkStatus::Failed(_))));
}

#[test]
fn unknown_external_ids_are_noops() {
    let mut sched = WorkScheduler::new();
    sched.complete_task(99999);
    sched.fail_task(99999, "nope".into());
    sched.complete_process(99999);
    sched.fail_process(99999, "nope".into());
    sched.cancel(99999);
    assert!(sched.list().is_empty());
}

#[test]
fn completing_sets_progress_to_total() {
    let mut sched = WorkScheduler::new();
    let id = sched.register_task(1, "index");
    sched.update_progress(id, 4, 10);
    sched.mark_completed(id);
    let progress = sched.progress(id).unwrap();
    assert_eq!(progress.done, 10);
    assert_eq!(progress.total, 10);
}

#[test]
fn completed_items_are_not_overwritten_by_later_events() {
    let mut sched = WorkScheduler::new();
    let id = sched.register_task(1, "build");
    sched.mark_completed(id);
    sched.mark_failed(id, "late error".into());
    assert_eq!(sched.status(id), Some(&WorkStatus::Completed));
}

#[test]
fn cancel_marks_running_item_cancelled_and_sets_token() {
    let mut sched = WorkScheduler::new();
    let id = sched.register_task(1, "long-build");
    sched.cancel(id);
    assert_eq!(sched.status(id), Some(&WorkStatus::Cancelled));
    assert_eq!(sched.active_count(), 0);
}

#[test]
fn list_is_sorted_by_id() {
    let mut sched = WorkScheduler::new();
    sched.register_task(1, "a");
    sched.register_task(2, "b");
    sched.register_process(3, "c");
    let ids: Vec<WorkId> = sched.list().iter().map(|&(id, ..)| id).collect();
    assert_eq!(ids, vec![1, 2, 3]);
    assert_eq!(sched.list().len(), 3);
}

// ── Host integration ─────────────────────────────────────────────────────────

#[test]
fn submit_reports_progress_then_completion() {
    let mut sched = WorkScheduler::new();
    let host = TestHost::new();
    let id = sched.submit_fn("copy".into(), &host, |progress, cancel| {
        assert!(!cancel.is_cancelled());
        progress.report(0, 2);
        progress.report(1, 2);
        progress.report(2, 2);
        Ok(())
    });
    assert_eq!(host.spawned.load(Ordering::SeqCst), 1);
    assert_eq!(
        host.events(),
        vec![
            WorkEvent::Progress { id, done: 0, total: 2 },
            WorkEvent::Progress { id, done: 1, total: 2 },
            WorkEvent::Progress { id, done: 2, total: 2 },
            WorkEvent::Completed { id },
        ]
    );
}

#[test]
fn submit_reports_failure_with_the_error_message() {
    let mut sched = WorkScheduler::new();
    let host = TestHost::new();
    let id = sched.submit_fn("boom".into(), &host, |_progress, _cancel| {
        Err("exit code 3".into())
    });
    assert_eq!(
        host.events(),
        vec![WorkEvent::Failed { id, error: "exit code 3".into() }]
    );
    assert_eq!(sched.status(id), Some(&WorkStatus::Running));
}

#[test]
fn emitted_events_carry_the_id_the_scheduler_registered() {
    let mut sched = WorkScheduler::new();
    let host = TestHost::new();
    let id = sched.submit_fn("scan".into(), &host, |_progress, _cancel| Ok(()));
    assert_eq!(sched.name(id), Some("scan"));
    assert_eq!(host.events(), vec![WorkEvent::Completed { id }]);
    assert_eq!(sched.list().len(), 1);
}

#[test]
fn a_work_impl_receives_its_own_cancellation_token() {
    struct Probe {
        saw: Arc<AtomicBool>,
    }

    impl Work for Probe {
        fn name(&self) -> &str { "probe" }

        fn execute(
            self: Box<Self>,
            _progress: ProgressReporter,
            cancel: CancellationToken,
        ) -> Result<(), String> {
            self.saw.store(cancel.is_cancelled(), Ordering::SeqCst);
            Ok(())
        }
    }

    let mut sched = WorkScheduler::new();
    let host = TestHost::new();
    let saw = Arc::new(AtomicBool::new(true));
    let id = sched.submit(Probe { saw: saw.clone() }, &host);
    assert!(!saw.load(Ordering::SeqCst));
    assert_eq!(sched.name(id), Some("probe"));
}

#[test]
fn cancellation_is_visible_inside_the_running_job() {
    struct CancelsItself;

    impl Work for CancelsItself {
        fn name(&self) -> &str { "self-cancel" }

        fn execute(
            self: Box<Self>,
            _progress: ProgressReporter,
            cancel: CancellationToken,
        ) -> Result<(), String> {
            cancel.cancel();
            if cancel.is_cancelled() { Err("cancelled".into()) } else { Ok(()) }
        }
    }

    let mut sched = WorkScheduler::new();
    let host = TestHost::new();
    let id = sched.submit(CancelsItself, &host);
    assert!(matches!(host.events().as_slice(), [WorkEvent::Failed { id: failed, .. }] if *failed == id));
}
