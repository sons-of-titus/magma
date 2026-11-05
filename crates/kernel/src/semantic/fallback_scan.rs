//! Native fallback scanner — the byte passes behind
//! [`super::fallback_syntax`].
//!
//! Split from that module at the file-size gate: the scans are pure text
//! transforms with no editor state, which is also what makes them testable
//! on their own.  Comments and strings are collected first and shield their
//! contents: a word inside `// fn x` is prose and a word inside `"let"` is
//! a literal — painting either as a keyword would put syntax where the
//! reader sees none.  The delimiters (`"`, `\`, comment markers, digits)
//! are ASCII, so scanning bytes can never split a match; UTF-8 continuation
//! bytes are simply non-word bytes.

use std::collections::HashSet;

/// The scanner's input: a language's keyword set and line-comment marker.
pub struct FallbackRules {
    pub keywords: HashSet<String>,
    pub comment_marker: Option<String>,
}

pub(crate) type Range = (usize, usize, String);

pub(crate) fn scan(text: &str, rules: &FallbackRules) -> Vec<Range> {
    let mut out = Vec::new();
    scan_keywords(text, rules, &mut out);
    if let Some(marker) = &rules.comment_marker {
        scan_comments(text, marker, &mut out);
    }
    scan_strings(text, rules, &mut out);
    scan_numbers(text, &mut out);

    let mut shielded: Vec<(usize, usize)> = out
        .iter()
        .filter(|(_, _, face)| face == "comment-face" || face == "string-face")
        .map(|(start, end, _)| (*start, *end))
        .collect();
    if !shielded.is_empty() {
        shielded.sort_unstable();
        out.retain(|(start, end, face)| {
            if face != "keyword-face" && face != "constant-face" {
                return true;
            }
            !shielded
                .iter()
                .any(|(ps, pe)| *start < *pe && *ps < *end)
        });
    }

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

fn scan_strings(text: &str, rules: &FallbackRules, out: &mut Vec<Range>) {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let single_quotes = rules.comment_marker.as_deref() == Some("--");
    let mut i = 0;
    while i < n {
        if bytes[i] == b'"' || (single_quotes && bytes[i] == b'\'') {
            let quote = bytes[i];
            let start = i;
            let mut j = start + 1;
            let mut closed = false;
            while j < n {
                if bytes[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if bytes[j] == quote {
                    if quote == b'\'' && j + 1 < n && bytes[j + 1] == b'\'' {
                        j += 2;
                        continue;
                    }
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
    fn comments_shield_their_contents() {
        let r = rules(&["fn", "let"], Some("//"));
        let out = scan("// fn let\nlet x;", &r);
        assert_eq!(
            out,
            vec![
                (0, 9, "comment-face".into()),
                (10, 13, "keyword-face".into())
            ],
            "keywords inside the comment must not paint over it"
        );
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
    fn strings_shield_their_contents() {
        let r = rules(&["let"], None);
        let out = scan("let s = \"let\";", &r);
        assert_eq!(
            out,
            vec![
                (0, 3, "keyword-face".into()),
                (8, 13, "string-face".into())
            ],
            "keywords inside a string literal must not paint over it"
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
    fn comments_shield_numbers_too() {
        let r = rules(&[], Some("//"));
        let out = scan("// fix 42 now\nx 7", &r);
        assert_eq!(
            out,
            vec![
                (0, 13, "comment-face".into()),
                (16, 17, "constant-face".into())
            ]
        );
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
