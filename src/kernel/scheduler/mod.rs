//! Work Scheduler — Phase 9 Concurrency Model.
//!
//! Defines the `Work` trait and `WorkScheduler` that manages background work
//! items with progress tracking and cancellation.  The Janet runtime stays on
//! the main thread; results arrive via `BackgroundEvent` variants handled in
//! `process_background_event`.

use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

pub type WorkId = u64;

/// Status of a work item.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkStatus {
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// How much of a work item has been processed.
#[derive(Debug, Clone, Default)]
pub struct WorkProgress {
    pub done: u64,
    pub total: u64,
}

/// Cancellation token passed into `Work::execute` so the work can detect early exit.
#[derive(Clone)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Passed into `Work::execute` so the work can push progress updates to the
/// main thread via `BackgroundEvent::WorkProgress`.
pub struct ProgressReporter {
    pub work_id: WorkId,
    pub(crate) sender: tokio::sync::mpsc::UnboundedSender<crate::kernel::runtime::BackgroundEvent>,
}

impl ProgressReporter {
    /// Report that `done` out of `total` units have been processed.
    pub fn report(&self, done: u64, total: u64) {
        let _ = self.sender.send(crate::kernel::runtime::BackgroundEvent::WorkProgress {
            id: self.work_id,
            done,
            total,
        });
    }
}

/// Unit of async background work with progress reporting and cancellation support.
pub trait Work: Send + 'static {
    fn name(&self) -> &str;
    fn execute(
        self: Box<Self>,
        progress: ProgressReporter,
        cancel: CancellationToken,
    ) -> Result<(), String>;
}

struct ClosureWork {
    name: String,
    f: Box<dyn FnOnce(ProgressReporter, CancellationToken) -> Result<(), String> + Send + 'static>,
}

impl Work for ClosureWork {
    fn name(&self) -> &str { &self.name }
    fn execute(
        self: Box<Self>,
        progress: ProgressReporter,
        cancel: CancellationToken,
    ) -> Result<(), String> {
        (self.f)(progress, cancel)
    }
}

struct WorkItem {
    name: String,
    progress: WorkProgress,
    cancel: CancellationToken,
    pub status: WorkStatus,
}

/// Orchestrates all background work: tracks progress, manages cancellation,
/// and maintains mappings between external IDs (task IDs, process IDs) and
/// their corresponding `WorkId` handles.
#[derive(Default)]
pub struct WorkScheduler {
    items: HashMap<WorkId, WorkItem>,
    next_id: WorkId,
    task_work: HashMap<u64, WorkId>,
    process_work: HashMap<u64, WorkId>,
}

impl WorkScheduler {
    pub fn new() -> Self {
        WorkScheduler {
            items: HashMap::new(),
            next_id: 1,
            task_work: HashMap::new(),
            process_work: HashMap::new(),
        }
    }

    /// Submit a typed `Work` item.  Spawns it on the blocking thread pool and
    /// returns its stable `WorkId`.  Completion/failure arrive as
    /// `BackgroundEvent::WorkCompleted` / `WorkFailed` on the main thread.
    pub fn submit<W: Work>(
        &mut self,
        work: W,
        bg: &crate::kernel::runtime::BackgroundHandle,
    ) -> WorkId {
        let id = self.next_id;
        self.next_id += 1;
        let cancel = CancellationToken::new();
        let cancel_token = cancel.clone();
        self.items.insert(id, WorkItem {
            name: work.name().to_string(),
            progress: WorkProgress::default(),
            cancel,
            status: WorkStatus::Running,
        });
        let progress_sender = bg.sender.clone();
        let done_sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let reporter = ProgressReporter { work_id: id, sender: progress_sender };
            let result = Box::new(work).execute(reporter, cancel_token);
            match result {
                Ok(()) => {
                    let _ = done_sender.send(
                        crate::kernel::runtime::BackgroundEvent::WorkCompleted { id },
                    );
                }
                Err(e) => {
                    let _ = done_sender.send(
                        crate::kernel::runtime::BackgroundEvent::WorkFailed { id, error: e },
                    );
                }
            }
        });
        id
    }

    /// Submit a closure as a named work item.
    pub fn submit_fn<F>(
        &mut self,
        name: String,
        bg: &crate::kernel::runtime::BackgroundHandle,
        f: F,
    ) -> WorkId
    where
        F: FnOnce(ProgressReporter, CancellationToken) -> Result<(), String> + Send + 'static,
    {
        self.submit(ClosureWork { name, f: Box::new(f) }, bg)
    }

    /// Register a `TaskScheduler` task as a tracked work item.
    /// The `task_id` is mapped to the returned `WorkId` so completion can be
    /// forwarded when `BackgroundEvent::TaskRunCompleted` arrives.
    pub fn register_task(&mut self, task_id: u64, name: &str) -> WorkId {
        let id = self.next_id;
        self.next_id += 1;
        self.items.insert(id, WorkItem {
            name: name.to_string(),
            progress: WorkProgress::default(),
            cancel: CancellationToken::new(),
            status: WorkStatus::Running,
        });
        self.task_work.insert(task_id, id);
        id
    }

    /// Register an external process (`process/spawn`) as a tracked work item.
    pub fn register_process(&mut self, process_id: u64, cmd: &str) -> WorkId {
        let id = self.next_id;
        self.next_id += 1;
        self.items.insert(id, WorkItem {
            name: cmd.to_string(),
            progress: WorkProgress::default(),
            cancel: CancellationToken::new(),
            status: WorkStatus::Running,
        });
        self.process_work.insert(process_id, id);
        id
    }

    /// Cancel a work item.  Sets the cancellation token and marks status
    /// Cancelled.  Directly-submitted work checks the token in its loop;
    /// externally-tracked work is not forcibly interrupted.
    pub fn cancel(&mut self, id: WorkId) {
        if let Some(item) = self.items.get_mut(&id) {
            item.cancel.cancel();
            if item.status == WorkStatus::Running {
                item.status = WorkStatus::Cancelled;
            }
        }
    }

    /// Mark a work item completed (called from `process_background_event`).
    pub fn mark_completed(&mut self, id: WorkId) {
        if let Some(item) = self.items.get_mut(&id) {
            if item.status == WorkStatus::Running {
                item.status = WorkStatus::Completed;
                item.progress.done = item.progress.total.max(1);
            }
        }
    }

    /// Mark a work item failed.
    pub fn mark_failed(&mut self, id: WorkId, error: String) {
        if let Some(item) = self.items.get_mut(&id) {
            if item.status == WorkStatus::Running {
                item.status = WorkStatus::Failed(error);
            }
        }
    }

    /// Update the progress counters of a work item.
    pub fn update_progress(&mut self, id: WorkId, done: u64, total: u64) {
        if let Some(item) = self.items.get_mut(&id) {
            item.progress.done = done;
            item.progress.total = total;
        }
    }

    /// Complete the work item mapped to `task_id`.
    pub fn complete_task(&mut self, task_id: u64) {
        if let Some(&work_id) = self.task_work.get(&task_id) {
            self.mark_completed(work_id);
        }
    }

    /// Fail the work item mapped to `task_id`.
    pub fn fail_task(&mut self, task_id: u64, error: String) {
        if let Some(&work_id) = self.task_work.get(&task_id) {
            self.mark_failed(work_id, error);
        }
    }

    /// Complete the work item mapped to `process_id`.
    pub fn complete_process(&mut self, process_id: u64) {
        if let Some(work_id) = self.process_work.remove(&process_id) {
            self.mark_completed(work_id);
        }
    }

    /// Fail the work item mapped to `process_id`.
    pub fn fail_process(&mut self, process_id: u64, error: String) {
        if let Some(work_id) = self.process_work.remove(&process_id) {
            self.mark_failed(work_id, error);
        }
    }

    pub fn progress(&self, id: WorkId) -> Option<WorkProgress> {
        self.items.get(&id).map(|item| item.progress.clone())
    }

    pub fn status(&self, id: WorkId) -> Option<&WorkStatus> {
        self.items.get(&id).map(|item| &item.status)
    }

    pub fn name(&self, id: WorkId) -> Option<&str> {
        self.items.get(&id).map(|item| item.name.as_str())
    }

    /// Return all tracked work items as `(id, name, status, progress)` tuples.
    pub fn list(&self) -> Vec<(WorkId, &str, &WorkStatus, &WorkProgress)> {
        let mut v: Vec<_> = self.items.iter()
            .map(|(&id, item)| (id, item.name.as_str(), &item.status, &item.progress))
            .collect();
        v.sort_by_key(|&(id, _, _, _)| id);
        v
    }

    pub fn active_count(&self) -> usize {
        self.items.values().filter(|i| i.status == WorkStatus::Running).count()
    }
}
