//! Janet API for tree-sitter — parse buffers and run queries.
//! Registered as `extern "C-unwind"` functions via evil-janet.

use evil_janet::*;
use super::conv;
use super::with_editor;
use std::sync::Mutex;
use streaming_iterator::StreamingIterator;

/// A tree-sitter Language is a raw pointer that is `Send + Sync`.
/// We store loaded languages in a global registry so they live for
/// the entire editor session.
static TS_LANGUAGES: Mutex<Option<std::collections::HashMap<String, tree_sitter::Language>>> =
    Mutex::new(None);

fn with_language_map<F, R>(f: F) -> R
where
    F: FnOnce(&mut std::collections::HashMap<String, tree_sitter::Language>) -> R,
{
    let mut guard = TS_LANGUAGES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(std::collections::HashMap::new);
    f(map)
}

/// Parse a buffer's content with tree-sitter.
/// Does NOT cache the tree; call `ts/query` to parse and query in one step.
/// Returns true if the buffer has a language set.
///
/// (ts/parse buf-id) → bool
unsafe extern "C-unwind" fn c_ts_parse(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        if !ed.buffers.contains(buf_id) {
            return conv::boolean(false);
        }
        let has_lang = ed.ts_languages.contains_key(&buf_id);
        conv::boolean(has_lang)
    })
}

/// Parse and query a buffer in one step.
/// Returns an array of (start-byte end-byte scope-name) tuples.
///
/// (ts/query buf-id query-string) → [[start end scope] ...] or nil
unsafe extern "C-unwind" fn c_ts_query(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(query_str) = conv::get_str(argc, argv, 1) else {
            return conv::nil();
        };
        let Some(buf) = ed.buffers.get(buf_id) else {
            return conv::nil();
        };
        let lang_name = match ed.ts_languages.get(&buf_id) {
            Some(n) => n.clone(),
            None => return conv::nil(),
        };
        let text = buf.slice(0, buf.len()).to_string();
        let text_bytes = text.as_bytes();

        let result = with_language_map(|map| {
            let language = map.get(&lang_name)?;
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(language).ok()?;
            let tree = parser.parse(text_bytes, None)?;
            let query = tree_sitter::Query::new(language, &query_str).ok()?;
            let root = tree.root_node();
            let mut cursor = tree_sitter::QueryCursor::new();
            let capture_names = query.capture_names().iter().map(|s| s.to_string()).collect::<Vec<_>>();
            let mut query_captures = cursor.captures(&query, root, text_bytes);
            let mut result: Vec<(usize, usize, String)> = Vec::new();
            while let Some((matched, _capture_idx)) = query_captures.next() {
                for cap in matched.captures {
                    let start = cap.node.start_byte();
                    let end = cap.node.end_byte();
                    if start >= end { continue; }
                    let name_idx = cap.index as usize;
                    let scope_name = if name_idx < capture_names.len() {
                        capture_names[name_idx].clone()
                    } else {
                        "unknown".to_string()
                    };
                    result.push((start, end, scope_name));
                }
            }
            Some(result)
        });

        let result = match result {
            Some(r) => r,
            None => return conv::nil(),
        };

        let result_arr = janet_array(64);
        for (start, end, scope_name) in &result {
            let arr = janet_array(3);
            janet_array_push(arr, conv::integer(*start as i32));
            janet_array_push(arr, conv::integer(*end as i32));
            janet_array_push(arr, conv::string(scope_name));
            janet_array_push(result_arr, janet_wrap_array(arr));
        }
        janet_wrap_array(result_arr)
    })
}

/// Load a tree-sitter grammar from a shared library.
/// The library must export `tree_sitter_<language>` or `tree_sitter_language`.
///
/// (ts/load-grammar path name) → true or signals error
unsafe extern "C-unwind" fn c_ts_load_grammar(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|_ed| unsafe {
        let Some(path_str) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("ts/load-grammar requires a path")
        };
        let Some(name) = conv::get_str(argc, argv, 1) else {
            conv::signal_err("ts/load-grammar requires a language name")
        };

        match load_grammar_from_lib(&path_str) {
            Ok(lang) => {
                with_language_map(|map| {
                    map.insert(name.clone(), lang);
                });
                conv::boolean(true)
            }
            Err(e) => conv::signal_err(&format!("ts/load-grammar: {}", e)),
        }
    })
}

unsafe fn load_grammar_from_lib(path: &str) -> Result<tree_sitter::Language, String> {
    let lib = libloading::Library::new(path)
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

/// Set the tree-sitter language name for a buffer.
/// (ts/set-language buf-id lang-name) → nil
unsafe extern "C-unwind" fn c_ts_set_language(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(lang) = conv::get_str(argc, argv, 1) else {
            return conv::nil();
        };
        if ed.buffers.contains(buf_id) {
            ed.ts_languages.insert(buf_id, lang);
        }
        conv::nil()
    })
}

/// Check if a buffer has a tree-sitter language set.
/// (ts/has-tree? buf-id) → bool
unsafe extern "C-unwind" fn c_ts_has_tree(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        conv::boolean(ed.ts_languages.contains_key(&buf_id))
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"ts/parse".as_ptr() as *const _,
            cfun: Some(c_ts_parse as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Check if buffer can be parsed by tree-sitter".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ts/query".as_ptr() as *const _,
            cfun: Some(c_ts_query as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Parse buffer and run a tree-sitter query".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ts/load-grammar".as_ptr() as *const _,
            cfun: Some(c_ts_load_grammar as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Load a tree-sitter grammar from a shared library".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ts/set-language".as_ptr() as *const _,
            cfun: Some(c_ts_set_language as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the tree-sitter language for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ts/has-tree?".as_ptr() as *const _,
            cfun: Some(c_ts_has_tree as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Check if a buffer has a tree-sitter language set".as_ptr() as *const _,
        },
    ]
}
