//! Parse files with a bundled grammar dylib and report ERROR/MISSING nodes.
//!
//! Usage: cargo run -p semantic --example parse_check -- <lang> <file>…

use semantic::ts_registry;
use std::path::Path;

fn main() {
    let mut args = std::env::args().skip(1);
    let lang = args.next().expect("usage: parse_check <lang> <file>…");
    let files: Vec<String> = args.collect();
    if files.is_empty() {
        eprintln!("usage: parse_check <lang> <file>…");
        std::process::exit(2);
    }

    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let dylib = plugins
        .join(&lang)
        .join(format!("grammars/tree-sitter-{lang}.dylib"));
    let language = ts_registry::load_grammar_from_lib(
        dylib.to_str().expect("non-utf8 dylib path"),
        &lang,
        None,
    )
    .expect("grammar loads");
    ts_registry::register_language(&lang, language);

    for file in &files {
        let text = std::fs::read_to_string(file).expect("read file");
        let errors = ts_registry::parse_text(&lang, &text).unwrap_or_default();
        println!("{}: {} error node(s)", file, errors.len());
        for e in errors.iter().take(10) {
            println!("  {}", e);
        }
    }
}
