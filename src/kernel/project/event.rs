//! The project subsystem's background-event vocabulary.
//!
//! Find-files walks and project-wide searches send these.  The central
//! `runtime::BackgroundEvent` wraps them as `BackgroundEvent::Project(_)`, so
//! the foreground keeps one ordered channel while the project owns its
//! payloads.

use crate::kernel::task::QuickfixEntry;

/// An event produced by a project-wide file walk or content search.
#[derive(Debug, Clone)]
pub enum Event {
    /// A find-files walk finished; the foreground presents the results.
    FileSearchListed { pattern: String, files: Vec<String> },
    /// A streamed batch of project-search results; appended to the quickfix
    /// list when the generation is still current.
    GrepResultsBatch { generation: u64, entries: Vec<QuickfixEntry> },
    /// A project-wide search finished; the quickfix index resets.
    GrepDone { generation: u64 },
}
