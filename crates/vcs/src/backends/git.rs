//! Git backend.

use super::parsers::{
    parse_branch_lines, parse_porcelain_blame, parse_porcelain_z, parse_stash_lines,
};
use crate::{BlameLine, BranchInfo, StashEntry, VcBackend, VcStatus, run_cmd, run_cmd_stdin};
use std::path::{Path, PathBuf};

pub struct GitBackend;

impl VcBackend for GitBackend {
    fn name(&self) -> &str {
        "git"
    }

    fn detect(&self, dir: &Path) -> bool {
        std::process::Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        run_cmd(dir, "git", &["rev-parse", "--show-toplevel"])
            .ok()
            .map(|s| PathBuf::from(s.trim()))
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        let branch = run_cmd(root, "git", &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_else(|_| "HEAD".into())
            .trim()
            .to_string();

        let raw = run_cmd(root, "git", &["status", "--porcelain", "-z"])?;
        let (staged, unstaged, untracked, conflicts) = parse_porcelain_z(&raw);

        Ok(VcStatus {
            backend_name: "git".into(),
            branch,
            has_staging: true,
            staged,
            unstaged,
            untracked,
            conflicts,
        })
    }

    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String> {
        let mut args = vec!["diff", "HEAD"];
        if let Some(f) = file {
            args.extend_from_slice(&["--", f]);
        }
        run_cmd(root, "git", &args)
    }

    fn log(&self, root: &Path, limit: usize) -> Result<String, String> {
        let n = limit.to_string();
        run_cmd(
            root,
            "git",
            &[
                "log",
                "--oneline",
                "--graph",
                "--decorate",
                &format!("-{n}"),
            ],
        )
    }

    fn stage(&self, root: &Path, file: &str) -> Result<(), String> {
        run_cmd(root, "git", &["add", "--", file]).map(|_| ())
    }

    fn unstage(&self, root: &Path, file: &str) -> Result<(), String> {
        run_cmd(root, "git", &["restore", "--staged", "--", file]).map(|_| ())
    }

    fn stage_all(&self, root: &Path) -> Result<(), String> {
        run_cmd(root, "git", &["add", "-A"]).map(|_| ())
    }

    fn unstage_all(&self, root: &Path) -> Result<(), String> {
        run_cmd(root, "git", &["reset", "--quiet"]).map(|_| ())
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String> {
        run_cmd(root, "git", &["commit", "-m", message]).map(|_| ())
    }

    fn push(&self, root: &Path) -> Result<String, String> {
        run_cmd(root, "git", &["push"])
    }

    fn blame(&self, root: &Path, file: &str) -> Result<Vec<BlameLine>, String> {
        // Porcelain only: the default format's metadata is prose (author
        // names contain spaces and parens) and cannot be split reliably.
        let raw = run_cmd(root, "git", &["blame", "--porcelain", "--", file])?;
        Ok(parse_porcelain_blame(&raw))
    }

    fn list_branches(&self, root: &Path) -> Result<Vec<BranchInfo>, String> {
        let raw = run_cmd(root, "git", &["branch", "--list"])?;
        Ok(parse_branch_lines(&raw))
    }

    fn checkout(&self, root: &Path, branch: &str, create: bool) -> Result<(), String> {
        if create {
            run_cmd(root, "git", &["checkout", "-b", branch]).map(|_| ())
        } else {
            run_cmd(root, "git", &["checkout", branch]).map(|_| ())
        }
    }

    fn stage_patch(&self, root: &Path, patch: &str) -> Result<(), String> {
        // Zero-context patches (`-U0` hunks) need --unidiff-zero.
        run_cmd_stdin(
            root,
            "git",
            &["apply", "--cached", "--unidiff-zero", "--whitespace=nowarn"],
            patch,
        )
        .map(|_| ())
    }

    fn discard_patch(&self, root: &Path, patch: &str) -> Result<(), String> {
        run_cmd_stdin(
            root,
            "git",
            &["apply", "-R", "--unidiff-zero", "--whitespace=nowarn"],
            patch,
        )
        .map(|_| ())
    }

    fn hunk_diff(&self, root: &Path, file: &str) -> Result<String, String> {
        run_cmd(root, "git", &["diff", "-U0", "--", file])
    }

    fn amend(&self, root: &Path, message: Option<&str>) -> Result<(), String> {
        match message {
            Some(m) => run_cmd(root, "git", &["commit", "--amend", "-m", m]).map(|_| ()),
            None => run_cmd(root, "git", &["commit", "--amend", "--no-edit"]).map(|_| ()),
        }
    }

    fn stash_list(&self, root: &Path) -> Result<Vec<StashEntry>, String> {
        let raw = run_cmd(root, "git", &["stash", "list"])?;
        Ok(parse_stash_lines(&raw))
    }

    fn stash_push(&self, root: &Path, message: Option<&str>) -> Result<(), String> {
        match message {
            Some(m) => run_cmd(root, "git", &["stash", "push", "-m", m]).map(|_| ()),
            None => run_cmd(root, "git", &["stash"]).map(|_| ()),
        }
    }

    fn stash_apply(&self, root: &Path, index: usize, pop: bool) -> Result<(), String> {
        let spec = format!("stash@{{{index}}}");
        if pop {
            run_cmd(root, "git", &["stash", "pop", &spec]).map(|_| ())
        } else {
            run_cmd(root, "git", &["stash", "apply", &spec]).map(|_| ())
        }
    }

    fn stash_drop(&self, root: &Path, index: usize) -> Result<(), String> {
        let spec = format!("stash@{{{index}}}");
        run_cmd(root, "git", &["stash", "drop", &spec]).map(|_| ())
    }

    fn cat_file(&self, root: &Path, rev: &str, path: &str) -> Result<String, String> {
        // `rev` is `HEAD` or a merge stage shorthand (`:2`, `:3`); both form a
        // valid `<rev>:<path>` object name.
        run_cmd(root, "git", &["show", &format!("{rev}:{path}")])
    }

    fn resolve_ours(&self, root: &Path, path: &str) -> Result<(), String> {
        run_cmd(root, "git", &["checkout", "--ours", "--", path])?;
        run_cmd(root, "git", &["add", "--", path]).map(|_| ())
    }

    fn resolve_theirs(&self, root: &Path, path: &str) -> Result<(), String> {
        run_cmd(root, "git", &["checkout", "--theirs", "--", path])?;
        run_cmd(root, "git", &["add", "--", path]).map(|_| ())
    }

    fn fetch(&self, root: &Path) -> Result<String, String> {
        run_cmd(root, "git", &["fetch", "--all", "--prune"])
    }

    fn pull(&self, root: &Path) -> Result<String, String> {
        run_cmd(root, "git", &["pull"])
    }
}
