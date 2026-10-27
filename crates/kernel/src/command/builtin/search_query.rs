//! The shared in-buffer search query: substring fast path, regex mode,
//! and smart case (lowercase-only patterns match case-insensitively).
//! Used by the search dialog, the repeat commands, and replace.

use regex::Regex;

#[derive(Clone)]
pub enum SearchQuery {
    /// Case-sensitive literal — the chunk-scan fast path.
    Literal(String),
    /// Regex (also carries case-insensitive literals, compiled as an
    /// escaped pattern so both share one matcher).
    Regex(Regex),
}

fn smart_case_insensitive(pattern: &str) -> bool {
    !pattern.chars().any(|c| c.is_uppercase())
}

impl SearchQuery {
    pub fn compile(pattern: &str, regex_mode: bool) -> Result<SearchQuery, String> {
        if pattern.is_empty() {
            return Err("empty search pattern".to_string());
        }
        let ci = smart_case_insensitive(pattern);
        if !regex_mode && !ci {
            return Ok(SearchQuery::Literal(pattern.to_string()));
        }
        let body = if regex_mode { pattern.to_string() } else { regex::escape(pattern) };
        let re = Regex::new(&format!("(?i){body}"))
            .map_err(|e| format!("invalid pattern: {e}"))?;
        Ok(SearchQuery::Regex(re))
    }

    /// Next match at/after `from`: `(start, end)` byte offsets.
    /// Regex mode materializes the buffer once per call — search is an
    /// explicit action, not a per-keystroke path (the literal fast path
    /// stays chunk-scanned).
    pub fn find_from(
        &self,
        buf: &crate::text_engine::Buffer,
        from: usize,
    ) -> Option<(usize, usize)> {
        match self {
            SearchQuery::Literal(needle) => {
                buf.find_sub_from(from, needle).map(|at| (at, at + needle.len()))
            }
            SearchQuery::Regex(re) => {
                let text = buf.slice(0, buf.len());
                re.find_at(&text, from).map(|m| (m.start(), m.end()))
            }
        }
    }

    /// Expand `replacement` for a matched span — capture groups
    /// (`$1`, `${name}`) in regex mode, verbatim otherwise.
    pub fn expand(&self, matched: &str, replacement: &str) -> String {
        match self {
            SearchQuery::Literal(_) => replacement.to_string(),
            SearchQuery::Regex(re) => {
                let Some(caps) = re.captures(matched) else {
                    return replacement.to_string();
                };
                let mut out = String::with_capacity(replacement.len());
                caps.expand(replacement, &mut out);
                out
            }
        }
    }

    /// All matches `(start, end)` from `from`, capped at `limit`.
    pub fn find_all(
        &self,
        buf: &crate::text_engine::Buffer,
        from: usize,
        limit: usize,
    ) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut at = from;
        while let Some((s, e)) = self.find_from(buf, at) {
            if out.len() >= limit {
                break;
            }
            out.push((s, e));
            at = if e > s { e } else { s + 1 }; // empty matches advance
        }
        out
    }
}

/// Replace every match in the focused buffer.  One undo step for the
/// whole operation; returns the replacement count.
pub fn replace_all(
    ed: &mut crate::state::Editor,
    pattern: &str,
    replacement: &str,
    regex_mode: bool,
) -> Result<usize, String> {
    let query = SearchQuery::compile(pattern, regex_mode)?;
    let buf_id = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .ok_or("no focused buffer")?;
    let Some(view) = ed.view_of_buffer_mut(buf_id) else {
        return Err("no view for focused buffer".to_string());
    };
    let spans = {
        let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
        query.find_all(&buf, 0, 100_000)
    };
    if spans.is_empty() {
        return Ok(0);
    }
    // Back-to-front keeps earlier offsets valid; expansions are computed
    // against the original matched text before any mutation.
    let texts: Vec<String> = {
        let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
        spans.iter().map(|(s, e)| query.expand(&buf.slice(*s, *e), replacement)).collect()
    };
    view.open_undo_session();
    for ((start, end), text) in spans.iter().zip(texts).rev() {
        view.replace(*start, *end, &text);
    }
    view.close_undo_session();
    ed.frame_dirty = true;
    Ok(spans.len())
}

/// Replace the first match at/after the cursor (wrapping once) and move
/// the cursor after it.  Returns false when nothing matched.
pub fn replace_next(
    ed: &mut crate::state::Editor,
    pattern: &str,
    replacement: &str,
    regex_mode: bool,
) -> Result<bool, String> {
    let query = SearchQuery::compile(pattern, regex_mode)?;
    let buf_id = ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.buffer(wid))
        .ok_or("no focused buffer")?;
    let Some(view) = ed.view_of_buffer_mut(buf_id) else {
        return Err("no view for focused buffer".to_string());
    };
    let cursor = view.cursor_offset();
    let found = {
        let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
        query.find_from(&buf, cursor)
            .or_else(|| query.find_from(&buf, 0).filter(|(s, _)| *s < cursor))
    };
    let Some((start, end)) = found else {
        return Ok(false);
    };
    view.replace(start, end, replacement);
    view.set_cursor(start + replacement.len());
    ed.frame_dirty = true;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::id::BufferId;

    fn buf(text: &str) -> crate::text_engine::Buffer {
        crate::text_engine::Buffer::from_string(BufferId(1), "t", text)
    }

    #[test]
    fn smart_case_literal_matches_insensitively_when_lowercase() {
        let q = SearchQuery::compile("hello", false).unwrap();
        let matches = q.find_all(&buf("Hello hello HELLO"), 0, 100);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn mixed_case_literal_is_case_sensitive() {
        let q = SearchQuery::compile("Hello", false).unwrap();
        let matches = q.find_all(&buf("Hello hello HELLO"), 0, 100);
        assert_eq!(matches, vec![(0, 5)]);
    }

    #[test]
    fn literal_special_characters_are_not_regex() {
        let q = SearchQuery::compile("a.c*", false).unwrap();
        let b = buf("a.c* abc*");
        let matches = q.find_all(&b, 0, 100);
        assert_eq!(matches, vec![(0, 4)], "the literal dots and star match verbatim");
    }

    #[test]
    fn regex_mode_compiles_patterns() {
        let q = SearchQuery::compile(r"\bfn\s+\w+", true).unwrap();
        let b = buf("fn  main() {} fn x()");
        let matches = q.find_all(&b, 0, 100);
        assert_eq!(matches.len(), 2);
        assert_eq!(b.slice(matches[0].0, matches[0].1), "fn  main");
    }

    #[test]
    fn invalid_regex_reports_an_error() {
        assert!(SearchQuery::compile("(unclosed", true).is_err());
    }
}
