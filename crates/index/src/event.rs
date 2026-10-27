//! The index subsystem's background-event vocabulary.
//!
//! The project walk and the per-file extraction workers produce these; the
//! editor wraps them in its own transport enum so the foreground keeps one
//! ordered channel while the crate owns the payload shapes.

use crate::IndexRecord;

/// An event produced by a project walk or a per-file extraction worker.
#[derive(Debug, Clone)]
pub enum Event {
    /// A project's file list finished indexing.
    ProjectIndexed { project_name: String, files: Vec<String>, ignored_files: Vec<String> },
    /// One file's symbol extraction finished and is ready to merge.
    RecordArrived(IndexRecord),
}
