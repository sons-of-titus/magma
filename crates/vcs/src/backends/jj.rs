//! Jujutsu backend.

use crate::{BlameLine, FileStatus, VcBackend, VcStatus, run_cmd};
use std::path::{Path, PathBuf};

pub struct JjBackend;

impl VcBackend for JjBackend {
    fn name(&self) -> &str {
        "jj"
    }

    fn detect(&self, dir: &Path) -> bool {
        let mut d = dir.to_path_buf();
        loop {
            if d.join(".jj").is_dir() {
                return true;
            }
            if !d.pop() {
                return false;
            }
        }
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        run_cmd(dir, "jj", &["root"])
            .ok()
            .map(|s| PathBuf::from(s.trim()))
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        // Current bookmark / branch label
        let branch = run_cmd(
            root,
            "jj",
            &[
                "log",
                "--no-graph",
                "--limit",
                "1",
                "--template",
                "if(bookmarks, bookmarks, change_id.short())",
            ],
        )
        .unwrap_or_else(|_| "working copy".into())
        .trim()
        .to_string();

        let raw = run_cmd(root, "jj", &["status"])?;
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        // jj status output lines look like: "M path/to/file"
        for line in raw.lines() {
            if line.len() < 3 {
                continue;
            }
            let code = line.chars().next().unwrap_or(' ');
            let path = line[2..].trim().to_string();
            if code == '?' {
                untracked.push(path);
            } else {
                unstaged.push(FileStatus { code, path });
            }
        }

        Ok(VcStatus {
            backend_name: "jj".into(),
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
            args.extend_from_slice(&["--", f]);
        }
        run_cmd(root, "jj", &args)
    }

    fn log(&self, root: &Path, limit: usize) -> Result<String, String> {
        let n = limit.to_string();
        run_cmd(root, "jj", &["log", "--limit", &n])
    }

    fn stage(&self, _root: &Path, _file: &str) -> Result<(), String> {
        Err("jj tracks all working-copy changes automatically; no staging needed".into())
    }

    fn unstage(&self, _root: &Path, _file: &str) -> Result<(), String> {
        Err("jj has no staging area; use `jj restore` to discard changes".into())
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String> {
        // `jj describe` sets the message on the current change; `jj new` starts the next
        run_cmd(root, "jj", &["describe", "-m", message]).map(|_| ())?;
        run_cmd(root, "jj", &["new"]).map(|_| ())
    }

    fn push(&self, root: &Path) -> Result<String, String> {
        run_cmd(root, "jj", &["git", "push"])
    }

    fn blame(&self, root: &Path, file: &str) -> Result<Vec<BlameLine>, String> {
        // jj's annotate layout varies across releases; the change prefix and
        // author lead every row, so those two tokens are the attribution.
        let raw = run_cmd(root, "jj", &["file", "annotate", "--", file])?;
        Ok(raw
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let mut it = l.split_whitespace();
                BlameLine {
                    commit: it.next().unwrap_or("").to_string(),
                    author: it.next().unwrap_or("").to_string(),
                    date: String::new(),
                }
            })
            .collect())
    }
}
