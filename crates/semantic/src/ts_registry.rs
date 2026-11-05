//! Tree-sitter language grammar registry.
//!
//! Grammars are loaded at runtime from shared libraries via `ts/load-grammar`.
//! The registry lives here (in the semantic module) so that both the
//! scripting bridge and the TreesitterLanguageProvider can share it without
//! a layering violation.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use streaming_iterator::StreamingIterator;

pub static TS_LANGUAGES: Mutex<Option<HashMap<String, tree_sitter::Language>>> = Mutex::new(None);

/// Compiled queries, keyed by (language, query source).  Compiling a
/// highlight query is not free and the highlight path runs one per landed
/// tree — the source is stable per mode, so the cache stays tiny.  Dropped
/// whenever a grammar is (re)registered, since a query is bound to the
/// language it was compiled against.
static COMPILED_QUERIES: LazyLock<Mutex<HashMap<(String, String), Arc<tree_sitter::Query>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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
    COMPILED_QUERIES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    with_language_map(|m| {
        m.insert(name.to_string(), language);
    });
}

/// Check whether a grammar is registered.
pub fn has_language(name: &str) -> bool {
    with_language_map(|m| m.contains_key(name))
}

/// Compile-check a query against the named grammar without running it.
/// Registration-time validation so a broken query is reported when its
/// language loads, not discovered as silent missing highlights later.
pub fn validate_query(lang_name: &str, query_src: &str) -> Result<(), String> {
    with_language_map(|map| {
        let Some(language) = map.get(lang_name) else {
            return Err(format!("grammar {lang_name} is not registered"));
        };
        tree_sitter::Query::new(language, query_src)
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
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
            if !cursor.goto_next_sibling() {
                break;
            }
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
    let language = with_language_map(|map| map.get(lang_name).cloned())?;
    let query = compiled_query(&language, lang_name, query_src)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(text.as_bytes(), None)?;
    Some(query_tree(&query, &tree, text.as_bytes()))
}

/// Run `query_src` against an already-parsed tree of `text` — the
/// highlight path's cheap case: a tree that already describes the current
/// text needs no reparse before it can be queried.
pub fn query_parsed(
    lang_name: &str,
    tree: &tree_sitter::Tree,
    text: &str,
    query_src: &str,
) -> Option<Vec<(usize, usize, String)>> {
    let language = with_language_map(|map| map.get(lang_name).cloned())?;
    let query = compiled_query(&language, lang_name, query_src)?;
    Some(query_tree(&query, tree, text.as_bytes()))
}

/// The compiled form of `query_src`, from the cache when possible.
fn compiled_query(
    language: &tree_sitter::Language,
    lang_name: &str,
    query_src: &str,
) -> Option<Arc<tree_sitter::Query>> {
    let key = (lang_name.to_string(), query_src.to_string());
    let mut cache = COMPILED_QUERIES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(query) = cache.get(&key) {
        return Some(query.clone());
    }
    let query = Arc::new(tree_sitter::Query::new(language, query_src).ok()?);
    cache.insert(key, query.clone());
    Some(query)
}

fn query_tree(
    query: &tree_sitter::Query,
    tree: &tree_sitter::Tree,
    text_bytes: &[u8],
) -> Vec<(usize, usize, String)> {
    let root = tree.root_node();
    let mut cursor = tree_sitter::QueryCursor::new();
    let capture_names: Vec<String> = query
        .capture_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut query_caps = cursor.captures(query, root, text_bytes);
    let mut result = Vec::new();
    while let Some((m, _)) = query_caps.next() {
        for cap in m.captures() {
            let start = cap.node.start_byte();
            let end = cap.node.end_byte();
            if start >= end {
                continue;
            }
            let name_idx = cap.index as usize;
            let scope = if name_idx < capture_names.len() {
                capture_names[name_idx].clone()
            } else {
                "unknown".to_string()
            };
            result.push((start, end, scope));
        }
    }
    result
}

/// Incremental parse state cached per buffer.
/// Parse `text` into a tree off the interactive path (lane scheduler).
pub fn parse_tree(lang_name: &str, text: &str) -> Option<tree_sitter::Tree> {
    with_language_map(|map| {
        let language = map.get(lang_name)?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).ok()?;
        parser.parse(text, None)
    })
}

/// Parse a rope without materializing it as a `String`: the parser's read
/// callback reads straight off the rope.  `old` enables tree-sitter's
/// incremental reparse — the tree must already have the pending edits
/// applied via `Tree::edit`.
pub fn parse_rope(
    lang_name: &str,
    rope: &text::rope::Rope,
    old: Option<&tree_sitter::Tree>,
) -> Option<tree_sitter::Tree> {
    with_language_map(|map| {
        let language = map.get(lang_name)?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).ok()?;
        parser.parse_with_options(
            &mut |offset: usize, _: tree_sitter::Point| -> &[u8] {
                rope.read_from(offset).as_bytes()
            },
            old,
            None,
        )
    })
}

#[derive(Clone)]
pub struct TsTreeState {
    pub tree: tree_sitter::Tree,
    /// Document revision the tree was parsed at.
    pub revision: u64,
    /// Buffer transaction sequence the tree was parsed at — the incremental
    /// base for `txns_since`.  Zero when unknown (the query-side cache
    /// stamps it when it can).
    pub txn_seq: u64,
}

/// Incrementally reparse: apply `edits` to `old_tree`, then reparse only
/// the changed regions against the rope (no whole-document copy) and run
/// the query on the updated tree (docs/core-redesign.md §7.2).
/// `query_text` must be the buffer's current full text (queries with
/// predicates need contiguous bytes).
#[allow(clippy::type_complexity)]
pub fn highlight_incremental(
    lang_name: &str,
    rope: &text::rope::Rope,
    query_text: &str,
    query_src: &str,
    edits: &[tree_sitter::InputEdit],
    old: Option<TsTreeState>,
) -> Option<(Vec<(usize, usize, String)>, TsTreeState)> {
    let language = with_language_map(|map| map.get(lang_name).cloned())?;
    let query = compiled_query(&language, lang_name, query_src)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).ok()?;

    let old_tree = old.map(|mut state| {
        for e in edits {
            state.tree.edit(e);
        }
        state.tree
    });

    let tree = parser.parse_with_options(
        &mut |offset: usize, _: tree_sitter::Point| -> &[u8] {
            rope.read_from(offset).as_bytes()
        },
        old_tree.as_ref(),
        None,
    )?;

    let ranges = query_tree(&query, &tree, query_text.as_bytes());
    // Caller stamps the buffer revision and txn_seq the tree was parsed at.
    Some((ranges, TsTreeState { tree, revision: 0, txn_seq: 0 }))
}

/// Load a tree-sitter grammar from a shared library.  `lang_name` is the
/// registry name the grammar is being registered under — the loader probes
/// the conventional `tree_sitter_<name>` export, then the export implied by
/// the library file name (`tree-sitter-<grammar>.dylib`), before the legacy
/// list.  `symbol` overrides both conventions for grammars whose C entry
/// point deviates (declared in a plugin's language spec).
/// The language function is ABI-validated by building a parser before the
/// library handle is released; an incompatible grammar is a clean error,
/// never a silent parse failure later.
/// Returns `Ok(Language)` on success or `Err(message)` on failure.
pub fn load_grammar_from_lib(
    path: &str,
    lang_name: &str,
    symbol: Option<&str>,
) -> Result<tree_sitter::Language, String> {
    let lib = unsafe { libloading::Library::new(path) }
        .map_err(|e| format!("cannot open shared library: {}", e))?;

    let probe = format!("tree_sitter_{}", lang_name.replace('-', "_"));
    let stem_probe = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim_start_matches("lib"))
        .map(|s| s.strip_prefix("tree-sitter-").unwrap_or(s))
        .map(|s| format!("tree_sitter_{}", s.replace('-', "_")));
    let mut candidates: Vec<&[u8]> = Vec::new();
    if let Some(s) = symbol {
        candidates.push(s.as_bytes());
    }
    candidates.push(probe.as_bytes());
    if let Some(stem) = stem_probe.as_deref() {
        candidates.push(stem.as_bytes());
    }
    candidates.push(b"tree_sitter_language");
    candidates.extend([
        &b"tree_sitter_rust"[..],
        b"tree_sitter_javascript",
        b"tree_sitter_python",
        b"tree_sitter_java",
        b"tree_sitter_json",
        b"tree_sitter_html",
        b"tree_sitter_bash",
    ]);
    let func_ptr: unsafe extern "C-unwind" fn() -> *const std::ffi::c_void = unsafe {
        let sym = candidates
            .iter()
            .find_map(|name| {
                lib.get::<unsafe extern "C-unwind" fn() -> *const std::ffi::c_void>(name)
                    .ok()
            })
            .ok_or_else(|| format!("cannot find language function (tried {})", probe))?;
        *sym
    };

    let lang_fn: unsafe extern "C" fn() -> *const () = unsafe { std::mem::transmute(func_ptr) };
    let lang_fn = unsafe { tree_sitter_language::LanguageFn::from_raw(lang_fn) };
    let language = tree_sitter::Language::new(lang_fn);
    // The grammar is about to be leaked into the registry; validate the ABI
    // now, while a load failure can still refuse it instead of leaving a
    // parser that silently returns `None` on every `set_language`.
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|e| format!("grammar ABI incompatible with this tree-sitter runtime: {e}"))?;
    std::mem::forget(lib);
    Ok(language)
}
