//! Effective highlight capture per Rust token.
//!
//! The renderer paints captures in the order the query produced them, later
//! spans over earlier ones.  A token matched by both a specific pattern and
//! the generic `(identifier)` fallback must therefore resolve to the
//! specific one — the fallback has to come first in the query file.

use semantic::ts_registry;

fn load_rust_query() -> Option<(String, String)> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let plugins = std::path::Path::new(manifest).join("../../plugins/rust");
    let dylib = plugins.join("grammars/tree-sitter-rust.dylib");
    let query_path = plugins.join("syntax.scm");
    if !dylib.exists() || !query_path.exists() {
        return None;
    }
    let lang = ts_registry::load_grammar_from_lib(dylib.to_str().unwrap(), "rustcaptures", None)
        .expect("grammar loads");
    ts_registry::register_language("rustcaptures", lang);
    Some((
        std::fs::read_to_string(query_path).unwrap(),
        "rustcaptures".to_string(),
    ))
}

/// Replay the capture list the way `apply_highlights` paints it: last write
/// wins, so the scope covering a byte is the one from the final matching
/// range.
fn winning_scopes(
    text: &str,
    caps: &[(usize, usize, String)],
) -> std::collections::HashMap<String, String> {
    let mut winner: Vec<Option<&str>> = vec![None; text.len()];
    for (start, end, scope) in caps {
        for slot in winner.iter_mut().take(*end).skip(*start) {
            *slot = Some(scope);
        }
    }
    let mut scopes = std::collections::HashMap::new();
    for (word_start, word) in words(text) {
        if let Some(Some(scope)) = winner.get(word_start) {
            scopes.insert(word.to_string(), (*scope).to_string());
        }
    }
    scopes
}

fn words(text: &str) -> Vec<(usize, &str)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            let start = i;
            while i < text.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            out.push((start, &text[start..i]));
        } else {
            i += 1;
        }
    }
    out
}

/// Winning scope at a byte, replaying captures the way `apply_highlights`
/// paints: within the layer, the last span covering a byte wins.
fn winning_scope_at(caps: &[(usize, usize, String)], byte: usize) -> Option<(usize, usize, String)> {
    caps.iter()
        .filter(|(start, end, _)| *start <= byte && byte < *end)
        .last()
        .cloned()
}

#[test]
fn every_comment_flavor_resolves_to_a_comment_scope() {
    let Some((query, lang)) = load_rust_query() else {
        eprintln!("rust grammar not built — run `cargo xtask grammars` to cover this test");
        return;
    };
    // Every flavor the grammar distinguishes — a doc comment is a
    // line_comment/block_comment node carrying marker + doc children, so
    // all of them must land on a comment scope, exactly like `//`.
    let text = "\
// plain line
//// four slashes
/// outer doc
//! inner doc
/** outer block doc */
/*! inner block doc */
/* plain block */
";
    let caps = ts_registry::run_query(&lang, text, &query).expect("query runs");

    let mut offset = 0;
    for line in text.lines() {
        let winning = winning_scope_at(&caps, offset);
        let (start, end, scope) = winning.unwrap_or_else(|| {
            panic!("no highlight span covers {line:?} at byte {offset}");
        });
        assert!(
            scope.starts_with("comment"),
            "{line:?} must resolve to a comment scope, got {scope:?}"
        );
        assert!(
            start <= offset && end >= offset + line.len(),
            "{line:?} must be covered whole, got span {start}..{end}"
        );
        offset += line.len() + 1;
    }
}

const SOURCE: &str = "\
use std::time::Instant;

fn editor_with(content: &str) -> Editor {
    let mut ed = Editor::new();
    helper_call(content);
    ed.push_str(&format!(\"{}\", content));
    let _ = Instant::now();
}

struct Stats {
    samples_ns: Vec<u64>,
}

fn build() -> Stats {
    Stats { samples_ns: names() }
}
";

#[test]
fn specific_captures_beat_the_generic_identifier_fallback() {
    let Some((query, lang)) = load_rust_query() else {
        eprintln!("rust grammar not built — run `cargo xtask grammars` to cover this test");
        return;
    };
    let caps = ts_registry::run_query(&lang, SOURCE, &query).expect("query runs");
    let scopes = winning_scopes(SOURCE, &caps);

    for (token, expected) in [
        // Definitions and call targets read as functions.
        ("editor_with", "function"),
        ("helper_call", "function"),
        ("format", "function"),
        ("push_str", "function"),
        ("new", "function"),
        ("now", "function"),
        // Path segments and imported names read as paths.
        ("std", "module"),
        ("time", "module"),
        ("Instant", "module"),
        ("Editor", "module"),
        // Types, fields, and the plain fallback.
        ("Stats", "type"),
        ("Vec", "type"),
        ("samples_ns", "property"),
        ("content", "variable"),
        ("ed", "variable"),
    ] {
        assert_eq!(
            scopes.get(token).map(String::as_str),
            Some(expected),
            "token `{token}` must resolve to {expected}, got {:?}",
            scopes.get(token)
        );
    }
}
