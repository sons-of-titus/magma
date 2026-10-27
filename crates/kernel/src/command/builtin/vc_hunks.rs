//! Zero-context diff hunks: parsing and the marks derived from them.
//!
//! The `@@ -a,b +c,d @@` headers are the single source for both the side-by-
//! side diff's colour marks and the hunk a cursor addresses, so these are
//! pure functions on the patch text — no editor state involved.

/// Which side of the diff a changed line sits on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Bar { Add, Del }

/// One parsed `@@ -a,b +c,d @@` hunk range (1-based; lengths may be 0).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct HunkRange {
    pub old_start: usize,
    pub old_len: usize,
    pub new_start: usize,
    pub new_len: usize,
}

/// Parse `@@ -a,b +c,d @@` headers; omitted lengths mean 1.
pub(crate) fn parse_hunks(patch: &str) -> Vec<HunkRange> {
    patch.lines().filter_map(parse_hunk_header).collect()
}

fn parse_hunk_header(line: &str) -> Option<HunkRange> {
    let rest = line.strip_prefix("@@ ")?;
    let end = rest.find(" @@")?;
    let mut parts = rest[..end].split(' ');
    let (old_start, old_len) = parse_range(parts.next()?.strip_prefix('-')?)?;
    let (new_start, new_len) = parse_range(parts.next()?.strip_prefix('+')?)?;
    Some(HunkRange { old_start, old_len, new_start, new_len })
}

fn parse_range(s: &str) -> Option<(usize, usize)> {
    match s.split_once(',') {
        Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        None => Some((s.parse().ok()?, 1)),
    }
}

/// Does `hunk` cover 0-based `line` on the new side?  A pure deletion has no
/// new-side lines, so it matches the position where the lines used to be.
pub(crate) fn hunk_covers(hunk: &HunkRange, line: usize) -> bool {
    let target = line + 1;
    if hunk.new_len == 0 {
        return target == hunk.new_start;
    }
    target >= hunk.new_start && target < hunk.new_start + hunk.new_len
}

/// 0-based changed line numbers on one side of the patch.
pub(crate) fn changed_lines(hunks: &[HunkRange], left: bool) -> Vec<usize> {
    let mut out = Vec::new();
    for h in hunks {
        let (start, len) = if left { (h.old_start, h.old_len) } else { (h.new_start, h.new_len) };
        for n in start..start + len {
            if n > 0 { out.push(n - 1); }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Byte spans `(start, end)` per line, excluding the newline.
pub(crate) fn line_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        spans.push((offset, offset + line.len()));
        offset += raw.len();
    }
    spans
}
