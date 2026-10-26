//! Work bookkeeping for background jobs: stable ids, status, progress, and
//! cooperative cancellation.
//!
//! The scheduler owns no threads and no event vocabulary.  It hands a job to
//! a [`WorkHost`], which decides where the job runs and how its lifecycle
//! events reach the foreground.  The events themselves are [`WorkEvent`], so
//! the crate has no dependency on the editor or on any specific runtime.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

pub type WorkId = u64;

/// Lifecycle state of a tracked work item.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkStatus {
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// How much of a work item has been processed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkProgress {
    pub done: u64,
    pub total: u64,
}

/// Lifecycle notification emitted by the scheduler.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkEvent {
    Progress { id: WorkId, done: u64, total: u64 },
    Completed { id: WorkId },
    Failed { id: WorkId, error: String },
}

/// Cheap cloneable handle that forwards [`WorkEvent`]s to the foreground.
/// Cloning shares the same destination.
#[derive(Clone)]
pub struct WorkSink(Arc<dyn Fn(WorkEvent) + Send + Sync>);

impl WorkSink {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(WorkEvent) + Send + Sync + 'static,
    {
        Self(Arc::new(f))
    }

    pub fn emit(&self, event: WorkEvent) {
        (self.0)(event);
    }
}

/// Where work runs and where its events go.
///
/// Implementors bridge to whatever concurrency primitive the host provides —
/// a blocking pool, an async runtime, or a synchronous test double.
pub trait WorkHost {
    /// Run `job` off the caller's thread (or inline, for a test host).
    fn spawn(&self, job: Box<dyn FnOnce() + Send + 'static>);
    /// Return a sender that delivers lifecycle events to the foreground.
    fn sink(&self) -> WorkSink;
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

/// Passed into `Work::execute` so the work can push progress updates.
pub struct ProgressReporter {
    pub work_id: WorkId,
    sink: WorkSink,
}

impl ProgressReporter {
    /// Report that `done` out of `total` units have been processed.
    pub fn report(&self, done: u64, total: u64) {
        self.sink.emit(WorkEvent::Progress { id: self.work_id, done, total });
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
    status: WorkStatus,
}

/// Tracks background work items and the external ids (task ids, process ids)
/// that map onto them.
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

    /// Submit a typed `Work` item.  Hands it to `host` and returns its stable
    /// `WorkId`.  Completion and failure arrive as `WorkEvent` on the host's
    /// event sink.
    pub fn submit<W: Work>(&mut self, work: W, host: &dyn WorkHost) -> WorkId {
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

        let progress_sink = host.sink();
        let done_sink = progress_sink.clone();
        host.spawn(Box::new(move || {
            let reporter = ProgressReporter { work_id: id, sink: progress_sink };
            match Box::new(work).execute(reporter, cancel_token) {
                Ok(()) => done_sink.emit(WorkEvent::Completed { id }),
                Err(error) => done_sink.emit(WorkEvent::Failed { id, error }),
            }
        }));
        id
    }

    /// Submit a closure as a named work item.
    pub fn submit_fn<F>(&mut self, name: String, host: &dyn WorkHost, f: F) -> WorkId
    where
        F: FnOnce(ProgressReporter, CancellationToken) -> Result<(), String> + Send + 'static,
    {
        self.submit(ClosureWork { name, f: Box::new(f) }, host)
    }

    /// Register an externally-driven task as a tracked work item.  The
    /// `task_id` is mapped to the returned `WorkId` so completion can be
    /// forwarded when the task reports back.
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

    /// Register an external process as a tracked work item.
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

    /// Mark a work item completed (called when its completion event arrives).
    pub fn mark_completed(&mut self, id: WorkId) {
        if let Some(item) = self.items.get_mut(&id)
            && item.status == WorkStatus::Running {
                item.status = WorkStatus::Completed;
                item.progress.done = item.progress.total.max(1);
            }
    }

    /// Mark a work item failed.
    pub fn mark_failed(&mut self, id: WorkId, error: String) {
        if let Some(item) = self.items.get_mut(&id)
            && item.status == WorkStatus::Running {
                item.status = WorkStatus::Failed(error);
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
