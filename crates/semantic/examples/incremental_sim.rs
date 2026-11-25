//! Simulate typing on a Rust buffer through both incremental feed paths and
//! compare each result against a full reparse of the same text.
//!
//! Path A (foreground ts/highlight): single pure insertion fed via InputEdit,
//! everything else a full reparse — mirrors treesitter_api.rs.
//! Path B (lane schedule_tree_update): any single-edit transaction fed via
//! single_edit_input_edit — mirrors ts_schedule.rs.
//!
//! Usage: cargo run -p semantic --example incremental_sim

use semantic::ts_registry;
use std::path::Path;
use tree_sitter::StreamingIterator;

fn lang() -> tree_sitter::Language {
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let dylib = plugins.join("rust/grammars/tree-sitter-rust.dylib");
    let language = ts_registry::load_grammar_from_lib(
        dylib.to_str().unwrap(),
        "rustsim",
        None,
    )
    .expect("grammar loads");
    ts_registry::register_language("rustsim", language.clone());
    language
}

fn query_src() -> String {
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    std::fs::read_to_string(plugins.join("rust/syntax.scm")).unwrap()
}

fn captures(tree: &tree_sitter::Tree, text: &str, query: &tree_sitter::Query) -> Vec<(usize, usize, String)> {
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut out = Vec::new();
    let mut iter = cursor.captures(query, tree.root_node(), text.as_bytes());
    while let Some((m, idx)) = iter.next() {
        let n = m.captures()[*idx as usize].node;
        out.push((
            n.start_byte(),
            n.end_byte(),
            query.capture_names()[*idx as usize].to_string(),
        ));
    }
    out
}

/// The ts_schedule.rs lane feed for any single-edit transaction.
fn lane_input_edit(
    text_after: &str,
    start: usize,
    old_end: usize,
    inserted_len: usize,
    old_text: &str,
) -> tree_sitter::InputEdit {
    fn point_of(text: &str, offset: usize) -> tree_sitter::Point {
        let offset = offset.min(text.len());
        let row = text[..offset].matches('\n').count();
        let col = match text[..offset].rfind('\n') {
            Some(i) => offset - i - 1,
            None => offset,
        };
        tree_sitter::Point::new(row, col)
    }
    let start_point = point_of(text_after, start);
    let old_end_point = if old_text.is_empty() {
        start_point
    } else {
        let rows = old_text.bytes().filter(|&b| b == b'\n').count();
        match old_text.rfind('\n') {
            Some(i) => tree_sitter::Point::new(start_point.row + rows, old_text.len() - i - 1),
            None => tree_sitter::Point::new(start_point.row, start_point.column + old_text.len()),
        }
    };
    let new_end_point = if inserted_len == 0 {
        start_point
    } else {
        point_of(text_after, start + inserted_len)
    };
    tree_sitter::InputEdit {
        start_byte: start,
        old_end_byte: old_end,
        new_end_byte: start + inserted_len,
        start_position: start_point,
        old_end_position: old_end_point,
        new_end_position: new_end_point,
    }
}

fn main() {
    let language = lang();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let query = tree_sitter::Query::new(&language, &query_src()).expect("query compiles");

    let initial = "fn main() {\n    let wörld = \"héllo\";\n    println!(\"{} {}\", wörld, 42);\n}\n\nstruct Pöint {\n    x: i32,\n    y: i32,\n}\n\nimpl Pöint {\n    fn néw(x: i32, y: i32) -> Self {\n        Self { x, y }\n    }\n    // döc comment über\n    fn sum(&self) -> i32 {\n        self.x + self.y\n    }\n}\n";

    let mut text = initial.to_string();
    let mut tree = parser.parse(&text, None).unwrap();

    // Typing script: each step resolves its byte range against the CURRENT
    // text, so offsets stay valid as earlier steps shift the document.
    type Step = Box<dyn Fn(&str) -> (usize, usize, String)>;
    let steps: Vec<Step> = vec![
        // multibyte insertion in the middle of the first line
        Box::new(|t: &str| (t.find("() {").unwrap() + 4, t.find("() {").unwrap() + 4, "é".into())),
        // insertion with a newline after `let `
        Box::new(|t: &str| {
            let i = t.find("let ").unwrap();
            (i, i, "let x = 5;\n    ".into())
        }),
        // delete one byte after `fn main`
        Box::new(|_t: &str| (4, 5, "".into())),
        // replace `fn ` prefix at the top
        Box::new(|_t: &str| (0, 3, "pub ".into())),
        // newline inside the println line
        Box::new(|t: &str| {
            let i = t.find("println!").unwrap() + 8;
            (i, i, "\n".into())
        }),
        // delete the whole doc-comment line
        Box::new(|t: &str| {
            let s = t.find("    // döc").unwrap();
            let e = t[s..].find('\n').unwrap() + s + 1;
            (s, e, "".into())
        }),
        // multibyte code at the top
        Box::new(|_t: &str| (0, 0, "strúct Q;\n".into())),
        // append at end
        Box::new(|t: &str| (t.len(), t.len(), "\nfn extra() {}\n".into())),
        // replace the struct name (multibyte replacement)
        Box::new(|t: &str| {
            let s = t.find("Pöint {").unwrap();
            (s, s + "Pöint".len(), "Polar".into())
        }),
        // delete inside the string literal (hé — two bytes for é)
        Box::new(|t: &str| {
            let s = t.find("\"héllo\"").unwrap() + 1;
            (s, s + "hé".len(), "".into())
        }),
    ];

    println!("step | diffs vs full reparse");
    let mut failures = 0;
    for (i, step) in steps.iter().enumerate() {
        let (start, end, ins) = step(&text);
        let old_text = text[start..end].to_string();
        let after = format!("{}{}{}", &text[..start], ins, &text[end..]);

        // Path A: foreground — pure insertion only.
        let path_a = if old_text.is_empty() {
            let mut edited = tree.clone();
            let edit = lane_input_edit(&after, start, end, ins.len(), &old_text);
            edited.edit(&edit);
            parser
                .parse_with_options(
                    &mut |off, _| -> &[u8] { &after.as_bytes()[off.min(after.len())..] },
                    Some(&edited),
                    None,
                )
                .map(|t| captures(&t, &after, &query))
        } else {
            parser.parse(&after, None).map(|t| captures(&t, &after, &query))
        };

        // Path B: lane — always fed.
        let mut edited = tree.clone();
        let edit = lane_input_edit(&after, start, end, ins.len(), &old_text);
        edited.edit(&edit);
        let path_b = parser
            .parse_with_options(
                &mut |off, _| -> &[u8] { &after.as_bytes()[off.min(after.len())..] },
                Some(&edited),
                None,
            )
            .map(|t| captures(&t, &after, &query));

        // Ground truth.
        let truth = parser.parse(&after, None).unwrap();
        let truth_caps = captures(&truth, &after, &query);

        let text_changed = after != text;
        if path_a != Some(truth_caps.clone()) {
            failures += 1;
            let a = path_a.clone().unwrap_or_default();
            println!(
                "step {i} PATH-A MISMATCH: inc={} caps, truth={} caps (text_changed={text_changed})",
                a.len(),
                truth_caps.len()
            );
            for (ai, (ta, tb)) in a.iter().zip(truth_caps.iter()).enumerate() {
                if ta != tb {
                    println!("  first diff at #{ai}: inc={ta:?} truth={tb:?}");
                    break;
                }
            }
        }
        if path_b != Some(truth_caps.clone()) {
            failures += 1;
            let b = path_b.clone().unwrap_or_default();
            println!(
                "step {i} PATH-B MISMATCH: inc={} caps, truth={} caps",
                b.len(),
                truth_caps.len()
            );
            for (bi, (tb_, tt)) in b.iter().zip(truth_caps.iter()).enumerate() {
                if tb_ != tt {
                    println!("  first diff at #{bi}: inc={tb_:?} truth={tt:?}");
                    break;
                }
            }
        }

        text = after;
        tree = truth;
    }

    if failures == 0 {
        println!("all steps: incremental captures identical to full reparse");
    }
}
