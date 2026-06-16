//! Janet API for tree-sitter — parse buffers and run queries.
//! Registered as `extern "C-unwind"` functions via evil-janet.
//!
//! The tree-sitter language grammar registry now lives in
//! `kernel/semantic/ts_registry` so that both the scripting bridge and the
//! TreesitterLanguageProvider can share it without a layering violation.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::semantic::ts_registry;

/// Parse a buffer's content with tree-sitter.
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
        let lang_name = match ed.ts_languages.get(&buf_id) {
            Some(n) => n.clone(),
            None => return conv::nil(),
        };
        let text = match ed.buffers.get(buf_id) {
            Some(arc) => {
                let buf = arc.lock().unwrap();
                buf.slice(0, buf.len()).to_string()
            }
            None => return conv::nil(),
        };

        let result = ts_registry::run_query(&lang_name, &text, &query_str);
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

        match ts_registry::load_grammar_from_lib(&path_str) {
            Ok(lang) => {
                ts_registry::register_language(&name, lang);
                conv::boolean(true)
            }
            Err(e) => conv::signal_err(&format!("ts/load-grammar: {}", e)),
        }
    })
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
