//! Plain-text rendering of a `VcStatus` for the `*vc*` buffer, plus the
//! line→path mapping the buffer's cursor commands use.  Split from `vc::mod`
//! at the display boundary.

use std::path::Path;

use super::VcStatus;

/// Number of display lines above the first file entry in the VC buffer
/// when any section has entries: header line, blank, section label.
pub const VC_HEADER_LINES: usize = 3;

/// Build the full text for the `*vc*` buffer from a `VcStatus`.
pub fn build_display(root: &Path, status: &VcStatus) -> String {
    let root_str = root.to_string_lossy();
    let mut out = format!(
        "## {}  [{}]  {}\n\n",
        status.branch, status.backend_name, root_str,
    );

    if status.staged.is_empty() && status.unstaged.is_empty()
        && status.untracked.is_empty() && status.conflicts.is_empty() {
        out.push_str("  Nothing to commit, working tree clean\n");
    } else {
        if !status.conflicts.is_empty() {
            out.push_str("Conflicts (resolve, then stage with r):\n");
            for f in &status.conflicts {
                out.push_str(&format!("  U  {f}\n"));
            }
            out.push('\n');
        }
        if !status.staged.is_empty() {
            out.push_str("Staged:\n");
            for f in &status.staged {
                out.push_str(&format!("  {}  {}\n", f.code, f.path));
            }
            out.push('\n');
        }
        if !status.unstaged.is_empty() {
            let label = if status.has_staging { "Unstaged:" } else { "Changed:" };
            out.push_str(&format!("{label}\n"));
            for f in &status.unstaged {
                out.push_str(&format!("  {}  {}\n", f.code, f.path));
            }
            out.push('\n');
        }
        if !status.untracked.is_empty() {
            out.push_str("Untracked:\n");
            for f in &status.untracked {
                out.push_str(&format!("  ?  {}\n", f));
            }
            out.push('\n');
        }
    }

    out.push_str("────────────────────────────────────────────────────────────────\n");
    if status.has_staging {
        out.push_str("s stage  u unstage  d diff  h hunks  c commit  A amend  p push  l log  S stash  b branches  r resolve  q close\n");
    } else {
        out.push_str("d diff  c commit  p push  l log  q close\n");
    }
    out
}

/// Return the file path on display line `line` (0-indexed) in the VC buffer.
///
/// Mirrors `build_display`'s layout exactly: header line, blank, then each
/// non-empty section as label line + entry lines + trailing blank.
pub fn path_at_line(status: &VcStatus, line: usize) -> Option<String> {
    if status.staged.is_empty() && status.unstaged.is_empty()
        && status.untracked.is_empty() && status.conflicts.is_empty() {
        return None;
    }
    let mut row = VC_HEADER_LINES - 1; // "## …" + blank; next line is a label

    // Conflicts, staged, and unstaged sections share the layout:
    // label line, one line per entry, one blank line.
    let section = |row: &mut usize, entries: &[String]| -> Option<String> {
        if entries.is_empty() { return None; }
        *row += 1; // label
        let mut found = None;
        for path in entries {
            if *row == line { found = Some(path.clone()); }
            *row += 1;
        }
        *row += 1; // blank
        found
    };
    let conflicts = status.conflicts.clone();
    let staged: Vec<String> = status.staged.iter().map(|f| f.path.clone()).collect();
    let unstaged: Vec<String> = status.unstaged.iter().map(|f| f.path.clone()).collect();
    if let Some(p) = section(&mut row, &conflicts) { return Some(p); }
    if let Some(p) = section(&mut row, &staged) { return Some(p); }
    if let Some(p) = section(&mut row, &unstaged) { return Some(p); }

    if !status.untracked.is_empty() {
        row += 1; // "Untracked:" label
        for f in &status.untracked {
            if row == line { return Some(f.clone()); }
            row += 1;
        }
    }
    None
}
