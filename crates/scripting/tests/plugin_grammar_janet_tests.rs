//! End-to-end plugin grammar convention: a plugin that ships a grammar
//! shared library at `grammars/tree-sitter-<lang>.<ext>` gets it loaded
//! and registered by plugin/define → language/define, and buffers of the
//! declared extension are tagged with the tree-sitter language id.

#[macro_use]
mod helpers;
use scripting;

use std::io::Write as _;

fn grammar_lib_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "tree-sitter-tiny.dylib"
    } else if cfg!(windows) {
        "tree-sitter-tiny.dll"
    } else {
        "tree-sitter-tiny.so"
    }
}

/// A fixture plugin shipping the tiny grammar: language `tiny`, files with
/// extension `tst`, and a conventional `<lang>-mode` major mode.
fn grammar_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "magma_plugin_grammar_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    let plugin = dir.join("plugins/zzts/grammars");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::copy(
        helpers::tiny_grammar_lib().as_path(),
        plugin.join(grammar_lib_name()),
    )
    .unwrap();
    let mut manifest = std::fs::File::create(dir.join("plugins/zzts/manifest.janet")).unwrap();
    writeln!(
        manifest,
        r#"(plugin/define {{:name "zzts" :version "0.1.0" :entry "main.janet"
                            :languages ["tiny"]
                            :extensions {{"tiny" ["tst"]}}}})"#
    )
    .unwrap();
    let mut main = std::fs::File::create(dir.join("plugins/zzts/main.janet")).unwrap();
    writeln!(
        main,
        r#"(major-mode/define "tiny-mode" {{:parent "prog"}})"#
    )
    .unwrap();
    let mut syntax = std::fs::File::create(dir.join("plugins/zzts/syntax.scm")).unwrap();
    writeln!(syntax, "(source_file) @module").unwrap();
    dir
}

fn open_file_cmd(ed: &mut kernel::state::Editor, path: &str) {
    let mut args = std::collections::HashMap::new();
    args.insert(
        "path".to_string(),
        kernel::command::args::ArgValue::Path(path.to_string()),
    );
    kernel::command::execute_command(ed, "open-file", &args).unwrap();
}

#[test]
fn plugin_grammar_is_picked_up_and_registered() {
    let dir = grammar_root("e2e");
    let root = dir.to_str().unwrap().replace('\\', "/");
    janet_test!(ed, {
        // Before discovery nothing knows the language.
        let before = scripting::eval_result(
            r#"(length (filter (fn [l] (= (get l :name) "tiny")) (language/list)))"#,
        )
        .unwrap_or_default();
        assert_eq!(before, "0");

        scripting::eval(&format!(
            r#"(plugin/discover "{root}/plugins") (plugin/enable "zzts")"#
        ));

        let registered = scripting::eval_result(
            r#"(let [l (first (filter (fn [l] (= (get l :name) "tiny")) (language/list)))]
                 (string (get l :grammar-loaded) "|" (length (get l :extensions))))"#,
        )
        .unwrap_or_default();
        assert_eq!(registered, r#""true|1""#);
        assert_eq!(
            scripting::eval_result(r#"(ts/has-language "tiny")"#).unwrap_or_default(),
            "true"
        );
    });
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn opened_file_of_declared_extension_gets_the_language() {
    let dir = grammar_root("open");
    let root = dir.to_str().unwrap().replace('\\', "/");
    let file = dir.join("sample.tst");
    std::fs::write(&file, "hello world\n").unwrap();
    let file = file.to_str().unwrap().replace('\\', "/");
    janet_test!(ed, {
        scripting::eval(&format!(
            r#"(plugin/discover "{root}/plugins") (plugin/enable "zzts")"#
        ));
        open_file_cmd(&mut ed, &file);
        // buffer-created is queued: drain it so the Janet handlers run.
        ed.events.drain_and_dispatch();

        // The mode activated through the extension map on buffer-created…
        let state = scripting::eval_result(
            r#"(let [buf (buffer/current)]
                 (string (buffer/major-mode buf) "|" (ts/parse buf)))"#,
        )
        .unwrap_or_default();
        assert_eq!(state, r#""tiny-mode|true""#);

        // …and the buffer carries the language ID, not the mode name: it
        // is the key the semantic engine and providers register under.
        let buf_id: usize = scripting::eval_result("(buffer/current)")
            .unwrap_or_default()
            .trim_matches('"')
            .parse()
            .expect("buffer id");
        assert_eq!(
            ed.ts_languages.get(&buf_id).map(String::as_str),
            Some("tiny")
        );
    });
    std::fs::remove_dir_all(&dir).ok();
}
