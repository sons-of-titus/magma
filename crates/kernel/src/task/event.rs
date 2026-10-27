//! The task subsystem's background-event vocabulary.
//!
//! `TaskScheduler` sends these from its worker thread.  The central
//! `runtime::BackgroundEvent` wraps them as `BackgroundEvent::Task(_)`, so
//! the foreground keeps one ordered channel and one dispatch entry point
//! while the task subsystem owns the shape of what it emits.

use super::TaskId;

/// An event produced by a running or finishing task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// One streamed output line from a running task.
    OutputLine {
        id: TaskId,
        stream: String,
        line: String,
    },
    /// A task finished; carries the captured output and timing.
    RunCompleted {
        id: TaskId,
        exit_code: i32,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    },
}
