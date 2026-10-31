//! The index subsystem's background-event vocabulary.
//!
//! The project walk and the per-file extraction workers produce these; the
//! editor wraps them in its own transport enum so the foreground keeps one
//! ordered channel while the crate owns the payload shapes.

use crate::IndexRecord;

/// An event produced by a project walk or a per-file extraction worker.
#[derive(Debug, Clone)]
pub enum Event {
    /// A walk batch for one project: the accumulated file list so far, in
    /// walk order.  Batches from a single walk arrive in order and each
    /// carries the full list to that point, so the foreground replaces its
    /// state statelessly; `done` marks the final batch.
    ProjectIndexed {
        project_name: String,
        generation: u64,
        files: Vec<String>,
        ignored_files: Vec<String>,
        done: bool,
    },
    /// One file's symbol extraction finished and is ready to merge.  Applied
    /// only while `generation` is still the active project's — an in-flight
    /// extraction from a superseded walk or project must not pollute the
    /// store of whatever is open now.
    RecordArrived {
        generation: u64,
        record: IndexRecord,
    },
    /// A file counted into the extraction queue yielded no record (missing
    /// on disk, or not UTF-8 — the index is a source index).  It still
    /// arrives so the queue depth the UI reports drains completely.
    RecordSkipped { generation: u64 },
    /// Size/mtime stamps for the fresh walk, measured off the foreground
    /// thread; the foreground diffs them against the index store to pick
    /// the files needing re-extraction.
    StampScan {
        generation: u64,
        stamps: Vec<(String, u64, u64)>,
    },
}
