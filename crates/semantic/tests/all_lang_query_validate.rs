//! Every bundled plugin that ships a grammar must also ship a highlight
//! query that compiles against that grammar.  A query that fails to compile
//! takes the mode's syntax layer away from the fallback highlighter and then
//! produces nothing — the user sees a file with no colors at all.

use semantic::ts_registry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn ext() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    }
}

fn plugins_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

/// `:grammar-symbols {"terraform" "tree_sitter_hcl"}` declares the C entry
/// point for grammars whose export deviates from `tree_sitter_<lang>`.
fn declared_symbols(manifest: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(start) = manifest.find(":grammar-symbols") else {
        return out;
    };
    let rest = &manifest[start..];
    let (Some(open), Some(close)) = (rest.find('{'), rest.find('}')) else {
        return out;
    };
    let quoted: Vec<&str> = rest[open + 1..close]
        .split('"')
        .skip(1)
        .step_by(2)
        .collect();
    for pair in quoted.chunks(2) {
        if let [lang, symbol] = pair {
            out.insert((*lang).to_string(), (*symbol).to_string());
        }
    }
    out
}

fn bundled_languages() -> Vec<String> {
    let mut langs: Vec<String> = std::fs::read_dir(plugins_root())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let dir = e.path();
            let lang = dir.file_name()?.to_str()?.to_string();
            let grammar = dir.join(format!("grammars/tree-sitter-{lang}.{}", ext()));
            (grammar.exists() && dir.join("syntax.scm").exists()).then_some(lang)
        })
        .collect();
    langs.sort();
    langs
}

fn validate(lang: &str) -> Result<(), String> {
    let dir = plugins_root().join(lang);
    let manifest = std::fs::read_to_string(dir.join("manifest.janet"))
        .map_err(|e| format!("manifest: {e}"))?;
    let symbol = declared_symbols(&manifest).remove(lang);
    let dylib = dir.join(format!("grammars/tree-sitter-{lang}.{}", ext()));

    let registry_name = format!("bundled_{lang}");
    let language = ts_registry::load_grammar_from_lib(
        dylib.to_str().unwrap(),
        &registry_name,
        symbol.as_deref(),
    )
    .map_err(|e| format!("grammar load: {e}"))?;
    ts_registry::register_language(&registry_name, language);

    let src =
        std::fs::read_to_string(dir.join("syntax.scm")).map_err(|e| format!("query read: {e}"))?;
    ts_registry::validate_query(&registry_name, &src)
}

#[test]
fn every_bundled_repository_query_compiles() {
    let mut broken = Vec::new();
    for lang in bundled_languages() {
        match validate(&lang) {
            Ok(()) => println!("{lang}: query compiles"),
            Err(e) => {
                println!("{lang}: BROKEN: {e}");
                broken.push(format!("{lang}: {e}"));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "{} bundled query/grammar pair(s) broken:\n{}",
        broken.len(),
        broken.join("\n")
    );
}
