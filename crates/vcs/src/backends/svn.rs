//! Subversion backend.

use crate::{FileStatus, VcBackend, VcStatus, run_cmd};
use std::path::{Path, PathBuf};

pub struct SvnBackend;

impl VcBackend for SvnBackend {
    fn name(&self) -> &str {
        "svn"
    }

    fn detect(&self, dir: &Path) -> bool {
        let mut d = dir.to_path_buf();
        loop {
            if d.join(".svn").is_dir() {
                return true;
            }
            if !d.pop() {
                return false;
            }
        }
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        // `svn info --show-item wc-root` (svn >= 1.9)
        run_cmd(dir, "svn", &["info", "--show-item", "wc-root"])
            .ok()
            .map(|s| PathBuf::from(s.trim()))
            .or_else(|| {
                // Fallback: walk up as long as .svn exists
                let mut d = dir.to_path_buf();
                let mut root = None;
                loop {
                    if d.join(".svn").is_dir() {
                        root = Some(d.clone());
                    } else {
                        break;
                    }
                    if !d.pop() {
                        break;
                    }
                }
                root
            })
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        // Extract branch from URL (last path component)
        let info = run_cmd(root, "svn", &["info"]).unwrap_or_default();
        let branch = info
            .lines()
            .find(|l| l.starts_with("Relative URL:"))
            .and_then(|l| l.split('/').next_back().map(|s| s.to_string()))
            .unwrap_or_else(|| "trunk".into());

        let raw = run_cmd(root, "svn", &["status"])?;
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for line in raw.lines() {
            if line.is_empty() {
                continue;
            }
            let code = line.chars().next().unwrap_or(' ');
            // svn status: first 8 chars are flags, then filename
            let path = line.get(8..).unwrap_or("").trim().to_string();
            if path.is_empty() {
                continue;
            }
            if code == '?' {
                untracked.push(path);
            } else {
                unstaged.push(FileStatus { code, path });
            }
        }

        Ok(VcStatus {
            backend_name: "svn".into(),
            branch,
            has_staging: false,
            staged: vec![],
            unstaged,
            untracked,
            conflicts: vec![],
        })
    }

    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String> {
        let mut args = vec!["diff"];
        if let Some(f) = file {
            args.push(f);
        }
        run_cmd(root, "svn", &args)
    }

    fn log(&self, root: &Path, limit: usize) -> Result<String, String> {
        let n = limit.to_string();
        run_cmd(root, "svn", &["log", "--limit", &n])
    }

    fn stage(&self, _root: &Path, _file: &str) -> Result<(), String> {
        Err("svn commits go directly to the server; there is no staging area".into())
    }

    fn unstage(&self, _root: &Path, _file: &str) -> Result<(), String> {
        Err("svn has no staging area".into())
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String> {
        run_cmd(root, "svn", &["commit", "-m", message]).map(|_| ())
    }

    fn push(&self, _root: &Path) -> Result<String, String> {
        Ok("svn commits are sent to the server immediately; no separate push step".into())
    }

    fn blame(&self, root: &Path, file: &str) -> Result<String, String> {
        run_cmd(root, "svn", &["blame", file])
    }
}
