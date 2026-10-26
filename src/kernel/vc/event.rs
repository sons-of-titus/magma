//! The version-control subsystem's background-event vocabulary.
//!
//! Repo-operation jobs and status/branch/stash/diff workers send these.  The
//! central `runtime::BackgroundEvent` wraps them as `BackgroundEvent::Vc(_)`,
//! so the foreground keeps one ordered channel while VC owns its payloads.

use std::path::PathBuf;

use crate::kernel::vc::{DiffData, StashEntry, VcStatus};

/// An event produced by a version-control operation or fetch.
#[derive(Debug, Clone)]
pub enum Event {
    /// A status fetch finished off the foreground thread.
    StatusFetched {
        backend_idx: usize,
        root: PathBuf,
        result: Result<VcStatus, String>,
    },
    /// A repository operation (stage/diff/log/push/blame/commit) finished.
    OpDone {
        scratch: Option<String>,
        then_refresh: bool,
        result: Result<String, String>,
    },
    /// A branch list for the Commit window's branch widget finished.
    BranchesReady { branches: Vec<String> },
    /// A stash list for the Commit window's stash section finished.
    StashListed { result: Result<Vec<StashEntry>, String> },
    /// A one-file diff bundle (HEAD + worktree + patch) finished.
    DiffReady { path: String, result: Result<DiffData, String> },
}
