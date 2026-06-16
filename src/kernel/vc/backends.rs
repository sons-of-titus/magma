//! Built-in VCS backends: Git, Mercurial (hg), Subversion (svn), Jujutsu (jj).
//!
//! Each backend is a zero-size struct that shells out to the VCS CLI.
//! All methods receive the repo root directory detected by `VcState::detect_for`.

use std::path::{Path, PathBuf};
use super::{FileStatus, VcBackend, VcStatus, run_cmd};

// ── Git ───────────────────────────────────────────────────────────────────────

pub struct GitBackend;

impl VcBackend for GitBackend {
    fn name(&self) -> &str { "git" }

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
        run_cmd(dir, "git", &["rev-parse", "--show-toplevel"]).ok()
            .map(|s| PathBuf::from(s.trim()))
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        let branch = run_cmd(root, "git", &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_else(|_| "HEAD".into())
            .trim().to_string();

        let raw = run_cmd(root, "git", &["status", "--porcelain"])?;

        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for line in raw.lines() {
            if line.len() < 4 { continue; }
            let x = line.chars().next().unwrap_or(' ');
            let y = line.chars().nth(1).unwrap_or(' ');
            let path = line[3..].to_string();
            match (x, y) {
                ('?', '?') => untracked.push(path),
                _ => {
                    if x != ' ' && x != '?' {
                        staged.push(FileStatus { code: x, path: path.clone() });
                    }
                    if y != ' ' && y != '?' {
                        unstaged.push(FileStatus { code: y, path });
                    }
                }
            }
        }

        Ok(VcStatus { backend_name: "git".into(), branch, has_staging: true, staged, unstaged, untracked })
    }

    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String> {
        let mut args = vec!["diff", "HEAD"];
        if let Some(f) = file { args.extend_from_slice(&["--", f]); }
        run_cmd(root, "git", &args)
    }

    fn log(&self, root: &Path, limit: usize) -> Result<String, String> {
        let n = limit.to_string();
        run_cmd(root, "git", &["log", "--oneline", "--graph", "--decorate", &format!("-{n}")])
    }

    fn stage(&self, root: &Path, file: &str) -> Result<(), String> {
        run_cmd(root, "git", &["add", "--", file]).map(|_| ())
    }

    fn unstage(&self, root: &Path, file: &str) -> Result<(), String> {
        run_cmd(root, "git", &["restore", "--staged", "--", file]).map(|_| ())
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String> {
        run_cmd(root, "git", &["commit", "-m", message]).map(|_| ())
    }

    fn push(&self, root: &Path) -> Result<String, String> {
        run_cmd(root, "git", &["push"])
    }

    fn blame(&self, root: &Path, file: &str) -> Result<String, String> {
        run_cmd(root, "git", &["blame", "--", file])
    }
}

// ── Mercurial ─────────────────────────────────────────────────────────────────

pub struct HgBackend;

impl VcBackend for HgBackend {
    fn name(&self) -> &str { "hg" }

    fn detect(&self, dir: &Path) -> bool {
        // Walk ancestors looking for .hg
        let mut d = dir.to_path_buf();
        loop {
            if d.join(".hg").is_dir() { return true; }
            if !d.pop() { return false; }
        }
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        run_cmd(dir, "hg", &["root"]).ok().map(|s| PathBuf::from(s.trim()))
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        let branch = run_cmd(root, "hg", &["branch"])
            .unwrap_or_else(|_| "default".into())
            .trim().to_string();

        let raw = run_cmd(root, "hg", &["status"])?;
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for line in raw.lines() {
            if line.len() < 3 { continue; }
            let code = line.chars().next().unwrap_or(' ');
            let path = line[2..].to_string();
            if code == '?' { untracked.push(path); }
            else { unstaged.push(FileStatus { code, path }); }
        }

        Ok(VcStatus { backend_name: "hg".into(), branch, has_staging: false,
                      staged: vec![], unstaged, untracked })
    }

    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String> {
        let mut args = vec!["diff"];
        if let Some(f) = file { args.push(f); }
        run_cmd(root, "hg", &args)
    }

    fn log(&self, root: &Path, limit: usize) -> Result<String, String> {
        let n = limit.to_string();
        run_cmd(root, "hg", &["log", "--limit", &n,
            "--template", "{rev}:{node|short}  {desc|firstline}\n"])
    }

    fn stage(&self, root: &Path, file: &str) -> Result<(), String> {
        // hg add tracks a previously-untracked file
        run_cmd(root, "hg", &["add", "--", file]).map(|_| ())
    }

    fn unstage(&self, _root: &Path, _file: &str) -> Result<(), String> {
        Err("hg has no staging area; use `hg revert` to discard changes".into())
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String> {
        run_cmd(root, "hg", &["commit", "-m", message]).map(|_| ())
    }

    fn push(&self, root: &Path) -> Result<String, String> {
        run_cmd(root, "hg", &["push"])
    }

    fn blame(&self, root: &Path, file: &str) -> Result<String, String> {
        run_cmd(root, "hg", &["blame", "-u", "-l", "--", file])
    }
}

// ── Subversion ────────────────────────────────────────────────────────────────

pub struct SvnBackend;

impl VcBackend for SvnBackend {
    fn name(&self) -> &str { "svn" }

    fn detect(&self, dir: &Path) -> bool {
        let mut d = dir.to_path_buf();
        loop {
            if d.join(".svn").is_dir() { return true; }
            if !d.pop() { return false; }
        }
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        // `svn info --show-item wc-root` (svn >= 1.9)
        run_cmd(dir, "svn", &["info", "--show-item", "wc-root"]).ok()
            .map(|s| PathBuf::from(s.trim()))
            .or_else(|| {
                // Fallback: walk up as long as .svn exists
                let mut d = dir.to_path_buf();
                let mut root = None;
                loop {
                    if d.join(".svn").is_dir() { root = Some(d.clone()); }
                    else { break; }
                    if !d.pop() { break; }
                }
                root
            })
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        // Extract branch from URL (last path component)
        let info = run_cmd(root, "svn", &["info"]).unwrap_or_default();
        let branch = info.lines()
            .find(|l| l.starts_with("Relative URL:"))
            .and_then(|l| l.split('/').next_back().map(|s| s.to_string()))
            .unwrap_or_else(|| "trunk".into());

        let raw = run_cmd(root, "svn", &["status"])?;
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for line in raw.lines() {
            if line.is_empty() { continue; }
            let code = line.chars().next().unwrap_or(' ');
            // svn status: first 8 chars are flags, then filename
            let path = line.get(8..).unwrap_or("").trim().to_string();
            if path.is_empty() { continue; }
            if code == '?' { untracked.push(path); }
            else { unstaged.push(FileStatus { code, path }); }
        }

        Ok(VcStatus { backend_name: "svn".into(), branch, has_staging: false,
                      staged: vec![], unstaged, untracked })
    }

    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String> {
        let mut args = vec!["diff"];
        if let Some(f) = file { args.push(f); }
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

// ── Jujutsu ───────────────────────────────────────────────────────────────────

pub struct JjBackend;

impl VcBackend for JjBackend {
    fn name(&self) -> &str { "jj" }

    fn detect(&self, dir: &Path) -> bool {
        let mut d = dir.to_path_buf();
        loop {
            if d.join(".jj").is_dir() { return true; }
            if !d.pop() { return false; }
        }
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        run_cmd(dir, "jj", &["root"]).ok().map(|s| PathBuf::from(s.trim()))
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        // Current bookmark / branch label
        let branch = run_cmd(root, "jj", &[
            "log", "--no-graph", "--limit", "1",
            "--template", "if(bookmarks, bookmarks, change_id.short())",
        ]).unwrap_or_else(|_| "working copy".into())
          .trim().to_string();

        let raw = run_cmd(root, "jj", &["status"])?;
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        // jj status output lines look like: "M path/to/file"
        for line in raw.lines() {
            if line.len() < 3 { continue; }
            let code = line.chars().next().unwrap_or(' ');
            let path = line[2..].trim().to_string();
            if code == '?' { untracked.push(path); }
            else { unstaged.push(FileStatus { code, path }); }
        }

        Ok(VcStatus { backend_name: "jj".into(), branch, has_staging: false,
                      staged: vec![], unstaged, untracked })
    }

    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String> {
        let mut args = vec!["diff"];
        if let Some(f) = file { args.extend_from_slice(&["--", f]); }
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

    fn blame(&self, root: &Path, file: &str) -> Result<String, String> {
        run_cmd(root, "jj", &["file", "annotate", "--", file])
    }
}

// ── Shell-template backend (registered from Janet) ────────────────────────────

/// A backend whose operations are specified as shell-command template strings.
/// Placeholders: `{dir}`, `{file}`, `{msg}`, `{limit}`.
pub struct ShellTemplateBackend {
    pub name: String,
    /// Filename or directory name whose presence (walking ancestors) signals
    /// this VCS. E.g. `".fslckout"` for Fossil.
    pub detect_marker: String,
    pub root_cmd:    Option<String>,
    pub status_cmd:  Option<String>,
    pub diff_cmd:    Option<String>,
    pub log_cmd:     Option<String>,
    pub stage_cmd:   Option<String>,
    pub unstage_cmd: Option<String>,
    pub commit_cmd:  Option<String>,
    pub push_cmd:    Option<String>,
    pub blame_cmd:   Option<String>,
    pub has_staging: bool,
}

impl ShellTemplateBackend {
    fn render(&self, tmpl: &str, dir: &Path, file: Option<&str>, msg: Option<&str>, limit: Option<usize>) -> String {
        let dir_s = dir.to_string_lossy();
        let mut s = tmpl.replace("{dir}", &dir_s);
        s = s.replace("{file}", file.unwrap_or(""));
        s = s.replace("{msg}", msg.unwrap_or(""));
        s = s.replace("{limit}", &limit.unwrap_or(20).to_string());
        s
    }

    fn run_tmpl(&self, tmpl: &str, dir: &Path, file: Option<&str>, msg: Option<&str>, limit: Option<usize>) -> Result<String, String> {
        let cmd = self.render(tmpl, dir, file, msg, limit);
        let out = std::process::Command::new("sh")
            .args(["-c", &cmd])
            .current_dir(dir)
            .output()
            .map_err(|e| format!("sh: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if out.status.success() || stderr.is_empty() { Ok(stdout) } else { Err(stderr) }
    }
}

impl VcBackend for ShellTemplateBackend {
    fn name(&self) -> &str { &self.name }

    fn detect(&self, dir: &Path) -> bool {
        let marker = &self.detect_marker;
        let mut d = dir.to_path_buf();
        loop {
            if d.join(marker).exists() { return true; }
            if !d.pop() { return false; }
        }
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        let tmpl = self.root_cmd.as_deref()?;
        self.run_tmpl(tmpl, dir, None, None, None).ok()
            .map(|s| PathBuf::from(s.trim()))
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        let raw = self.status_cmd.as_ref()
            .ok_or_else(|| format!("{}: no status-cmd", self.name))?;
        let out = self.run_tmpl(raw, root, None, None, None)?;
        // Default parse: "X path" lines
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();
        for line in out.lines() {
            if line.len() < 3 { continue; }
            let code = line.chars().next().unwrap_or(' ');
            let path = line[2..].trim().to_string();
            if code == '?' { untracked.push(path); } else { unstaged.push(FileStatus { code, path }); }
        }
        Ok(VcStatus {
            backend_name: self.name.clone(),
            branch: String::new(),
            has_staging: self.has_staging,
            staged: vec![], unstaged, untracked,
        })
    }

    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String> {
        let tmpl = self.diff_cmd.as_deref()
            .ok_or_else(|| format!("{}: no diff-cmd", self.name))?;
        self.run_tmpl(tmpl, root, file, None, None)
    }

    fn log(&self, root: &Path, limit: usize) -> Result<String, String> {
        let tmpl = self.log_cmd.as_deref()
            .ok_or_else(|| format!("{}: no log-cmd", self.name))?;
        self.run_tmpl(tmpl, root, None, None, Some(limit))
    }

    fn stage(&self, root: &Path, file: &str) -> Result<(), String> {
        let tmpl = self.stage_cmd.as_deref()
            .ok_or_else(|| format!("{}: staging not supported", self.name))?;
        self.run_tmpl(tmpl, root, Some(file), None, None).map(|_| ())
    }

    fn unstage(&self, root: &Path, file: &str) -> Result<(), String> {
        let tmpl = self.unstage_cmd.as_deref()
            .ok_or_else(|| format!("{}: unstaging not supported", self.name))?;
        self.run_tmpl(tmpl, root, Some(file), None, None).map(|_| ())
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String> {
        let tmpl = self.commit_cmd.as_deref()
            .ok_or_else(|| format!("{}: no commit-cmd", self.name))?;
        self.run_tmpl(tmpl, root, None, Some(message), None).map(|_| ())
    }

    fn push(&self, root: &Path) -> Result<String, String> {
        let tmpl = self.push_cmd.as_deref()
            .ok_or_else(|| format!("{}: no push-cmd", self.name))?;
        self.run_tmpl(tmpl, root, None, None, None)
    }

    fn blame(&self, root: &Path, file: &str) -> Result<String, String> {
        let tmpl = self.blame_cmd.as_deref()
            .ok_or_else(|| format!("{}: no blame-cmd", self.name))?;
        self.run_tmpl(tmpl, root, Some(file), None, None)
    }
}
