//! Fuzzy matching for GUI pickers (goto-file, goto-symbol, command palette).
//!
//! Subsequence matcher with simple ranking: consecutive-character runs and
//! word-start matches score higher, so `mm` ranks `main.rs` above `mmap_utils.rs`
//! when the match starts at a word boundary.  Pure functions — unit-testable
//! without an egui context.

/// Score how well `text` fuzzy-matches `query`.  `None` when `text` does not
/// contain every query character in order (case-insensitive).
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() { return Some(0); }
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    if q.len() > t.len() { return None; }

    let mut score = 0i32;
    let mut ti = 0usize;
    let mut prev_match: Option<usize> = None;

    for &qc in &q {
        let mut found = None;
        while ti < t.len() {
            if t[ti] == qc {
                found = Some(ti);
                break;
            }
            ti += 1;
        }
        let mi = found?;
        score += 10;
        if let Some(pm) = prev_match
            && mi == pm + 1 { score += 15; }             // consecutive run
        if mi == 0 || is_word_boundary(t[mi - 1]) { score += 20; }  // word start
        if text.chars().nth(mi).map(|c| c.is_uppercase()) == Some(true) {
            score += 5;                                   // case match bonus
        }
        prev_match = Some(mi);
        ti = mi + 1;
    }

    // Prefer shorter candidates: they match more tightly.
    score -= t.len() as i32 / 4;
    Some(score)
}

fn is_word_boundary(c: char) -> bool {
    c == '/' || c == '_' || c == '-' || c == '.' || c == ' '
}

/// Rank `items` against `query`, returning `(score, index)` pairs sorted
/// best-first.  `key` extracts the matchable text from each item.
pub fn fuzzy_rank<T, F>(query: &str, items: &[T], key: F) -> Vec<(i32, usize)>
where F: Fn(&T) -> &str {
    let mut out: Vec<(i32, usize)> = items.iter().enumerate()
        .filter_map(|(i, item)| fuzzy_score(query, key(item)).map(|s| (s, i)))
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn subsequence_order_matters() {
        assert!(fuzzy_score("mrs", "main.rs").is_some());
        assert!(fuzzy_score("smr", "main.rs").is_none());
    }

    #[test]
    fn case_insensitive() {
        assert!(fuzzy_score("MRS", "main.rs").is_some());
    }

    #[test]
    fn word_start_outranks_middle() {
        let start  = fuzzy_score("mm", "mmap_utils.rs").unwrap();
        let middle = fuzzy_score("mm", "common.rs").unwrap();
        assert!(start > middle, "{start} should outrank {middle}");
    }

    #[test]
    fn rank_sorts_best_first() {
        let items = vec!["common.rs".to_string(), "main.rs".to_string()];
        let ranked = fuzzy_rank("mrs", &items, |s| s.as_str());
        assert_eq!(ranked[0].1, 1, "main.rs should rank first");
        assert_eq!(ranked.len(), 2, "both are valid subsequences");
    }

    #[test]
    fn rank_empty_query_keeps_all() {
        let items = vec!["a".to_string(), "b".to_string()];
        assert_eq!(fuzzy_rank("", &items, |s| s).len(), 2);
    }
}
