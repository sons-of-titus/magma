//! Version-control abstraction layer.
//!
//! All VCS interaction goes through the `VcBackend` trait. Built-in
//! implementations cover Git, Mercurial (hg), Subversion (svn), and
//! Jujutsu (jj). Additional backends can be registered at runtime from
//! Janet via `vc/register-backend`.

pub mod backends;
mod display;

pub use display::{VC_HEADER_LINES, build_display, path_at_line};

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
    /// Slab key of the `*vc*` display buffer.
    pub buf_key: Option<usize>,
    /// Detected repository root.
    pub repo_root: Option<PathBuf>,
    /// Last fetched status (used to map cursor lines to file paths).
    pub last_status: Option<VcStatus>,
    /// When true the next `vc-commit-save` amends instead of committing.
    pub amend_pending: bool,
    /// Single-hunk patches behind the `*vc-hunks*` buffer, aligned with
    /// the hunk markers shown in it.
    pub last_hunks: Vec<String>,
    /// Path the inline diff preview belongs to (Commit tool window).
    pub preview_path: Option<String>,
    /// Latest inline diff text for `preview_path`.
    pub preview: Option<String>,
    /// Branch names for the Commit window's branch widget.
    pub branches: Vec<String>,
}

impl std::fmt::Debug for VcState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VcState")
            .field("active_backend", &self.active_backend)
            .field("buf_key", &self.buf_key)
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
            buf_key: None,
            repo_root: None,
            last_status: None,
            amend_pending: false,
            last_hunks: Vec::new(),
            preview_path: None,
            preview: None,
            branches: Vec::new(),
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

// ── Shell helper (shared by backends and shell-template backends) ──────────────

/// Run a command in `dir` and return combined stdout + stderr.
/// One background-dispatchable repository operation.
#[derive(Debug, Clone)]
pub enum VcRemoteOp {
    Stage(String),
    Unstage(String),
    Diff(Option<String>),
    Log,
    Push,
    Blame(String),
    Commit(String),
    /// Hunk-level staging: the single-hunk patch goes to the backend's stdin.
    StagePatch(String),
    /// Hunk-level discard: reverse-apply in the worktree.
    DiscardPatch(String),
    /// Zero-context diff of one file — the granularity the hunk splitter
    /// and `git apply` want (default context merges nearby hunks).
    HunkDiff(String),
    /// Unified diff of one file for the inline preview (routed to
    /// `VcPreviewReady`, not a scratch buffer).
    PreviewDiff(String),
    /// Discard all worktree changes to one file (diff, then reverse-apply).
    DiscardFile(String),
    /// Amend the last commit, keeping or replacing its message.
    Amend(Option<String>),
    ListBranches,
    Checkout { branch: String, create: bool },
    StashPush(Option<String>),
    /// List stashes; rendered as `stash@{N}  message` lines.
    StashList,
    StashApply { index: usize, pop: bool },
    StashDrop(usize),
}

/// Run `op` against the named backend at `root`.  Backends are stateless
/// CLI wrappers, so a fresh detection by name is equivalent to the
/// foreground's registry entry.
pub fn run_op(root: &Path, backend_name: &str, op: &VcRemoteOp) -> Result<String, String> {
    let backend = detect_backends(root).into_iter()
        .find(|b| b.name() == backend_name)
        .ok_or_else(|| format!("no '{backend_name}' backend available"))?;
    match op {
        VcRemoteOp::Stage(path) => backend.stage(root, path).map(|_| String::new()),
        VcRemoteOp::Unstage(path) => backend.unstage(root, path).map(|_| String::new()),
        VcRemoteOp::Diff(file) => backend.diff(root, file.as_deref()),
        VcRemoteOp::Log => backend.log(root, 30),
        VcRemoteOp::Push => backend.push(root),
        VcRemoteOp::Blame(path) => backend.blame(root, path),
        VcRemoteOp::Commit(message) => backend.commit(root, message).map(|_| String::new()),
        VcRemoteOp::StagePatch(patch) => backend.stage_patch(root, patch).map(|_| String::new()),
        VcRemoteOp::DiscardPatch(patch) => backend.discard_patch(root, patch).map(|_| String::new()),
        VcRemoteOp::HunkDiff(file) => backend.hunk_diff(root, file),
        VcRemoteOp::PreviewDiff(file) => backend.diff(root, Some(file)),
        VcRemoteOp::DiscardFile(file) => {
            let patch = backend.hunk_diff(root, file)?;
            backend.discard_patch(root, &patch).map(|_| String::new())
        }
        VcRemoteOp::Amend(msg) => backend.amend(root, msg.as_deref()).map(|_| String::new()),
        VcRemoteOp::ListBranches => {
            let branches = backend.list_branches(root)?;
            Ok(branches.iter()
                .map(|b| format!("{} {}", if b.current { "*" } else { " " }, b.name))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        VcRemoteOp::Checkout { branch, create } => {
            backend.checkout(root, branch, *create).map(|_| String::new())
        }
        VcRemoteOp::StashPush(msg) => backend.stash_push(root, msg.as_deref()).map(|_| String::new()),
        VcRemoteOp::StashList => {
            let entries = backend.stash_list(root)?;
            Ok(entries.iter()
                .map(|e| format!("stash@{{{}}}  {}", e.index, e.message))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        VcRemoteOp::StashApply { index, pop } => {
            backend.stash_apply(root, *index, *pop).map(|_| String::new())
        }
        VcRemoteOp::StashDrop(index) => backend.stash_drop(root, *index).map(|_| String::new()),
    }
}

/// Detect the backend for `root` and fetch its status.  Runs on background
/// threads for the async refresh path (backends are stateless).
pub fn status_for_root(root: &Path) -> Result<VcStatus, String> {
    for backend in &detect_backends(root) {
        if let Ok(status) = backend.status(root) {
            return Ok(status);
        }
    }
    Err("no version-control backend for this root".to_string())
}

fn detect_backends(_root: &Path) -> Vec<Box<dyn VcBackend>> {
    vec![
        Box::new(backends::GitBackend),
        Box::new(backends::HgBackend),
        Box::new(backends::SvnBackend),
        Box::new(backends::JjBackend),
    ]
}

pub fn run_cmd(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if out.status.success() || stderr.is_empty() {
        Ok(stdout)
    } else if stdout.is_empty() {
        Err(stderr)
    } else {
        Ok(format!("{stdout}\n{stderr}"))
    }
}

/// Like `run_cmd`, but feeds `stdin_data` to the child — how patch text
/// reaches `git apply` without a temp file.
pub fn run_cmd_stdin(dir: &Path, program: &str, args: &[&str], stdin_data: &str) -> Result<String, String> {
    use std::io::Write;
    let mut child = std::process::Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_data.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| format!("{program}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if out.status.success() || stderr.is_empty() {
        Ok(stdout)
    } else if stdout.is_empty() {
        Err(stderr)
    } else {
        Ok(format!("{stdout}\n{stderr}"))
    }
}
