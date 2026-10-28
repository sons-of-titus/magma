//! Version-control abstraction layer.
//!
//! All VCS interaction goes through the `VcBackend` trait. Built-in
//! implementations cover Git, Mercurial (hg), Subversion (svn), and
//! Jujutsu (jj). Additional backends can be registered at runtime from
//! Janet via `vc/register-backend`.
//!
//! This crate has no editor dependency: backends are stateless CLI wrappers
//! and `remote::run_op` re-detects one by name, so the kernel can run an
//! operation on a background thread without moving a trait object.

pub mod backends;
pub mod diff;
pub mod event;
pub mod remote;

pub use diff::DiffData;
pub use event::Event;
pub use remote::{VcRemoteOp, run_cmd, run_cmd_stdin, run_op, status_for_root};

#[cfg(test)]
mod tests;

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

/// Per-line blame attribution for one file line, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    /// Backend-specific short commit identifier (display only).
    pub commit: String,
    pub author: String,
    /// Short date (`YYYY-MM-DD`) when the backend provides one, else empty.
    pub date: String,
}

impl BlameLine {
    /// The one-line annotation rendered in the editor's prefix margin.
    pub fn annotation(&self) -> String {
        match (self.author.as_str(), self.date.as_str()) {
            ("", "") => self.commit.clone(),
            ("", d) => d.to_string(),
            (a, "") => a.to_string(),
            (a, d) => format!("{a} {d}"),
        }
    }
}

/// Serialize blame lines through the textual `run_op` channel: one
/// `commit\tauthor\tdate` record per line.
pub fn serialize_blame(lines: &[BlameLine]) -> String {
    lines
        .iter()
        .map(|l| format!("{}\t{}\t{}", l.commit, l.author, l.date))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Inverse of [`serialize_blame`].
pub fn parse_blame_tsv(raw: &str) -> Vec<BlameLine> {
    raw.lines()
        .map(|l| {
            let mut it = l.splitn(3, '\t');
            BlameLine {
                commit: it.next().unwrap_or("").to_string(),
                author: it.next().unwrap_or("").to_string(),
                date: it.next().unwrap_or("").to_string(),
            }
        })
        .collect()
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

    /// Stage every change in the worktree, including untracked files.
    fn stage_all(&self, _root: &Path) -> Result<(), String> {
        Err(format!("{}: stage-all not supported", self.name()))
    }

    /// Unstage everything, keeping the worktree.
    fn unstage_all(&self, _root: &Path) -> Result<(), String> {
        Err(format!("{}: unstage-all not supported", self.name()))
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String>;
    fn push(&self, root: &Path) -> Result<String, String>;
    /// Per-line blame for `file`; structured so the editor never parses
    /// backend prose.
    fn blame(&self, root: &Path, file: &str) -> Result<Vec<BlameLine>, String>;

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
        Err(format!(
            "{}: conflict resolution not supported",
            self.name()
        ))
    }

    /// Resolve a conflict by taking the "theirs" side and staging the result.
    fn resolve_theirs(&self, _root: &Path, _path: &str) -> Result<(), String> {
        Err(format!(
            "{}: conflict resolution not supported",
            self.name()
        ))
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
