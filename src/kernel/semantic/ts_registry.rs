//! Tree-sitter language grammar registry.
//!
//! Grammars are loaded at runtime from shared libraries via `ts/load-grammar`.
//! The registry lives here (in the semantic module) so that both the
//! scripting bridge and the TreesitterLanguageProvider can share it without
//! a layering violation.

use std::collections::HashMap;
use std::sync::Mutex;

use streaming_iterator::StreamingIterator;

pub static TS_LANGUAGES: Mutex<Option<HashMap<String, tree_sitter::Language>>> =
    Mutex::new(None);

pub fn with_language_map<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashMap<String, tree_sitter::Language>) -> R,
{
    let mut guard = TS_LANGUAGES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    f(map)
}

/// Register a named language grammar.
pub fn register_language(name: &str, language: tree_sitter::Language) {
    with_language_map(|m| { m.insert(name.to_string(), language); });
}

/// Check whether a grammar is registered.
pub fn has_language(name: &str) -> bool {
    with_language_map(|m| m.contains_key(name))
}

/// Parse `text` with the named grammar, returning parse error strings.
/// Returns `None` if the grammar is not registered.
pub fn parse_text(lang_name: &str, text: &str) -> Option<Vec<String>> {
    with_language_map(|map| {
        let language = map.get(lang_name)?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).ok()?;
        let tree = parser.parse(text.as_bytes(), None)?;
        let root = tree.root_node();
        let mut errors = Vec::new();
        collect_errors(root, &mut errors);
        Some(errors)
    })
}

fn collect_errors(node: tree_sitter::Node<'_>, out: &mut Vec<String>) {
    if node.is_error() || node.is_missing() {
        out.push(format!(
            "parse error at {}:{}",
            node.start_position().row + 1,
            node.start_position().column + 1,
        ));
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_errors(cursor.node(), out);
            if !cursor.goto_next_sibling() { break; }
        }
    }
}

/// Run a tree-sitter query on `text` with the named grammar.
/// Returns capture tuples `(start_byte, end_byte, capture_name)`.
/// Returns `None` if the grammar is not registered or the query fails to compile.
pub fn run_query(
    lang_name: &str,
    text: &str,
    query_src: &str,
) -> Option<Vec<(usize, usize, String)>> {
    with_language_map(|map| {
        let language = map.get(lang_name)?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).ok()?;
        let text_bytes = text.as_bytes();
        let tree = parser.parse(text_bytes, None)?;
        let query = tree_sitter::Query::new(language, query_src).ok()?;
        let root = tree.root_node();
        let mut cursor = tree_sitter::QueryCursor::new();
        let capture_names: Vec<String> =
            query.capture_names().iter().map(|s| s.to_string()).collect();
        let mut query_caps = cursor.captures(&query, root, text_bytes);
        let mut result = Vec::new();
        while let Some((m, _)) = query_caps.next() {
            for cap in m.captures {
                let start = cap.node.start_byte();
                let end   = cap.node.end_byte();
                if start >= end { continue; }
                let name_idx = cap.index as usize;
                let scope = if name_idx < capture_names.len() {
                    capture_names[name_idx].clone()
                } else {
                    "unknown".to_string()
                };
                result.push((start, end, scope));
            }
        }
        Some(result)
    })
}

/// Load a tree-sitter grammar from a shared library.
/// Returns `Ok(Language)` on success or `Err(message)` on failure.
pub fn load_grammar_from_lib(path: &str) -> Result<tree_sitter::Language, String> {
    let lib = unsafe { libloading::Library::new(path) }
        .map_err(|e| format!("cannot open shared library: {}", e))?;

    let func_ptr: unsafe extern "C-unwind" fn() -> *const std::ffi::c_void = unsafe {
        let sym: libloading::Symbol<unsafe extern "C-unwind" fn() -> *const std::ffi::c_void> =
            lib.get(b"tree_sitter_language")
                .or_else(|_| lib.get(b"tree_sitter_rust"))
                .or_else(|_| lib.get(b"tree_sitter_javascript"))
                .or_else(|_| lib.get(b"tree_sitter_python"))
                .or_else(|_| lib.get(b"tree_sitter_java"))
                .or_else(|_| lib.get(b"tree_sitter_json"))
                .or_else(|_| lib.get(b"tree_sitter_html"))
                .or_else(|_| lib.get(b"tree_sitter_bash"))
                .map_err(|e| format!("cannot find language function: {}", e))?;
        *sym
    };

    std::mem::forget(lib);
    let lang_fn: unsafe extern "C" fn() -> *const () =
        unsafe { std::mem::transmute(func_ptr) };
    let lang_fn = unsafe { tree_sitter_language::LanguageFn::from_raw(lang_fn) };
    Ok(tree_sitter::Language::new(lang_fn))
}
