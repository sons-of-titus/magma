//! Backend whose operations are shell-command templates registered at runtime.

use crate::{BlameLine, FileStatus, VcBackend, VcStatus};
use std::path::{Path, PathBuf};

/// A backend whose operations are specified as shell-command template strings.
/// Placeholders: `{dir}`, `{file}`, `{msg}`, `{limit}`.  Substituted values
/// are single-quoted by the renderer — templates must not add their own
/// quotes around placeholders.
pub struct ShellTemplateBackend {
    pub name: String,
    /// Filename or directory name whose presence (walking ancestors) signals
    /// this VCS. E.g. `".fslckout"` for Fossil.
    pub detect_marker: String,
    pub root_cmd: Option<String>,
    pub status_cmd: Option<String>,
    pub diff_cmd: Option<String>,
    pub log_cmd: Option<String>,
    pub stage_cmd: Option<String>,
    pub unstage_cmd: Option<String>,
    pub commit_cmd: Option<String>,
    pub push_cmd: Option<String>,
    pub blame_cmd: Option<String>,
    pub has_staging: bool,
}

/// POSIX single-quote a value so it can never become executable syntax
/// inside a rendered template line.
fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

impl ShellTemplateBackend {
    fn render(
        &self,
        tmpl: &str,
        dir: &Path,
        file: Option<&str>,
        msg: Option<&str>,
        limit: Option<usize>,
    ) -> String {
        // Values are single-quoted into the shell line: a filename or
        // commit message containing shell metacharacters must never become
        // executable syntax.  Template authors keep their own quoting.
        let dir_s = shell_quote(&dir.to_string_lossy());
        let mut s = tmpl.replace("{dir}", &dir_s);
        s = s.replace("{file}", &file.map(shell_quote).unwrap_or_default());
        s = s.replace("{msg}", &msg.map(shell_quote).unwrap_or_default());
        s = s.replace("{limit}", &limit.unwrap_or(20).to_string());
        s
    }

    fn run_tmpl(
        &self,
        tmpl: &str,
        dir: &Path,
        file: Option<&str>,
        msg: Option<&str>,
        limit: Option<usize>,
    ) -> Result<String, String> {
        let cmd = self.render(tmpl, dir, file, msg, limit);
        let out = std::process::Command::new("sh")
            .args(["-c", &cmd])
            .current_dir(dir)
            .output()
            .map_err(|e| format!("sh: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if out.status.success() || stderr.is_empty() {
            Ok(stdout)
        } else {
            Err(stderr)
        }
    }
}

impl VcBackend for ShellTemplateBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn detect(&self, dir: &Path) -> bool {
        let marker = &self.detect_marker;
        let mut d = dir.to_path_buf();
        loop {
            if d.join(marker).exists() {
                return true;
            }
            if !d.pop() {
                return false;
            }
        }
    }

    fn root(&self, dir: &Path) -> Option<PathBuf> {
        let tmpl = self.root_cmd.as_deref()?;
        self.run_tmpl(tmpl, dir, None, None, None)
            .ok()
            .map(|s| PathBuf::from(s.trim()))
    }

    fn status(&self, root: &Path) -> Result<VcStatus, String> {
        let raw = self
            .status_cmd
            .as_ref()
            .ok_or_else(|| format!("{}: no status-cmd", self.name))?;
        let out = self.run_tmpl(raw, root, None, None, None)?;
        // Default parse: "X path" lines
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();
        for line in out.lines() {
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
            backend_name: self.name.clone(),
            branch: String::new(),
            has_staging: self.has_staging,
            staged: vec![],
            unstaged,
            untracked,
            conflicts: vec![],
        })
    }

    fn diff(&self, root: &Path, file: Option<&str>) -> Result<String, String> {
        let tmpl = self
            .diff_cmd
            .as_deref()
            .ok_or_else(|| format!("{}: no diff-cmd", self.name))?;
        self.run_tmpl(tmpl, root, file, None, None)
    }

    fn log(&self, root: &Path, limit: usize) -> Result<String, String> {
        let tmpl = self
            .log_cmd
            .as_deref()
            .ok_or_else(|| format!("{}: no log-cmd", self.name))?;
        self.run_tmpl(tmpl, root, None, None, Some(limit))
    }

    fn stage(&self, root: &Path, file: &str) -> Result<(), String> {
        let tmpl = self
            .stage_cmd
            .as_deref()
            .ok_or_else(|| format!("{}: staging not supported", self.name))?;
        self.run_tmpl(tmpl, root, Some(file), None, None)
            .map(|_| ())
    }

    fn unstage(&self, root: &Path, file: &str) -> Result<(), String> {
        let tmpl = self
            .unstage_cmd
            .as_deref()
            .ok_or_else(|| format!("{}: unstaging not supported", self.name))?;
        self.run_tmpl(tmpl, root, Some(file), None, None)
            .map(|_| ())
    }

    fn commit(&self, root: &Path, message: &str) -> Result<(), String> {
        let tmpl = self
            .commit_cmd
            .as_deref()
            .ok_or_else(|| format!("{}: no commit-cmd", self.name))?;
        self.run_tmpl(tmpl, root, None, Some(message), None)
            .map(|_| ())
    }

    fn push(&self, root: &Path) -> Result<String, String> {
        let tmpl = self
            .push_cmd
            .as_deref()
            .ok_or_else(|| format!("{}: no push-cmd", self.name))?;
        self.run_tmpl(tmpl, root, None, None, None)
    }

    fn blame(&self, root: &Path, file: &str) -> Result<Vec<BlameLine>, String> {
        // A configured template backend owns its own format: each output row
        // is taken as one line's attribution verbatim (first two whitespace
        // tokens: commit, author).
        let tmpl = self
            .blame_cmd
            .as_deref()
            .ok_or_else(|| format!("{}: no blame-cmd", self.name))?;
        let raw = self.run_tmpl(tmpl, root, Some(file), None, None)?;
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
