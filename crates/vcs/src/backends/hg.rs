//! Mercurial backend.

use crate::{BlameLine, FileStatus, VcBackend, VcStatus, run_cmd};
use std::path::{Path, PathBuf};

pub struct HgBackend;

impl VcBackend for HgBackend {
    fn name(&self) -> &str {
        "hg"
    }

    fn detect(&self, dir: &Path) -> bool {
        // Walk ancestors looking for .hg
        let mut d = dir.to_path_buf();
        loop {
            if d.join(".hg").is_dir() {
                return true;
            }
            if !d.pop() {
                return false;
            }
        }
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        run_cmd(dir, "hg", &["root"])
            .ok()
            .map(|s| PathBuf::from(s.trim()))
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        let branch = run_cmd(root, "hg", &["branch"])
            .unwrap_or_else(|_| "default".into())
            .trim()
            .to_string();

        let raw = run_cmd(root, "hg", &["status"])?;
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for line in raw.lines() {
            if line.len() < 3 {
                continue;
            }
            let code = line.chars().next().unwrap_or(' ');
            let path = line[2..].to_string();
            if code == '?' {
                untracked.push(path);
            } else {
                unstaged.push(FileStatus { code, path });
            }
        }

        Ok(VcStatus {
            backend_name: "hg".into(),
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
        run_cmd(root, "hg", &args)
    }

    fn log(&self, root: &Path, limit: usize) -> Result<String, String> {
        let n = limit.to_string();
        run_cmd(
            root,
            "hg",
            &[
                "log",
                "--limit",
                &n,
                "--template",
                "{rev}:{node|short}  {desc|firstline}\n",
            ],
        )
    }

    fn stage(&self, root: &Path, file: &str) -> Result<(), String> {
        // hg add tracks a previously-untracked file
        run_cmd(root, "hg", &["add", "--", file]).map(|_| ())
    }

    fn unstage(&self, _root: &Path, _file: &str) -> Result<(), String> {
        Err("hg has no staging area; use `hg revert` to discard changes".into())
    }

    fn stage_all(&self, root: &Path) -> Result<(), String> {
        // hg has no index; "stage all" tracks previously-untracked files.
        run_cmd(root, "hg", &["add"]).map(|_| ())
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String> {
        run_cmd(root, "hg", &["commit", "-m", message]).map(|_| ())
    }

    fn push(&self, root: &Path) -> Result<String, String> {
        run_cmd(root, "hg", &["push"])
    }

    fn blame(&self, root: &Path, file: &str) -> Result<Vec<BlameLine>, String> {
        // A template gives field-separated records; the default prose format
        // is unparseable (dates and user names contain spaces).
        let raw = run_cmd(
            root,
            "hg",
            &[
                "blame",
                "-T",
                "{author}\t{date|shortdate}\t{rev}\n",
                "--",
                file,
            ],
        )?;
        Ok(raw
            .lines()
            .map(|l| {
                let mut it = l.splitn(3, '\t');
                BlameLine {
                    author: it.next().unwrap_or("").trim().to_string(),
                    date: it.next().unwrap_or("").trim().to_string(),
                    commit: it.next().unwrap_or("").trim().to_string(),
                }
            })
            .collect())
    }
}
