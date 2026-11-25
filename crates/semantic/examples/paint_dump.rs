//! Dump the winning scope per token, replaying apply_highlights' last-wins
//! painting over the real rust syntax.scm captures.
//!
//! Usage: cargo run -p semantic --example paint_dump [-- file.rs]

use semantic::ts_registry;
use std::path::Path;
use tree_sitter::StreamingIterator;

fn main() {
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let dylib = plugins.join("rust/grammars/tree-sitter-rust.dylib");
    let language = ts_registry::load_grammar_from_lib(
        dylib.to_str().unwrap(),
        "paintdump",
        None,
    )
    .expect("grammar loads");
    ts_registry::register_language("paintdump", language.clone());

    let query_src = std::fs::read_to_string(plugins.join("rust/syntax.scm")).unwrap();
    let query = tree_sitter::Query::new(&language, &query_src).expect("query compiles");

    let text = match std::env::args().nth(1) {
        Some(path) => std::fs::read_to_string(&path).expect("read file"),
        None => r#"use std::collections::HashMap;

/// A point in 2D space.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    const ORIGIN: Point = Point { x: 0.0, y: 0.0 };

    pub fn dist(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        (dx * dx + 1.0).sqrt()
    }
}

fn main() {
    let mut map: HashMap<String, Point> = HashMap::new();
    let p = Point { x: 1.5, y: -2.0 };
    map.insert("key".to_string(), p);
    // TODO: handle errors
    for (k, v) in &map {
        println!("{} -> {}", k, v.x);
    }
    if let Some(first) = map.values().next() {
        let d = first.dist(&Point::ORIGIN);
        println!("dist = {d:.2}");
    }
}
"#
        .to_string(),
    };

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(&text, None).unwrap();

    let errors = ts_registry::parse_text("paintdump", &text).unwrap_or_default();
    if !errors.is_empty() {
        println!("PARSE ERRORS: {:?}", errors);
    }

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut winner: Vec<Option<&str>> = vec![None; text.len()];
    let mut iter = cursor.captures(&query, tree.root_node(), text.as_bytes());
    let dump = std::env::var("DUMP_RAW").is_ok();
    while let Some((m, _)) = iter.next() {
        for cap in m.captures() {
            let n = cap.node;
            let scope = query.capture_names()[cap.index as usize];
            if dump {
                println!(
                    "cap @{:<9} pattern={:<3} {:?}",
                    format!("{}..{}", n.start_byte(), n.end_byte()),
                    m.pattern_index,
                    &text[n.start_byte().min(text.len())..n.end_byte().min(text.len())],
                );
            }
            for slot in winner.iter_mut().take(n.end_byte()).skip(n.start_byte()) {
                *slot = Some(scope);
            }
        }
    }

    // Print per line, annotating each word with [scope].
    println!("winner[0..8] = {:?}", &winner[0..8]);
    let mut byte = 0usize;
    for line in text.lines() {
        let mut out = String::new();
        let mut words: Vec<(usize, usize)> = Vec::new();
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
                let s = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                words.push((s, i));
            } else {
                i += 1;
            }
        }
        let line_start = byte;
        for (s, e) in words {
            let gs = line_start + s;
            let ge = line_start + e;
            let scope = winner[gs..ge].iter().find(|w| w.is_some()).and_then(|w| *w);
            out.push_str(&format!("{}{} ", &line[s..e], scope.map(|sc| format!("[{sc}]")).unwrap_or_default()));
        }
        println!("{}", out.trim_end());
        byte += line.len() + 1;
    }
}
