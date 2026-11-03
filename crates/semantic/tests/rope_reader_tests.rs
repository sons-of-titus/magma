//! The tree-sitter read contract over a rope.
//!
//! The parser's read callback may be asked for any byte offset — including
//! earlier ones than the last read.  Tree-sitter re-reads a token from its
//! start (keyword classification) and rewinds on error recovery; when a
//! token straddles a leaf boundary the lexer has already moved into the
//! next leaf, so the rewind lands *behind* the current read position.  A
//! reader that only walks forward serves the wrong bytes and the token —
//! with everything parsed after it — misparses.  That is the "highlighting
//! is fine at first, then it is a mess" behaviour of an edited buffer:
//! a freshly opened file is one leaf, and every edit at a new place splits
//! one, so boundaries inside tokens appear as a session goes on.

use semantic::ts_registry;
use text::rope::Rope;

fn grammar_path() -> Option<std::path::PathBuf> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let dylib =
        std::path::Path::new(manifest).join("../../plugins/rust/grammars/tree-sitter-rust.dylib");
    dylib.exists().then_some(dylib)
}

fn load_rust_grammar(name: &str) -> Option<String> {
    let dylib = grammar_path()?;
    let lang = ts_registry::load_grammar_from_lib(dylib.to_str().unwrap(), name, None)
        .expect("grammar loads");
    ts_registry::register_language(name, lang);
    Some(name.to_string())
}

/// A rust file shaped like real code: doc comments, a long identifier that
/// spans several leaves (nothing else would make a leaf boundary land
/// inside a token in a fixture this size), and ordinary items.
fn source() -> String {
    let long_name = format!("fragmented_{}_identifier", "name_".repeat(2_000));
    format!(
        "\
//! Module documentation for the rope reader.
use std::collections::HashMap;

/// The editor's incremental parse must read the rope at any byte offset,
/// including earlier ones, or tokens straddling a leaf boundary misparse.
fn editor_with(content: &str) -> usize {{
    let {long_name} = content.len();
    // a comment mentioning fn and let inside it
    {long_name} + 1
}}

/// A second doc comment, so both a comment and an identifier are near the
/// split points of the rope.
fn second() -> usize {{
    let value = editor_with(\"text\");
    value
}}
"
    )
}

/// Edits fragment the rope: each insert splits the leaf it lands in at its
/// midpoint, so a handful of spread-out inserts leave several leaves with
/// boundaries inside the long identifier.  The inserted spaces are harmless
/// to the parse, and callers compare against the rope's own text.  Both
/// texts of the incremental test take the same offsets, so the difference
/// between them stays exactly the one edited byte.
fn fragment_offsets(len: usize) -> Vec<usize> {
    (1..6).map(|k| len * k / 6).collect()
}

fn fragmented(text: &str, offsets: &[usize]) -> Rope {
    let mut rope = Rope::from_string(text);
    for &at in offsets {
        if at > 0 && at < rope.len() {
            rope.insert(at, " ");
        }
    }
    rope
}

fn leaves(rope: &Rope) -> usize {
    rope.chunks().count()
}

fn rope_text(rope: &Rope) -> String {
    rope.slice(0, rope.len()).into_owned()
}

fn named_nodes(node: tree_sitter::Node<'_>, out: &mut Vec<(usize, usize, String)>) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() {
                out.push((child.start_byte(), child.end_byte(), child.kind().to_string()));
                named_nodes(child, out);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn fingerprint(tree: &tree_sitter::Tree) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    named_nodes(tree.root_node(), &mut out);
    out
}

fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset];
    let row = before.matches('\n').count();
    let col = before.rfind('\n').map_or(offset, |i| offset - i - 1);
    (row, col)
}

#[test]
fn fragmented_rope_parses_identically_to_a_string_parse() {
    let Some(lang) = load_rust_grammar("rustreader") else {
        eprintln!("rust grammar not built — run `cargo xtask grammars` to cover this test");
        return;
    };
    let base = source();
    let offsets = fragment_offsets(base.len());
    let rope = fragmented(&base, &offsets);
    let text = rope_text(&rope);
    let leaf_count = leaves(&rope);
    assert!(
        leaf_count > 1,
        "the fixture must fragment the rope, got {leaf_count} leaf"
    );

    let from_rope = ts_registry::parse_rope(&lang, &rope, None).expect("rope parse");
    let from_string = ts_registry::parse_tree(&lang, &text).expect("string parse");

    assert_eq!(
        fingerprint(&from_rope),
        fingerprint(&from_string),
        "a rope with {leaf_count} leaves must parse exactly like the same text as one string"
    );
}

#[test]
fn incremental_parse_matches_a_fresh_parse_after_an_edit() {
    let Some(lang) = load_rust_grammar("rustreader") else {
        eprintln!("rust grammar not built — run `cargo xtask grammars` to cover this test");
        return;
    };
    let base = source();
    let offsets = fragment_offsets(base.len());
    let rope_before = fragmented(&base, &offsets);
    let before = rope_text(&rope_before);
    assert!(leaves(&rope_before) > 1, "fixture must fragment the rope");

    // Join two lines inside the doc comment: the edit whose old end point
    // cannot be read off the post-edit text.
    let cut = before.find("including earlier ones").unwrap();
    let newline = before[..cut].rfind('\n').unwrap();
    let (row, col) = line_col(&before, newline);
    let start_position = tree_sitter::Point::new(row, col);
    let after = format!("{}{}", &before[..newline], &before[newline + 1..]);

    let mut tree = ts_registry::parse_rope(&lang, &rope_before, None).expect("first parse");
    tree.edit(&tree_sitter::InputEdit {
        start_byte: newline,
        old_end_byte: newline + 1,
        new_end_byte: newline,
        start_position,
        old_end_position: tree_sitter::Point::new(row + 1, 0),
        new_end_position: start_position,
    });

    // The rope's text must be exactly `after`: `before` already carries the
    // fixture's spaces, so this one is built plainly.
    let rope_after = Rope::from_string(&after);
    let incremental =
        ts_registry::parse_rope(&lang, &rope_after, Some(&tree)).expect("incremental parse");
    let fresh = ts_registry::parse_tree(&lang, &after).expect("fresh parse");

    assert_eq!(
        fingerprint(&incremental),
        fingerprint(&fresh),
        "an incremental parse after a line join must match a fresh parse"
    );
}

/// The reader the rope used to expose: a monotonic chunk walk that cannot
/// serve an offset behind its cursor.  This test proves the fixture above
/// really exercises a backward read — if the parser only ever read forward,
/// both readers would agree and the equality test would be vacuous.
#[test]
fn a_forward_only_reader_corrupts_a_fragmented_parse() {
    let Some(dylib) = grammar_path() else {
        eprintln!("rust grammar not built — run `cargo xtask grammars` to cover this test");
        return;
    };
    let language = ts_registry::load_grammar_from_lib(dylib.to_str().unwrap(), "rustforward", None)
        .expect("grammar loads");
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).expect("language");

    let base = source();
    let offsets = fragment_offsets(base.len());
    let rope = fragmented(&base, &offsets);
    let text = rope_text(&rope);
    assert!(leaves(&rope) > 1, "fixture must fragment the rope");
    let mut chunks = rope.chunks();
    let mut cur: &str = "";
    let mut cur_pos = 0usize;
    let tree = parser
        .parse_with_options(
            &mut |offset: usize, _: tree_sitter::Point| -> &[u8] {
                while offset >= cur_pos + cur.len() {
                    match chunks.next() {
                        Some(c) => {
                            cur_pos += cur.len();
                            cur = c;
                        }
                        None => break,
                    }
                }
                let rel = offset.saturating_sub(cur_pos).min(cur.len());
                &cur.as_bytes()[rel..]
            },
            None,
            None,
        )
        .expect("parse");

    let mut reference = tree_sitter::Parser::new();
    reference.set_language(&language).expect("language");
    let reference = reference.parse(text.as_bytes(), None).expect("parse");

    assert_ne!(
        fingerprint(&tree),
        fingerprint(&reference),
        "the fixture must make a forward-only reader diverge — it did not, \
         so the equality tests above prove nothing"
    );
}
