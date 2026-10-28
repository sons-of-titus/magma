//! .gitignore matching for the project walk — the common subset:
//! comments, blanks, `*`/`?` within a segment, `**` across segments,
//! trailing-`/` directory-only rules, leading-`/` (or any internal `/`)
//! anchoring to the rule file's directory, and `!` negation where later
//! rules win.  Character classes and other exotic syntax are not
//! supported — files matching only those still index (fail-open, like
//! the previous behavior for everything).

use std::path::{Path, PathBuf};

struct Rule {
    negated: bool,
    dir_only: bool,
    anchored: bool,
    segments: Vec<String>,
}

/// One directory's parsed `.gitignore`.
pub struct Gitignore {
    /// Directory containing this ignore file; its rules anchor here.
    dir: PathBuf,
    rules: Vec<Rule>,
}

pub fn parse_file(path: &Path) -> Option<Gitignore> {
    let dir = path.parent()?.to_path_buf();
    let text = std::fs::read_to_string(path).ok()?;
    let mut rules = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let negated = line.starts_with('!');
        let line = if negated { &line[1..] } else { line };
        let dir_only = line.ends_with('/');
        let line = line.trim_end_matches('/');
        if line.is_empty() {
            continue;
        }
        // A leading or internal slash anchors the pattern; a bare name
        // matches at any depth.
        let anchored = line.starts_with('/') || line[1..].contains('/');
        let line = line.trim_start_matches('/');
        rules.push(Rule {
            negated,
            dir_only,
            anchored,
            segments: line.split('/').map(str::to_string).collect(),
        });
    }
    Some(Gitignore { dir, rules })
}

impl Gitignore {
    /// `None` = no rule in this file matches; `Some(true/false)` = the
    /// last matching rule's verdict.  `path` is absolute.  A rule that
    /// matches any ancestor directory also matches the entry (git
    /// semantics: ignoring a directory excludes its contents).
    fn verdict(&self, path: &Path, is_dir: bool) -> Option<bool> {
        let rel = path.strip_prefix(&self.dir).ok()?;
        let segments: Vec<String> = rel
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let mut verdict = None;
        for rule in &self.rules {
            let hit = (1..=segments.len()).any(|end| {
                // Ancestors are directories; the entry itself carries the
                // caller's kind.
                let as_dir = end < segments.len() || is_dir;
                if rule.dir_only && !as_dir {
                    return false;
                }
                let prefix = &segments[..end];
                if rule.anchored {
                    segments_match(&rule.segments, prefix)
                } else {
                    (0..=prefix.len()).any(|start| segments_match(&rule.segments, &prefix[start..]))
                }
            });
            if hit {
                verdict = Some(!rule.negated);
            }
        }
        verdict
    }
}

/// The walk's collected ignore files, root-first; deeper files are
/// consulted last so they win.
#[derive(Default)]
pub struct IgnoreStack {
    files: Vec<Gitignore>,
}

impl IgnoreStack {
    pub fn push(&mut self, gi: Gitignore) {
        self.files.push(gi);
    }

    pub fn pop(&mut self) {
        self.files.pop();
    }

    /// True when `path` (absolute) is ignored by any stacked file.
    pub fn ignored(&self, path: &Path, is_dir: bool) -> bool {
        self.files
            .iter()
            .rev() // deepest last-pushed wins
            .find_map(|gi| gi.verdict(path, is_dir))
            .unwrap_or(false)
    }
}

/// Pattern segments against path segments, `**` spanning separators.
fn segments_match(pat: &[String], path: &[String]) -> bool {
    match pat.split_first() {
        None => path.is_empty(),
        Some((p, rest)) if p == "**" => {
            // `**` consumes zero or more path segments.
            (0..=path.len()).any(|skip| segments_match(rest, &path[skip..]))
        }
        Some((p, rest)) => path
            .split_first()
            .is_some_and(|(seg, prest)| glob_segment(p, seg) && segments_match(rest, prest)),
    }
}

/// One path segment against one pattern segment: `*` and `?` wildcards.
fn glob_segment(pat: &str, seg: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let s: Vec<char> = seg.chars().collect();
    fn go(p: &[char], s: &[char]) -> bool {
        match p.split_first() {
            None => s.is_empty(),
            Some(('*', rest)) => (0..=s.len()).any(|skip| go(rest, &s[skip..])),
            Some(('?', rest)) => !s.is_empty() && go(rest, &s[1..]),
            Some((c, rest)) => s
                .split_first()
                .is_some_and(|(d, srest)| *c == *d && go(rest, srest)),
        }
    }
    go(&p, &s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gi(dir: &str, body: &str) -> Gitignore {
        let d = PathBuf::from(dir);
        let mut rules = Vec::new();
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let negated = line.starts_with('!');
            let l = if negated { &line[1..] } else { line };
            let dir_only = l.ends_with('/');
            let l = l.trim_end_matches('/');
            let anchored = l.starts_with('/') || l[1..].contains('/');
            let l = l.trim_start_matches('/');
            rules.push(Rule {
                negated,
                dir_only,
                anchored,
                segments: l.split('/').map(str::to_string).collect(),
            });
        }
        Gitignore { dir: d, rules }
    }

    #[test]
    fn bare_name_matches_any_depth() {
        let g = gi("/repo", "*.log\n");
        assert_eq!(g.verdict(Path::new("/repo/a.log"), false), Some(true));
        assert_eq!(
            g.verdict(Path::new("/repo/src/deep/x.log"), false),
            Some(true)
        );
        assert_eq!(g.verdict(Path::new("/repo/src/main.rs"), false), None);
    }

    #[test]
    fn slash_rules_anchor_to_their_directory() {
        let g = gi("/repo", "/top.txt\nsrc/gen\n");
        assert_eq!(g.verdict(Path::new("/repo/top.txt"), false), Some(true));
        assert_eq!(g.verdict(Path::new("/repo/sub/top.txt"), false), None);
        assert_eq!(
            g.verdict(Path::new("/repo/src/gen/file.rs"), false),
            Some(true)
        );
        assert_eq!(g.verdict(Path::new("/repo/other/gen/file.rs"), false), None);
    }

    #[test]
    fn dir_only_rules_skip_files() {
        let g = gi("/repo", "build/\n");
        assert_eq!(g.verdict(Path::new("/repo/build"), true), Some(true));
        assert_eq!(
            g.verdict(Path::new("/repo/build"), false),
            None,
            "a file literally named 'build' is not ignored"
        );
        assert_eq!(g.verdict(Path::new("/repo/a/build/x"), true), Some(true));
    }

    #[test]
    fn negation_restores_later_wins() {
        let g = gi("/repo", "*.log\n!keep.log\n");
        assert_eq!(g.verdict(Path::new("/repo/x.log"), false), Some(true));
        assert_eq!(g.verdict(Path::new("/repo/keep.log"), false), Some(false));
    }

    #[test]
    fn double_star_spans_directories() {
        let g = gi("/repo", "docs/**/generated\n");
        assert_eq!(
            g.verdict(Path::new("/repo/docs/a/b/generated"), true),
            Some(true)
        );
        assert_eq!(
            g.verdict(Path::new("/repo/docs/generated"), true),
            Some(true)
        );
        assert_eq!(g.verdict(Path::new("/repo/docs/a/b/other"), true), None);
    }

    #[test]
    fn stack_deepest_file_wins() {
        let mut stack = IgnoreStack::default();
        stack.push(gi("/repo", "*.tmp\n"));
        stack.push(gi("/repo/src", "!keep.tmp\n"));
        assert!(stack.ignored(Path::new("/repo/a.tmp"), false));
        assert!(!stack.ignored(Path::new("/repo/src/keep.tmp"), false));
        assert!(stack.ignored(Path::new("/repo/src/x.tmp"), false));
    }
}
