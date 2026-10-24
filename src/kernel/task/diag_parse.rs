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

use crate::kernel::semantic::provider::{Diagnostic, DiagnosticSeverity};
use crate::kernel::task::QuickfixEntry;
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

/// Parse combined tool output into findings, resolving relative paths
/// against `cwd`.  `command` selects nothing today — every pattern runs —
/// but documents where the output came from and leaves room for
/// tool-specific modes.
pub fn parse_tool_output(_command: &str, text: &str, cwd: &Path) -> Vec<ToolDiag> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    let mut pending: Option<(DiagnosticSeverity, String)> = None;
    for line in &lines {
        let trimmed = line.trim();
        // rustc/cargo message header — the span follows on the ` -->` line.
        if let Some(msg) = trimmed.strip_prefix("error") {
            let msg = msg.strip_prefix('[')
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
                && let Some(finding) = span_finding(span, severity, message, cwd) {
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
            if let Some(finding) = colon_finding(loc, DiagnosticSeverity::Error,
                    "test panicked".to_string(), cwd, 3) {
                out.push(finding);
            }
            continue;
        }

        // Generic `path:line[:col]: message`.
        if trimmed.contains(' ')
            && let Some(finding) = generic_line_finding(trimmed, cwd) {
                out.push(finding);
            }
    }
    out
}

/// `path:line:col` (rustc span) with its message.
fn span_finding(span: &str, severity: DiagnosticSeverity, message: String, cwd: &Path) -> Option<ToolDiag> {
    colon_finding(span.trim(), severity, message, cwd, 2)
}

/// Parse `path:line[:col]`; `max_parts` limits how many colon-separated
/// numeric suffixes are required/accepted.
fn colon_finding(loc: &str, severity: DiagnosticSeverity, message: String, cwd: &Path, _max_parts: usize) -> Option<ToolDiag> {
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
    let message = line.split_once(' ').map(|(_, m)| m.trim()).unwrap_or_default();
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
        Some(Ok(b)) => { idx -= 1; (b, Some(a)) }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cwd() -> &'static Path {
        Path::new("/repo")
    }

    #[test]
    fn cargo_error_span_becomes_error_diagnostic() {
        let text = "\
error[E0308]: mismatched types
 --> src/main.rs:12:5
  |
12 |     let x: u8 = \"text\";
  |                 ^^^^^^ expected `u8`, found `&str`";
        let d = parse_tool_output("cargo build", text, cwd());
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].path, "/repo/src/main.rs");
        assert_eq!(d[0].diag.severity, DiagnosticSeverity::Error);
        assert_eq!((d[0].diag.line, d[0].diag.column), (11, 4));
        assert_eq!(d[0].diag.message, "mismatched types");
    }

    #[test]
    fn cargo_warning_is_warning_severity() {
        let text = "warning: unused variable: `x`\n --> src/lib.rs:3:9\n";
        let d = parse_tool_output("cargo build", text, cwd());
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].diag.severity, DiagnosticSeverity::Warning);
        assert_eq!(d[0].path, "/repo/src/lib.rs");
    }

    #[test]
    fn cargo_test_panic_links_the_assertion_line() {
        let text = "---- it_works stdout ----\n\
thread 'it_works' panicked at src/lib.rs:8:5:\nassertion failed";
        let d = parse_tool_output("cargo test", text, cwd());
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].path, "/repo/src/lib.rs");
        assert_eq!(d[0].diag.line, 7);
    }

    #[test]
    fn generic_and_pytest_lines_parse() {
        let text = "\
FAILED tests/test_app.py::test_index - AssertionError: bad state
tests/test_app.py:12: AssertionError: bad state
make[1]: *** [Makefile:9: all] Error 1";
        let d = parse_tool_output("pytest", text, cwd());
        assert!(d.iter().any(|t| t.path.ends_with("tests/test_app.py")
            && t.diag.line == 11), "traceback line maps: {d:?}");
        // `make[1]: *** ...` has no leading path shape and must not parse.
        assert!(d.iter().all(|t| !t.path.contains("Makefile")), "{d:?}");
    }

    #[test]
    fn prose_with_colons_does_not_parse() {
        let d = parse_tool_output("x", "note: this: is just prose\nplain line", cwd());
        assert!(d.is_empty());
    }
}
