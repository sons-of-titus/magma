//! Version-control abstraction layer.
//!
//! All VCS interaction goes through the `VcBackend` trait. Built-in
//! implementations cover Git, Mercurial (hg), Subversion (svn), and
//! Jujutsu (jj). Additional backends can be registered at runtime from
//! Janet via `vc/register-backend`.

pub mod backends;

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
}

// ── Backend trait ─────────────────────────────────────────────────────────────

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
}

// ── Editor state ──────────────────────────────────────────────────────────────

/// Number of display lines above the first file entry in the VC buffer.
pub const VC_HEADER_LINES: usize = 5;

/// Mutable VC session state that lives on the `Editor`.
pub struct VcState {
    /// All registered backends, checked in order during detection.
    pub backends: Vec<Box<dyn VcBackend>>,
    /// Index into `backends` of the backend currently in use.
    pub active_backend: Option<usize>,
    /// Slab key of the `*vc*` display buffer.
    pub buf_key: Option<usize>,
    /// Detected repository root.
    pub repo_root: Option<PathBuf>,
    /// Last fetched status (used to map cursor lines to file paths).
    pub last_status: Option<VcStatus>,
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
            buf_key: None,
            repo_root: None,
            last_status: None,
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

// ── Display ───────────────────────────────────────────────────────────────────

/// Build the full text for the `*vc*` buffer from a `VcStatus`.
pub fn build_display(root: &Path, status: &VcStatus) -> String {
    let root_str = root.to_string_lossy();
    let mut out = format!(
        "## {}  [{}]  {}\n\n",
        status.branch, status.backend_name, root_str,
    );

    if status.staged.is_empty() && status.unstaged.is_empty() && status.untracked.is_empty() {
        out.push_str("  Nothing to commit, working tree clean\n");
    } else {
        if !status.staged.is_empty() {
            out.push_str("Staged:\n");
            for f in &status.staged {
                out.push_str(&format!("  {}  {}\n", f.code, f.path));
            }
            out.push('\n');
        }
        if !status.unstaged.is_empty() {
            let label = if status.has_staging { "Unstaged:" } else { "Changed:" };
            out.push_str(&format!("{label}\n"));
            for f in &status.unstaged {
                out.push_str(&format!("  {}  {}\n", f.code, f.path));
            }
            out.push('\n');
        }
        if !status.untracked.is_empty() {
            out.push_str("Untracked:\n");
            for f in &status.untracked {
                out.push_str(&format!("  ?  {}\n", f));
            }
            out.push('\n');
        }
    }

    out.push_str("────────────────────────────────────────────────────────────────\n");
    if status.has_staging {
        out.push_str("s stage  u unstage  d diff  c commit  p push  l log  g refresh  q close\n");
    } else {
        out.push_str("d diff  c commit  p push  l log  g refresh  q close\n");
    }
    out
}

/// Return the file path on display line `line` (0-indexed) in the VC buffer.
pub fn path_at_line(status: &VcStatus, line: usize) -> Option<String> {
    if line < VC_HEADER_LINES { return None; }
    let mut row = VC_HEADER_LINES;
    // iterate sections in the same order as build_display
    let sections: [&[FileStatus]; 2] = [&status.staged, &status.unstaged];
    for section in sections {
        for f in section {
            if row == line { return Some(f.path.clone()); }
            row += 1;
        }
        if !section.is_empty() { row += 1; } // blank line after section
    }
    // untracked section header
    if !status.untracked.is_empty() {
        row += 1; // "Untracked:" label
        for f in &status.untracked {
            if row == line { return Some(f.clone()); }
            row += 1;
        }
    }
    None
}

// ── Shell helper (shared by backends and shell-template backends) ──────────────

/// Run a command in `dir` and return combined stdout + stderr.
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
