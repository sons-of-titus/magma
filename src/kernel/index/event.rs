//! The project-index subsystem's background-event vocabulary.
//!
//! The project walk and the extraction workers send these.  The central
//! `runtime::BackgroundEvent` wraps them as `BackgroundEvent::Index(_)`, so
//! the foreground keeps one ordered channel while the index owns its payloads.

use crate::kernel::index::IndexRecord;

/// An event produced by a project walk or a per-file extraction worker.
#[derive(Debug, Clone)]
pub enum Event {
    /// A project's file list finished indexing.
    ProjectIndexed { project_name: String, files: Vec<String> },
    /// One file's symbol extraction finished and is ready to merge.
    RecordArrived(IndexRecord),
}
