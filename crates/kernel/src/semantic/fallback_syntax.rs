//! Native fallback syntax highlighting — the non-tree-sitter path.
//!
//! Ownership of the `"syntax"` highlight layer is explicit, never an accident
//! of handler ordering: a major mode with a registered tree-sitter highlight
//! query owns the layer (the tree-sitter lane writes it when a fresh tree
//! lands), and every other buffer with configured rules is owned by this
//! module.  The rules themselves stay Janet configuration, registered at
//! boot through `syntax/define-rules` and `syntax/map-extension`.
//!
//! Incremental model (docs/core-redesign.md §7.1): each buffer's cached
//! ranges carry the revision they describe.  A transaction whose
//! `revision_before` matches is spliced — ranges outside the edited line
//! region survive (shifted by the length delta), the region is rescanned —
//! and anything else falls back to a full rescan of that buffer.

use std::collections::{HashMap, HashSet};

use crate::state::Editor;
use crate::text_engine::Buffer;
use crate::text_engine::transaction::EditTransaction;

const LAYER: &str = "syntax";

pub struct FallbackRules {
    pub keywords: HashSet<String>,
    pub comment_marker: Option<String>,
}

pub struct BufferCache {
    revision: u64,
    lang: String,
    ranges: Vec<(usize, usize, String)>,
}

#[derive(Default)]
pub struct FallbackState {
    pub rules: HashMap<String, FallbackRules>,
    pub extensions: HashMap<String, String>,
    pub ts_modes: HashSet<String>,
    caches: HashMap<usize, BufferCache>,
}

impl FallbackState {
    /// Replace the rules for `lang`.  Keyword match is exact over the
    /// fallback word set (ASCII alphanumeric plus `_ / - ? !`).
    pub fn define_rules(
        &mut self,
        lang: &str,
        keywords: Vec<String>,
        comment_marker: Option<String>,
    ) {
        self.rules.insert(
            lang.to_string(),
            FallbackRules {
                keywords: keywords.into_iter().collect(),
                comment_marker,
            },
        );
        // Cached ranges were produced under the old rules.
        let lang = lang.to_string();
        self.caches.retain(|_, c| c.lang != lang);
    }

    /// Map a path extension (without the dot) to a fallback language.
    pub fn map_extension(&mut self, extension: &str, lang: &str) {
        self.extensions
            .insert(extension.to_lowercase(), lang.to_string());
    }

    pub fn drop_buffer(&mut self, key: usize) {
        self.caches.remove(&key);
    }
}

/// Hand the syntax layer for `mode` to (or take it back from) the
/// tree-sitter lane.  On a flip the fallback's cached ranges are dropped
/// and the layer is cleared on that mode's buffers, so the active owner
/// always starts from a clean slate.
pub fn set_ts_mode(ed: &mut Editor, mode: &str, enabled: bool) {
    if enabled {
        ed.fallback_syntax.ts_modes.insert(mode.to_string());
    } else {
        ed.fallback_syntax.ts_modes.remove(mode);
    }
    let owned: Vec<usize> = ed
        .buffers
        .iter()
        .filter_map(|(key, arc)| {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            (buf.major_mode.name() == mode).then_some(key)
        })
        .collect();
    for key in owned {
        ed.fallback_syntax.caches.remove(&key);
        ed.for_each_view_of_buffer_mut(key, |view| view.clear_highlights_layer(LAYER));
    }
}

/// Called from `Editor::drop_buffer` — the one buffer-teardown path.
pub fn on_buffer_dropped(ed: &mut Editor, key: usize) {
    ed.fallback_syntax.drop_buffer(key);
}

/// Full scan when a buffer comes into existence, so opened files are
/// colored before their first edit.
pub fn on_buffer_created(ed: &mut Editor, key: usize) {
    force_highlight(ed, key);
}

/// Explicit re-highlight (`syntax-highlight` command): full scan, still
/// respecting ownership.
pub fn force_highlight(ed: &mut Editor, key: usize) {
    let Some(arc) = ed.buffers.get(key).cloned() else {
        return;
    };
    let (mode_name, lang) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        (
            buf.major_mode.name().to_string(),
            lang_for(&ed.fallback_syntax, &buf),
        )
    };
    let Some(lang) = lang else {
        return;
    };
    if ed.fallback_syntax.ts_modes.contains(&mode_name) {
        return;
    }
    let (revision, ranges) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let revision = buf.revision();
        let text = buf.slice(0, buf.len());
        let Some(rules) = ed.fallback_syntax.rules.get(&lang) else {
            return;
        };
        (revision, scan(&text, rules))
    };
    ed.fallback_syntax.caches.insert(
        key,
        BufferCache {
            revision,
            lang,
            ranges: ranges.clone(),
        },
    );
    ed.for_each_view_of_buffer_mut(key, |view| view.set_highlights_layer(LAYER, ranges.clone()));
}

/// Incremental update from the canonical change-publication path: typed
/// transaction in hand, no display strings re-parsed, no whole-document
/// copy unless the cache is unusable.
pub fn on_transaction(ed: &mut Editor, key: usize, txn: &EditTransaction) {
    let Some(arc) = ed.buffers.get(key).cloned() else {
        ed.fallback_syntax.caches.remove(&key);
        return;
    };
    let (mode_name, lang, current_rev) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        (
            buf.major_mode.name().to_string(),
            lang_for(&ed.fallback_syntax, &buf),
            buf.revision(),
        )
    };
    let state = &mut ed.fallback_syntax;
    if state.ts_modes.contains(&mode_name) {
        // The tree-sitter lane owns this layer; never race it.
        state.caches.remove(&key);
        return;
    }
    let Some(lang) = lang else {
        // Nothing owns this buffer's syntax anymore — clear the layer and
        // the cache so no stale ranges outlive the language.
        state.caches.remove(&key);
        ed.for_each_view_of_buffer_mut(key, |view| view.clear_highlights_layer(LAYER));
        return;
    };
    if !state.rules.contains_key(&lang) {
        // No rules for this language: nothing to own, drop any stale cache.
        state.caches.remove(&key);
        return;
    }
    // Splicing needs the transaction's coordinates to line up with the
    // cached text AND the rescan region to be read from the state that
    // transaction produced.  Both hold only when this transaction is the
    // one that brought the buffer to its current revision; anything older
    // in the same drain falls back to a full rescan of the current text.
    let is_current = txn.revision_after.0 == current_rev;
    let computed = {
        let state = &ed.fallback_syntax;
        let rules = &state.rules[&lang];
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        match state
            .caches
            .get(&key)
            .filter(|c| c.revision == txn.revision_before.0 && c.lang == lang && is_current)
        {
            Some(cache) => incremental_ranges(&buf, cache, txn, rules),
            None => {
                let text = buf.slice(0, buf.len());
                scan(&text, rules)
            }
        }
    };
    let state = &mut ed.fallback_syntax;
    state.caches.insert(
        key,
        BufferCache {
            revision: current_rev,
            lang,
            ranges: computed.clone(),
        },
    );
    ed.for_each_view_of_buffer_mut(key, |view| {
        view.set_highlights_layer(LAYER, computed.clone())
    });
}

fn lang_for(state: &FallbackState, buf: &Buffer) -> Option<String> {
    let path = buf.language_path.as_deref().or(buf.path.as_deref())?;
    // Extension after the last dot, matching the Janet resolver this
    // replaces (directory dots included — existing configs never hit them).
    let ext = path.rsplit_once('.')?.1.to_lowercase();
    state.extensions.get(&ext).cloned()
}

// Via the line index (O(log n)), never a prefix copy: these run per
// transaction, and a prefix scan made a keystroke at the end of a large
// file cost O(edit-offset) on the foreground thread.
fn line_start_of(buf: &Buffer, offset: usize) -> usize {
    let (line, _) = buf.line_col_bytes(offset);
    buf.line_start_offset(line).unwrap_or(0)
}

fn line_end_of(buf: &Buffer, offset: usize) -> usize {
    let (line, _) = buf.line_col_bytes(offset);
    match buf.line_span(line) {
        Some(span) => span.end,
        None => buf.len(),
    }
}

fn incremental_ranges(
    buf: &Buffer,
    cache: &BufferCache,
    txn: &EditTransaction,
    rules: &FallbackRules,
) -> Vec<(usize, usize, String)> {
    if txn.edits.is_empty() {
        return cache.ranges.clone();
    }
    let mut min_start = usize::MAX;
    let mut max_new_end = 0;
    let mut delta: i64 = 0;
    for ce in &txn.edits {
        let start = ce.edit.start();
        let old_end = ce.edit.end();
        let new_end = start + ce.edit.inserted_len();
        min_start = min_start.min(start);
        max_new_end = max_new_end.max(new_end);
        delta += new_end as i64 - old_end as i64;
    }
    let region_start = line_start_of(buf, min_start);
    let region_end = line_end_of(buf, max_new_end);
    let Ok(old_end) = i64::try_from(region_end).map(|e| e - delta) else {
        return full_ranges(buf, rules);
    };
    if old_end < 0 {
        return full_ranges(buf, rules);
    }
    let old_end = old_end as usize;
    let mut out: Vec<(usize, usize, String)> = Vec::new();
    for &(rs, re, ref face) in &cache.ranges {
        if re <= region_start {
            out.push((rs, re, face.clone()));
        } else if rs >= old_end {
            out.push((
                (rs as i64 + delta).max(0) as usize,
                (re as i64 + delta).max(0) as usize,
                face.clone(),
            ));
        }
        // Intersecting the edited region: dropped, the rescan re-derives it.
    }
    let text = buf.slice(region_start, region_end.min(buf.len()));
    for (s, e, face) in scan(&text, rules) {
        out.push((s + region_start, e + region_start, face));
    }
    out.sort_by_key(|r| r.0);
    out
}

fn full_ranges(buf: &Buffer, rules: &FallbackRules) -> Vec<(usize, usize, String)> {
    let text = buf.slice(0, buf.len());
    scan(&text, rules)
}

// ── Scanner ──────────────────────────────────────────────────────────────────
//
// Byte-offset passes mirroring the Janet scanners this replaces.  The
// delimiters (`"`, `\`, comment markers, digits) are all ASCII, so scanning
// bytes can never split a match; UTF-8 continuation bytes are simply
// non-word bytes exactly as they were under the byte-wise Janet scans.

pub(crate) type Range = (usize, usize, String);

pub(crate) fn scan(text: &str, rules: &FallbackRules) -> Vec<Range> {
    let mut out = Vec::new();
    scan_keywords(text, rules, &mut out);
    if let Some(marker) = &rules.comment_marker {
        scan_comments(text, marker, &mut out);
    }
    scan_strings(text, &mut out);
    scan_numbers(text, &mut out);
    out.sort_by_key(|r| r.0);
    out
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '-' | '?' | '!')
}

fn scan_keywords(text: &str, rules: &FallbackRules, out: &mut Vec<Range>) {
    let mut prev_word = false;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if is_word_char(c) {
            if !prev_word {
                start = i;
            }
        } else if prev_word {
            if rules.keywords.contains(&text[start..i]) {
                out.push((start, i, "keyword-face".to_string()));
            }
        }
        prev_word = is_word_char(c);
    }
    if prev_word && rules.keywords.contains(&text[start..]) {
        out.push((start, text.len(), "keyword-face".to_string()));
    }
}

fn scan_comments(text: &str, marker: &str, out: &mut Vec<Range>) {
    let mut from = 0;
    while let Some(rel) = text[from..].find(marker) {
        let start = from + rel;
        let after = start + marker.len();
        let end = text[after..].find('\n').map_or(text.len(), |i| after + i);
        out.push((start, end, "comment-face".to_string()));
        from = after.max(end.min(text.len()));
    }
}

fn scan_strings(text: &str, out: &mut Vec<Range>) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if bytes[i] == b'"' {
            let start = i;
            let mut j = start + 1;
            let mut closed = false;
            while j < n {
                if bytes[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if bytes[j] == b'"' {
                    j += 1;
                    closed = true;
                    break;
                }
                j += 1;
            }
            if closed {
                out.push((start, j, "string-face".to_string()));
            }
            i = j.max(start + 1);
        } else {
            i += 1;
        }
    }
}

fn scan_numbers(text: &str, out: &mut Vec<Range>) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        let preceded_by_word = i > 0 && is_word_char(bytes[i - 1] as char);
        if bytes[i].is_ascii_digit() && !preceded_by_word {
            let start = i;
            let mut j = start + 1;
            while j < n && matches!(bytes[j], b'0'..=b'9' | b'.' | b'x' | b'X' | b'e' | b'E') {
                j += 1;
            }
            out.push((start, j, "constant-face".to_string()));
            i = j;
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(keywords: &[&str], marker: Option<&str>) -> FallbackRules {
        FallbackRules {
            keywords: keywords.iter().map(|s| s.to_string()).collect(),
            comment_marker: marker.map(|s| s.to_string()),
        }
    }

    #[test]
    fn keywords_respect_word_boundaries() {
        let r = rules(&["fn", "let"], None);
        let out = scan("fnord fn let", &r);
        // "fnord" must not match the "fn" keyword.
        assert_eq!(
            out,
            vec![
                (6, 8, "keyword-face".into()),
                (9, 12, "keyword-face".into())
            ]
        );
    }

    #[test]
    fn word_set_includes_slash_dash_question_bang() {
        let r = rules(&["string/slice", "swap!"], None);
        let out = scan("(string/slice s 0) (swap! a)", &r);
        assert_eq!(
            out,
            vec![
                (1, 13, "keyword-face".into()),
                (16, 17, "constant-face".into()),
                (20, 25, "keyword-face".into()),
            ]
        );
    }

    #[test]
    fn comments_end_at_newline_and_keep_it() {
        let r = rules(&[], Some("//"));
        let out = scan("a // note\nb", &r);
        assert_eq!(out, vec![(2, 9, "comment-face".into())]);
    }

    #[test]
    fn strings_handle_escapes_and_aremultiline() {
        let r = rules(&[], None);
        let out = scan(
            r#"a "x\"y" b "multi
line" c"#,
            &r,
        );
        assert_eq!(
            out,
            vec![(2, 8, "string-face".into()), (11, 23, "string-face".into())]
        );
    }

    #[test]
    fn unterminated_string_produces_no_range() {
        let r = rules(&[], None);
        assert!(scan("abc \"open", &r).is_empty());
    }

    #[test]
    fn numbers_need_a_non_word_left_neighbor() {
        let r = rules(&[], None);
        let out = scan("1 0x1F 2.5e3 a1 v2", &r);
        let faces: Vec<&str> = out.iter().map(|(_, _, f)| f.as_str()).collect();
        assert_eq!(faces, vec!["constant-face"; 3]);
        assert_eq!(out[0].0, 0);
        assert_eq!(out[1].0, 2);
        assert_eq!(out[2].0, 7);
    }

    #[test]
    fn unicode_offsets_stay_in_bytes() {
        let r = rules(&["let"], None);
        // é is two bytes; the keyword range must still be byte-accurate.
        let out = scan("é let", &r);
        assert_eq!(out, vec![(3, 6, "keyword-face".into())]);
    }

    #[test]
    fn empty_text_scans_to_nothing() {
        let r = rules(&["fn"], Some("//"));
        assert!(scan("", &r).is_empty());
    }

    #[test]
    fn large_region_scan_finds_tokens_across_chunk_sized_offsets() {
        // Longer than a typical rope chunk (4 KiB): the scanner works on
        // the materialized slice, so tokens deep in the text must surface
        // with exact offsets.
        let filler = "x".repeat(9 * 1024);
        let text = format!("{filler} fn rest");
        let r = rules(&["fn"], None);
        let out = scan(&text, &r);
        assert_eq!(
            out,
            vec![(9 * 1024 + 1, 9 * 1024 + 3, "keyword-face".into())]
        );
    }
}
