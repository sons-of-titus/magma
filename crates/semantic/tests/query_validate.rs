use semantic::ts_registry;

#[test]
fn rust_plugin_query_compiles() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let dylib =
        std::path::Path::new(manifest).join("../../plugins/rust/grammars/tree-sitter-rust.dylib");
    let dylib = if dylib.exists() {
        dylib
    } else {
        std::path::Path::new(manifest).join("../../plugins/rust/grammars/tree-sitter-rust.so")
    };
    let lang = ts_registry::load_grammar_from_lib(dylib.to_str().unwrap(), "rustvalidate", None)
        .expect("grammar loads");
    ts_registry::register_language("rustvalidate", lang);
    let src = std::fs::read_to_string(
        std::path::Path::new(manifest).join("../../plugins/rust/syntax.scm"),
    )
    .unwrap();
    match ts_registry::validate_query("rustvalidate", &src) {
        Ok(()) => println!("QUERY COMPILES"),
        Err(e) => panic!("QUERY ERROR: {e}"),
    }
}
