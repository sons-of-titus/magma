//! Version-control abstraction layer.
//!
//! All VCS interaction goes through the `VcBackend` trait. Built-in
//! implementations cover Git, Mercurial (hg), Subversion (svn), and
//! Jujutsu (jj). Additional backends can be registered at runtime from
//! Janet via `vc/register-backend`.

pub mod backends;
pub mod ops;

pub use ops::{DiffData, DiffView, HiddenPane, VcRemoteOp, run_cmd, run_cmd_stdin, run_op, status_for_root};

use std::path::{Path, PathBuf};

// ── Public data types ─────────────────────────────────────────────────────────

/// Status of a single changed file.
#[derive(Debug, Clone)]
pub struct FileStatus {
    /// Single-character status code (M, A, D, R, ?, …).
    pub code: char,
    pub path: String,
}

/// Aggregated repository status returned by a backend.
#[derive(Debug, Clone)]
pub struct VcStatus {
    pub backend_name: String,
    pub branch: String,
    /// True when the backend has an explicit staging area (git).
    pub has_staging: bool,
    pub staged: Vec<FileStatus>,
    pub unstaged: Vec<FileStatus>,
    pub untracked: Vec<String>,
    /// Paths with unresolved merge conflicts (F-23): shown once, with a
    /// resolve action, instead of being double-listed as staged+unstaged.
    pub conflicts: Vec<String>,
}

// ── Backend trait ─────────────────────────────────────────────────────────────

/// A named branch; `current` marks the checked-out one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchInfo {
    pub name: String,
    pub current: bool,
}

/// One saved stash: index (as listed by the backend) + message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
}

/// One version-control backend. All methods receive the **repo root** directory.
pub trait VcBackend: Send + Sync {
    fn name(&self) -> &str;

    /// Return true when `dir` (or any ancestor) is controlled by this VCS.
    fn detect(&self, dir: &Path) -> bool;

    /// Return the repo root containing `dir`, or `None` if not found.
    fn root(&self, dir: &Path) -> Option<PathBuf>;

    fn status(&self, root: &Path) -> Result<VcStatus, String>;
    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String>;
    fn log(&self, root: &Path, limit: usize) -> Result<String, String>;

    /// Stage (add) a file. Returns `Err` when staging is not supported.
    fn stage(&self, root: &Path, file: &str) -> Result<(), String>;
    /// Unstage a file. Returns `Err` when staging is not supported.
    fn unstage(&self, root: &Path, file: &str) -> Result<(), String>;

    fn commit(&self, root: &Path, message: &str) -> Result<(), String>;
    fn push(&self, root: &Path) -> Result<String, String>;
    fn blame(&self, root: &Path, file: &str) -> Result<String, String>;

    // ── Branch / hunk / stash / amend surface (F-23) ─────────────────────
    // Default: unsupported.  Backends implement the slices they can.

    fn list_branches(&self, _root: &Path) -> Result<Vec<BranchInfo>, String> {
        Err(format!("{}: branch listing not supported", self.name()))
    }

    /// Check out `branch`; `create` makes it first from HEAD.
    fn checkout(&self, _root: &Path, _branch: &str, _create: bool) -> Result<(), String> {
        Err(format!("{}: branch checkout not supported", self.name()))
    }

    /// Apply a single-hunk patch to the index (hunk staging).
    fn stage_patch(&self, _root: &Path, _patch: &str) -> Result<(), String> {
        Err(format!("{}: hunk staging not supported", self.name()))
    }

    /// Reverse-apply a single-hunk patch to the worktree (hunk discard).
    fn discard_patch(&self, _root: &Path, _patch: &str) -> Result<(), String> {
        Err(format!("{}: hunk discard not supported", self.name()))
    }

    /// Zero-context diff of one file for the hunk view.
    fn hunk_diff(&self, _root: &Path, _file: &str) -> Result<String, String> {
        Err(format!("{}: hunk view not supported", self.name()))
    }

    /// Amend the last commit — with `message` when given, otherwise keeping
    /// the previous message.
    fn amend(&self, _root: &Path, _message: Option<&str>) -> Result<(), String> {
        Err(format!("{}: amend not supported", self.name()))
    }

    fn stash_list(&self, _root: &Path) -> Result<Vec<StashEntry>, String> {
        Err(format!("{}: stash not supported", self.name()))
    }

    fn stash_push(&self, _root: &Path, _message: Option<&str>) -> Result<(), String> {
        Err(format!("{}: stash not supported", self.name()))
    }

    /// Apply stash `index` to the worktree, dropping it when `pop`.
    fn stash_apply(&self, _root: &Path, _index: usize, _pop: bool) -> Result<(), String> {
        Err(format!("{}: stash not supported", self.name()))
    }

    fn stash_drop(&self, _root: &Path, _index: usize) -> Result<(), String> {
        Err(format!("{}: stash not supported", self.name()))
    }

    // ── File / merge / remote surface (GUI diff and conflict resolution) ──

    /// File content at `rev` — `HEAD`, or a merge stage `:2:` (ours) / `:3:`
    /// (theirs).  Feeds the side-by-side diff's left half.
    fn cat_file(&self, _root: &Path, _rev: &str, _path: &str) -> Result<String, String> {
        Err(format!("{}: file-at-revision not supported", self.name()))
    }

    /// Resolve a conflict by taking the "ours" side and staging the result.
    fn resolve_ours(&self, _root: &Path, _path: &str) -> Result<(), String> {
        Err(format!("{}: conflict resolution not supported", self.name()))
    }

    /// Resolve a conflict by taking the "theirs" side and staging the result.
    fn resolve_theirs(&self, _root: &Path, _path: &str) -> Result<(), String> {
        Err(format!("{}: conflict resolution not supported", self.name()))
    }

    /// Fetch from the remote, returning the command's combined output.
    fn fetch(&self, _root: &Path) -> Result<String, String> {
        Err(format!("{}: fetch not supported", self.name()))
    }

    /// Pull from the remote, returning the command's combined output.
    fn pull(&self, _root: &Path) -> Result<String, String> {
        Err(format!("{}: pull not supported", self.name()))
    }
}

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
                && let Some(root) = backend.root(dir) {
                    return Some((i, root));
                }
        }
        None
    }

    /// Return the active backend, if one has been selected.
    pub fn backend(&self) -> Option<&dyn VcBackend> {
        self.active_backend.and_then(|i| self.backends.get(i)).map(|b| b.as_ref())
    }
}

