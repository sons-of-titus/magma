//! Output parsers shared by the CLI backends.

use crate::{BranchInfo, FileStatus, StashEntry};

/// Parse `git status --porcelain -z` output: NUL-separated records,
/// paths never C-quoted, and rename/copy records carry the original path
/// as the NUL field after the destination — which is the path `git add`
/// and friends want.
///
/// Unmerged pairs (UU, AA, DD, AU/UA, DU/UD) land in `conflicts` — a
/// conflicted path is one problem, not a staged row plus an unstaged row.
pub fn parse_porcelain_z(raw: &str)
    -> (Vec<FileStatus>, Vec<FileStatus>, Vec<String>, Vec<String>)
{
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    let mut conflicts = Vec::new();
    let mut fields = raw.split('\0');
    while let Some(rec) = fields.next() {
        if rec.len() < 3 { continue; }
        let mut chars = rec.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let path = rec[3..].to_string();
        if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
            let _original = fields.next();
        }
        match (x, y) {
            ('?', '?') => untracked.push(path),
            ('U', 'U') | ('A', 'A') | ('D', 'D')
            | ('A', 'U') | ('U', 'A') | ('D', 'U') | ('U', 'D') => conflicts.push(path),
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
    (staged, unstaged, untracked, conflicts)
}

/// Parse `git branch --list` lines: `* current`, `  other`.
pub fn parse_branch_lines(raw: &str) -> Vec<BranchInfo> {
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| BranchInfo {
            current: l.starts_with('*'),
            name: l.trim_start_matches(['*', ' ']).trim().to_string(),
        })
        .collect()
}

/// Parse `git stash list` records `stash@{N}: msg on branch: detail`.
pub fn parse_stash_lines(raw: &str) -> Vec<StashEntry> {
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let rest = l.strip_prefix("stash@{")?;
            let close = rest.find('}')?;
            let index: usize = rest[..close].parse().ok()?;
            let message = rest[close + 1..].trim_start_matches(':').trim().to_string();
            Some(StashEntry { index, message })
        })
        .collect()
}

/// Split a unified diff into single-hunk patches, each carrying the full
/// file header so `git apply --cached` accepts it verbatim.
pub fn split_hunks(diff: &str) -> Vec<String> {
    let mut header = String::new();
    let mut hunks: Vec<Vec<&str>> = Vec::new();
    for line in diff.lines() {
        if line.starts_with("@@") {
            hunks.push(vec![line]);
        } else if hunks.is_empty() {
            header.push_str(line);
            header.push('\n');
        } else {
            hunks.last_mut().unwrap().push(line);
        }
    }
    hunks.into_iter()
        // The trailing newline is load-bearing: a patch whose final line
        // lacks one is "corrupt" to `git apply`.
        .map(|body| format!("{header}{}\n", body.join("\n")))
        .collect()
}
