//! Tool output → diagnostics.  Parses build/test runs into typed
//! diagnostics + quickfix entries so a failing build becomes clickable
//! errors in the editor instead of raw text in a buffer.
//!
//! Patterns, in order of specificity:
//! - rustc/cargo spans: `error[..]: msg` / `warning: msg` followed by
//!   ` --> path:line:col` (the primary span).
//! - panics: `panicked at path:line:col` (cargo test failures).
//! - generic `path:line[:col]: message` (make, pytest tracebacks, rg
//!   --vimgrep, most compilers).
//!
//! The crate is pure: output text in, findings out.  Applying them to
//! buffers and the quickfix list is the editor's job.

#[cfg(test)]
mod tests;

use semantic::{Diagnostic, DiagnosticSeverity};
use std::borrow::Cow;
use std::path::Path;

/// One parsed finding: the (resolved) file plus the typed diagnostic.
#[derive(Debug, Clone)]
pub struct ToolDiag {
    pub path: String,
    pub diag: Diagnostic,
}

impl ToolDiag {
    pub fn quickfix(&self) -> QuickfixEntry {
        QuickfixEntry {
            filename: self.path.clone(),
            line: self.diag.line + 1,
            col: self.diag.column + 1,
            message: self.diag.message.clone(),
            severity: self.diag.severity,
        }
    }
}

/// A parsed quickfix entry from compiler error output.
#[derive(Debug, Clone)]
pub struct QuickfixEntry {
    pub filename: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

impl Default for QuickfixEntry {
    fn default() -> Self {
        QuickfixEntry {
            filename: String::new(),
            line: 0,
            col: 0,
            message: String::new(),
            severity: DiagnosticSeverity::Error,
        }
    }
}

/// Strip terminal escape sequences from `line`.  Forced color
/// (`CARGO_TERM_COLOR=always`, `--color=always`) wraps compiler output in
/// SGR codes that would defeat the prefix matching in the parser and in
/// clickable-link detection.
pub fn strip_ansi(line: &str) -> Cow<'_, str> {
    if !line.contains('\u{1b}') {
        return Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // CSI: ESC [ params intermediates final-byte (@ through ~).
            Some('[') => {
                chars.next();
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            // Any other escape: drop its introducer's second byte with it.
            _ => {
                chars.next();
            }
        }
    }
    Cow::Owned(out)
}

/// Parse combined tool output into findings, resolving relative paths
/// against `cwd`.  `command` selects nothing today — every pattern runs —
/// but documents where the output came from and leaves room for
/// tool-specific modes.
pub fn parse_tool_output(_command: &str, text: &str, cwd: &Path) -> Vec<ToolDiag> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    let mut pending: Option<(DiagnosticSeverity, String)> = None;
    for line in &lines {
        let clean = strip_ansi(line);
        let trimmed = clean.trim();
        // rustc/cargo message header — the span follows on the ` -->` line.
        if let Some(msg) = trimmed.strip_prefix("error") {
            let msg = msg
                .strip_prefix('[')
                .and_then(|r| r.find(']').map(|i| &r[i + 1..]))
                .unwrap_or(msg);
            if let Some(msg) = msg.strip_prefix(": ") {
                pending = Some((DiagnosticSeverity::Error, msg.trim().to_string()));
                continue;
            }
        }
        if let Some(msg) = trimmed.strip_prefix("warning: ") {
            pending = Some((DiagnosticSeverity::Warning, msg.trim().to_string()));
            continue;
        }
        if let Some(span) = trimmed.strip_prefix("--> ") {
            if let Some((severity, message)) = pending.take()
                && let Some(finding) = span_finding(span, severity, message, cwd)
            {
                out.push(finding);
            }
            continue;
        }
        pending = None;

        // Panics: `thread 'x' panicked at src/lib.rs:8:5:` (cargo test)
        // or `panicked at 'msg', src/lib.rs:8:5`.  The location trails the
        // marker; anything after the last comma is the older message form.
        if let Some(at) = trimmed.find("panicked at") {
            let rest = trimmed[at + "panicked at".len()..].trim();
            let loc = rest.rsplit(", ").next().unwrap_or(rest);
            let loc = loc.trim().trim_end_matches(':');
            if let Some(finding) = colon_finding(
                loc,
                DiagnosticSeverity::Error,
                "test panicked".to_string(),
                cwd,
                3,
            ) {
                out.push(finding);
            }
            continue;
        }

        // Generic `path:line[:col]: message`.
        if trimmed.contains(' ')
            && let Some(finding) = generic_line_finding(trimmed, cwd)
        {
            out.push(finding);
        }
    }
    out
}

/// `path:line:col` (rustc span) with its message.
fn span_finding(
    span: &str,
    severity: DiagnosticSeverity,
    message: String,
    cwd: &Path,
) -> Option<ToolDiag> {
    colon_finding(span.trim(), severity, message, cwd, 2)
}

/// Parse `path:line[:col]`; `max_parts` limits how many colon-separated
/// numeric suffixes are required/accepted.
fn colon_finding(
    loc: &str,
    severity: DiagnosticSeverity,
    message: String,
    cwd: &Path,
    _max_parts: usize,
) -> Option<ToolDiag> {
    let (path, line, col) = split_loc(loc)?;
    Some(ToolDiag {
        path: resolve(&path, cwd),
        diag: Diagnostic {
            message,
            severity,
            line: line.saturating_sub(1),
            column: col.unwrap_or(1).saturating_sub(1),
            end_line: line.saturating_sub(1),
            end_column: col.unwrap_or(1).saturating_sub(1),
        },
    })
}

/// `path:line:col: message` on one line (make/pytest/rg shapes).
fn generic_line_finding(line: &str, cwd: &Path) -> Option<ToolDiag> {
    // The path must contain a path separator or a dot — otherwise plain
    // prose with colons ("note: this: that") would match.
    let head = line.split_once(' ')?.0;
    if !head.contains('/') && !head.contains('.') {
        return None;
    }
    let (path, line_no, col) = split_loc(head)?;
    let message = line
        .split_once(' ')
        .map(|(_, m)| m.trim())
        .unwrap_or_default();
    if message.is_empty() {
        return None;
    }
    Some(ToolDiag {
        path: resolve(&path, cwd),
        diag: Diagnostic {
            message: message.to_string(),
            severity: DiagnosticSeverity::Error,
            line: line_no.saturating_sub(1),
            column: col.unwrap_or(1).saturating_sub(1),
            end_line: line_no.saturating_sub(1),
            end_column: col.unwrap_or(1).saturating_sub(1),
        },
    })
}

/// `path:line[:col]` → (path, line, col).  Line must be numeric.
fn split_loc(loc: &str) -> Option<(String, usize, Option<usize>)> {
    // A trailing separator (from splitting the line on the first space)
    // would shift every field: drop one before splitting.
    let loc = loc.strip_suffix(':').unwrap_or(loc);
    let segs: Vec<&str> = loc.split(':').collect();
    let mut idx = segs.len();
    // The last segment must be numeric (the column when two numerics
    // trail, the line when only one does); everything before is the path,
    // keeping its own colons (Windows drive letters, URLs).
    let last = segs.get(idx - 1)?.trim();
    let a: usize = last.parse().ok()?;
    idx -= 1;
    let (line, col) = match segs.get(idx - 1).map(|s| s.trim().parse::<usize>()) {
        Some(Ok(b)) => {
            idx -= 1;
            (b, Some(a))
        }
        _ => (a, None),
    };
    let path = segs[..idx].join(":");
    if path.is_empty() {
        return None;
    }
    Some((path, line, col))
}

fn resolve(path: &str, cwd: &Path) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_string_lossy().into_owned()
    } else {
        cwd.join(p).to_string_lossy().into_owned()
    }
}
