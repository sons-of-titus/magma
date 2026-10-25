//! Version-control remote operations and shell helpers.
//!
//! `VcRemoteOp` names one repository operation, `run_op` executes it against a
//! freshly detected backend (backends are stateless CLI wrappers, so the pool
//! can re-detect by name), and `run_cmd`/`run_cmd_stdin` are the shared
//! process primitives.  Split from `vc::mod` at the operations boundary.

use std::path::Path;

use super::{VcBackend, VcStatus, backends};

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
    /// File content at a revision — the left half of the side-by-side diff.
    CatFile { rev: String, path: String },
    /// Resolve a conflict by keeping one side, then stage.
    ResolveOurs(String),
    ResolveTheirs(String),
    /// Remote sync; the combined output lands in a read-only buffer.
    Fetch,
    Pull,
}

/// The three pieces the side-by-side editor diff renders.
#[derive(Debug, Clone, Default)]
pub struct DiffData {
    /// File content at HEAD — the left pane.
    pub head: String,
    /// Working-tree content — the right pane.
    pub work: String,
    /// Zero-context diff; its hunks drive per-hunk stage/discard.
    pub patch: String,
}

/// One pane the diff view concealed while open — enough to put the user's
/// editor layout back exactly as it was.
#[derive(Debug, Clone)]
pub struct HiddenPane {
    pub id: crate::kernel::state::id::WindowId,
    pub buffer_id: Option<usize>,
    pub layout: crate::kernel::render::view_tree::LayoutConstraint,
    pub weight: f32,
}

/// The side-by-side editor diff: two linked panes that stand in for the whole
/// editor layout while open.  The two halves read as a single tab
/// (`{file}:HEAD->TREE`); `hidden` carries the panes they displaced.
#[derive(Debug, Clone)]
pub struct DiffView {
    pub left: crate::kernel::state::id::WindowId,
    pub right: crate::kernel::state::id::WindowId,
    pub left_buf: usize,
    pub right_buf: usize,
    /// Repository-relative path the two panes show.
    pub path: String,
    /// Zero-context patch the view was built from — the hunk source.
    pub patch: String,
    /// Every pane the diff concealed, with its buffer, layout and weight.
    pub hidden: Vec<HiddenPane>,
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
        VcRemoteOp::CatFile { rev, path } => backend.cat_file(root, rev, path),
        VcRemoteOp::ResolveOurs(path) => backend.resolve_ours(root, path).map(|_| String::new()),
        VcRemoteOp::ResolveTheirs(path) => backend.resolve_theirs(root, path).map(|_| String::new()),
        VcRemoteOp::Fetch => backend.fetch(root),
        VcRemoteOp::Pull => backend.pull(root),
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
