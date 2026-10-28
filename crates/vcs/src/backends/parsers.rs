//! Output parsers shared by the CLI backends.

use std::collections::HashMap;

use crate::{BlameLine, BranchInfo, FileStatus, StashEntry};

/// Parse `git status --porcelain -z` output: NUL-separated records,
/// paths never C-quoted, and rename/copy records carry the original path
/// as the NUL field after the destination — which is the path `git add`
/// and friends want.
///
/// Unmerged pairs (UU, AA, DD, AU/UA, DU/UD) land in `conflicts` — a
/// conflicted path is one problem, not a staged row plus an unstaged row.
pub fn parse_porcelain_z(
    raw: &str,
) -> (Vec<FileStatus>, Vec<FileStatus>, Vec<String>, Vec<String>) {
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    let mut conflicts = Vec::new();
    let mut fields = raw.split('\0');
    while let Some(rec) = fields.next() {
        if rec.len() < 3 {
            continue;
        }
        let mut chars = rec.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let path = rec[3..].to_string();
        if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
            let _original = fields.next();
        }
        match (x, y) {
            ('?', '?') => untracked.push(path),
            ('U', 'U')
            | ('A', 'A')
            | ('D', 'D')
            | ('A', 'U')
            | ('U', 'A')
            | ('D', 'U')
            | ('U', 'D') => conflicts.push(path),
            _ => {
                if x != ' ' && x != '?' {
                    staged.push(FileStatus {
                        code: x,
                        path: path.clone(),
                    });
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
    hunks
        .into_iter()
        // The trailing newline is load-bearing: a patch whose final line
        // lacks one is "corrupt" to `git apply`.
        .map(|body| format!("{header}{}\n", body.join("\n")))
        .collect()
}

/// Parse `git blame --porcelain` into per-line blame.
///
/// A block opens with `<sha> <orig> <final>[ <count>]`; the commit's
/// metadata lines (`author`, `author-time`, …) follow only on its first
/// block, so metadata is cached by sha and reused for later blocks.  Each
/// tab-prefixed content line closes one file line's record.
pub fn parse_porcelain_blame(raw: &str) -> Vec<BlameLine> {
    let mut lines: Vec<BlameLine> = Vec::new();
    let mut cache: HashMap<String, (String, i64)> = HashMap::new();
    let mut sha = String::new();
    let mut author = String::new();
    let mut time: i64 = 0;
    for l in raw.lines() {
        if l.starts_with('\t') {
            let (a, d) = match cache.get(&sha) {
                Some((a, t)) => (a.clone(), short_date(*t)),
                None => {
                    cache.insert(sha.clone(), (author.clone(), time));
                    (author.clone(), short_date(time))
                }
            };
            lines.push(BlameLine {
                commit: sha.chars().take(8).collect(),
                author: a,
                date: d,
            });
            continue;
        }
        let is_header = {
            let mut parts = l.split(' ');
            match (parts.next(), parts.next()) {
                (Some(h), Some(_)) => {
                    h.len() >= 7
                        && h.chars().all(|c| c.is_ascii_hexdigit())
                        && !l.starts_with("previous ")
                }
                _ => false,
            }
        };
        if is_header {
            sha = l.split(' ').next().unwrap_or("").to_string();
            match cache.get(&sha) {
                Some((a, t)) => {
                    author = a.clone();
                    time = *t;
                }
                None => {
                    author.clear();
                    time = 0;
                }
            }
            continue;
        }
        if let Some(v) = l.strip_prefix("author ") {
            author = v.to_string();
        } else if let Some(v) = l.strip_prefix("author-time ") {
            time = v.parse().unwrap_or(0);
        }
    }
    lines
}

/// Unix seconds → `YYYY-MM-DD` (UTC).  Civil-from-days is Howard Hinnant's
/// algorithm; no time dependency for one formatted field.
pub fn short_date(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
