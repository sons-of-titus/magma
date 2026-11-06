//! Work Scheduler — Phase 9 Concurrency Model.
//!
//! The bookkeeping (ids, status, progress, cancellation) lives in the
//! `scheduler` crate and knows nothing about the editor.  This module bridges
//! it to the kernel: `BackgroundHandle` is the `WorkHost`, and `WorkEvent`s
//! become the matching `BackgroundEvent` variants that
//! `process_background_event` already handles.

pub use scheduler::{
    CancellationToken, ProgressReporter, Work, WorkEvent, WorkHost, WorkId, WorkProgress,
    WorkScheduler, WorkSink, WorkStatus,
};

use super::runtime::{BackgroundEvent, BackgroundHandle};

impl WorkHost for BackgroundHandle {
    fn spawn(&self, job: Box<dyn FnOnce() + Send + 'static>) {
        bgpool::Pool::spawn_blocking(self, job);
    }

    fn sink(&self) -> WorkSink {
        let sender = self.sender.clone();
        WorkSink::new(move |event| {
            logging::LogErr::log_warn(sender.send(match event {
                WorkEvent::Progress { id, done, total } => {
                    BackgroundEvent::WorkProgress { id, done, total }
                }
                WorkEvent::Completed { id } => BackgroundEvent::WorkCompleted { id },
                WorkEvent::Failed { id, error } => BackgroundEvent::WorkFailed { id, error },
            }));
        })
    }
}
