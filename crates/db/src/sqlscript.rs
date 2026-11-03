//! SQL script splitting: statement boundaries for script execution and
//! statement-at-cursor execution.  Lexical only — quote, comment, bracket,
//! and PostgreSQL dollar-quote aware.

/// One statement span: the trimmed text plus its byte range in the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedStatement {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// Split a script into statements.  Spans that hold nothing but comments
/// and whitespace drop out.
pub fn split_statements(sql: &str) -> Vec<SpannedStatement> {
    let bytes = sql.as_bytes();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => i = scan_quoted(sql, i, bytes[i]),
            b'[' => i = scan_bracket(sql, i),
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => i = scan_to_line_end(sql, i),
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => i = scan_block_comment(sql, i),
            b'$' => {
                if let Some(end) = dollar_end(sql, i) {
                    i = end;
                } else {
                    i += 1;
                }
            }
            b';' => {
                spans.push((start, i));
                start = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    spans.push((start, sql.len()));
    spans
        .into_iter()
        .filter_map(|(a, b)| {
            let raw = &sql[a..b];
            if comments_stripped(raw).trim().is_empty() {
                return None;
            }
            let lead = raw.len() - raw.trim_start().len();
            Some(SpannedStatement {
                text: raw.trim().to_string(),
                start: a + lead,
                end: b,
            })
        })
        .collect()
}

/// The statement containing byte `offset` — the current-statement run
/// command's target.  A cursor sitting in trailing space after the last
/// `;` still belongs to that statement.  Returns the trimmed text.
pub fn statement_at(sql: &str, offset: usize) -> Option<SpannedStatement> {
    let offset = offset.min(sql.len());
    let spans = split_statements(sql);
    spans
        .iter()
        .find(|s| offset >= s.start && offset <= s.end)
        .cloned()
        .or_else(|| {
            offset
                .checked_sub(1)
                .and_then(|o| spans.into_iter().find(|s| o >= s.start && o <= s.end))
        })
}

fn scan_quoted(sql: &str, mut i: usize, quote: u8) -> usize {
    let bytes = sql.as_bytes();
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' && quote == b'\'' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            if i + 1 < bytes.len() && bytes[i + 1] == quote {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn scan_bracket(sql: &str, mut i: usize) -> usize {
    let bytes = sql.as_bytes();
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b']' {
            if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn scan_to_line_end(sql: &str, mut i: usize) -> usize {
    while i < sql.len() && sql.as_bytes()[i] != b'\n' {
        i += 1;
    }
    (i + 1).min(sql.len())
}

fn scan_block_comment(sql: &str, mut i: usize) -> usize {
    let bytes = sql.as_bytes();
    i += 2;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return i + 2;
        }
        i += 1;
    }
    bytes.len()
}

/// If `i` opens a dollar quote (`$$` or `$tag$`), return the byte index one
/// past its closing tag.  `$1`-style placeholders are not quotes.
fn dollar_end(sql: &str, i: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    let mut j = i + 1;
    while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b'$' {
        return None;
    }
    let tag = &sql[i..=j];
    let close = sql[j + 1..].find(tag)?;
    Some(j + 1 + close + tag.len())
}

/// The text with `--` and `/* */` comments blanked out — the emptiness
/// check for a span so comment-only spans do not become empty statements.
fn comments_stripped(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(sql.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                let end = scan_quoted(sql, i, bytes[i]).min(bytes.len());
                out.extend_from_slice(sql[i..end].as_bytes());
                i = end;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                i = scan_to_line_end(sql, i);
                out.push(b'\n');
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i = scan_block_comment(sql, i);
                out.push(b' ');
            }
            _ => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(sql: &str) -> Vec<String> {
        split_statements(sql).into_iter().map(|s| s.text).collect()
    }

    #[test]
    fn splits_on_semicolons_and_drops_empty_spans() {
        assert_eq!(texts("a; b ;;\n; c"), vec!["a", "b", "c"]);
        assert!(texts("-- only a comment").is_empty());
        assert!(texts("/* only */ /* comments */").is_empty());
        assert!(texts("   ").is_empty());
    }

    #[test]
    fn semicolons_inside_quotes_and_comments_do_not_split() {
        assert_eq!(
            texts("SELECT ';' AS x; SELECT 2"),
            vec!["SELECT ';' AS x", "SELECT 2"]
        );
        assert_eq!(
            texts("SELECT 1 -- hi ; there\n; SELECT 2"),
            vec!["SELECT 1 -- hi ; there", "SELECT 2"]
        );
        assert_eq!(
            texts("SELECT 'it''s; fine'; SELECT 2"),
            vec!["SELECT 'it''s; fine'", "SELECT 2"]
        );
        assert_eq!(
            texts("/* block ; comment */ SELECT 1; SELECT 2"),
            vec!["/* block ; comment */ SELECT 1", "SELECT 2"]
        );
    }

    #[test]
    fn dollar_quoted_bodies_keep_their_semicolons() {
        let sql = "CREATE FUNCTION f() RETURNS void AS $$ BEGIN x; END; $$ LANGUAGE plpgsql;\nSELECT 2";
        assert_eq!(texts(sql).len(), 2);
        assert!(texts(sql)[0].contains("BEGIN x; END;"));
        assert_eq!(
            texts("SELECT $$a; b$$; SELECT 2"),
            vec!["SELECT $$a; b$$", "SELECT 2"]
        );
        assert_eq!(
            texts("SELECT $body$one; two$body$; SELECT 2").len(),
            2
        );
    }

    #[test]
    fn placeholders_are_not_dollar_quotes() {
        assert_eq!(
            texts("SELECT $1, $2; SELECT 3"),
            vec!["SELECT $1, $2", "SELECT 3"]
        );
    }

    #[test]
    fn statement_at_finds_the_enclosing_span() {
        let sql = "SELECT 1;\nSELECT 2;\nSELECT 3;";
        assert_eq!(statement_at(sql, 2).unwrap().text, "SELECT 1");
        assert_eq!(statement_at(sql, 12).unwrap().text, "SELECT 2");
        assert_eq!(statement_at(sql, 21).unwrap().text, "SELECT 3");
        assert!(statement_at(sql, sql.len()).is_some());
        assert!(statement_at("", 0).is_none());
    }

    #[test]
    fn brackets_and_backticks_survive() {
        assert_eq!(texts("SELECT [a; b] FROM t; SELECT 2").len(), 2);
        assert_eq!(texts("SELECT `a; b` FROM t; SELECT 2").len(), 2);
    }

    #[test]
    fn comment_only_spans_carry_no_offset_vacuum() {
        let sql = "SELECT 1;\n-- trailing note\n";
        let spans = split_statements(sql);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "SELECT 1");
    }
}
