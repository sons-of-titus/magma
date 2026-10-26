//! Version-control integration for the editor.
//!
//! The backend abstraction and its built-in implementations (git, hg, svn,
//! jj) live in the `vcs` workspace crate.  This module owns what is
//! editor-facing: the VC session state on `Editor` and the side-by-side
//! diff layout in `ops`.

pub use vcs::backends;
pub mod ops;

mod event;
pub use event::Event;

pub use ops::{DiffData, DiffView, HiddenPane};
pub use vcs::{
    BranchInfo, FileStatus, StashEntry, VcBackend, VcRemoteOp, VcStatus, run_cmd, run_cmd_stdin,
    run_op, status_for_root,
};

use std::path::{Path, PathBuf};

// ── Editor state ──────────────────────────────────────────────────────────────

/// Mutable VC session state that lives on the `Editor`.
pub struct VcState {
    /// All registered backends, checked in order during detection.
    pub backends: Vec<Box<dyn VcBackend>>,
    /// Index into `backends` of the backend currently in use.
    pub active_backend: Option<usize>,
    /// Name of the active backend — the identity background op jobs use,
    /// since trait-object backends cannot cross into the pool by value.
    pub active_backend_name: Option<String>,
    /// Set by `:vc`; the GUI reveals the Commit tool window and clears it.
    pub request_open: bool,
    /// Detected repository root.
    pub repo_root: Option<PathBuf>,
    /// Last fetched status (drives the Commit tool window).
    pub last_status: Option<VcStatus>,
    /// When true the next commit amends instead of creating a new commit.
    pub amend_pending: bool,
    /// The side-by-side editor diff, while one is open.
    pub diff_view: Option<DiffView>,
    /// Branch names for the Commit window's branch widget.
    pub branches: Vec<String>,
    /// Saved stashes, as last listed.
    pub stashes: Vec<StashEntry>,
}

impl std::fmt::Debug for VcState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VcState")
            .field("active_backend", &self.active_backend)
            .field("repo_root", &self.repo_root)
            .finish()
    }
}

impl Default for VcState {
    fn default() -> Self {
        Self::new()
    }
}

impl VcState {
    pub fn new() -> Self {
        VcState {
            backends: vec![
                Box::new(backends::GitBackend),
                Box::new(backends::HgBackend),
                Box::new(backends::SvnBackend),
                Box::new(backends::JjBackend),
            ],
            active_backend: None,
            active_backend_name: None,
            request_open: false,
            repo_root: None,
            last_status: None,
            amend_pending: false,
            diff_view: None,
            branches: Vec::new(),
            stashes: Vec::new(),
        }
    }

    /// Detect which backend applies to `dir` and return `(backend_idx, root)`.
    pub fn detect_for(&self, dir: &Path) -> Option<(usize, PathBuf)> {
        for (i, backend) in self.backends.iter().enumerate() {
            if backend.detect(dir)
                && let Some(root) = backend.root(dir)
            {
                return Some((i, root));
            }
        }
        None
    }

    /// Return the active backend, if one has been selected.
    pub fn backend(&self) -> Option<&dyn VcBackend> {
        self.active_backend
            .and_then(|i| self.backends.get(i))
            .map(|b| b.as_ref())
    }
}
