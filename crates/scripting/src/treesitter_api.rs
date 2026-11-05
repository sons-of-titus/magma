//! Janet API for tree-sitter — parse buffers and run queries.
//! Registered as `extern "C-unwind"` functions via evil-janet.
//!
//! The tree-sitter language grammar registry now lives in
//! `kernel/semantic/ts_registry` so that both the scripting bridge and the
//! TreesitterLanguageProvider can share it without a layering violation.

use super::conv;
use super::with_editor;
use evil_janet::*;
use kernel::semantic::ts_registry;

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
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
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

        match ts_registry::load_grammar_from_lib(&path_str, &name, None) {
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
            ed.ts_trees.remove(&buf_id);
        }
        conv::nil()
    })
}

/// Run `query_str` against the buffer's fresh-or-cached tree, updating the
/// per-buffer tree state.  Returns (start, end, capture) triples with the
/// capture names verbatim — the query's semantics belong to the caller.
fn run_incremental_query(
    ed: &mut kernel::state::Editor,
    buf_id: usize,
    query_str: &str,
) -> Option<Vec<(usize, usize, String)>> {
    use kernel::semantic::ts_registry;

    fn point_of(buf: &kernel::text_engine::Buffer, offset: usize) -> tree_sitter::Point {
        let (row, col) = buf.line_col_bytes(offset);
        tree_sitter::Point::new(row, col)
    }

    let lang_name = ed.ts_languages.get(&buf_id).cloned()?;
    let arc = ed.buffers.get(buf_id)?;

    let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
    let current_rev = buf.revision();
    let mut cached = ed
        .ts_trees
        .get(&buf_id)
        .filter(|s| s.revision <= current_rev)
        .cloned();
    let mut edits: Vec<tree_sitter::InputEdit> = Vec::new();
    if let Some(state) = &cached {
        match buf.changes_since(state.revision) {
            Some(changes) => {
                let spans: Vec<&(usize, usize, usize)> =
                    changes.iter().flat_map(|(_, _, s)| s.iter()).collect();
                if spans.is_empty() {
                    // The tree already describes this text (the caller
                    // queries it directly below).
                } else if let [&(start, old_end, new_end)] = spans[..] {
                    // Only a pure insertion is feedable: its points read
                    // straight off the current text.  A deletion or
                    // replacement needs the displaced text's points, which
                    // cannot be read from the buffer — feeding tree-sitter
                    // positions that disagree with its byte offsets corrupts
                    // the tree, so those discard the cache and reparse whole
                    // (as does a multi-edit feed, whose intermediate text
                    // states are gone).
                    if old_end == start {
                        edits.push(tree_sitter::InputEdit {
                            start_byte: start,
                            old_end_byte: old_end,
                            new_end_byte: new_end,
                            start_position: point_of(&buf, start),
                            old_end_position: point_of(&buf, start),
                            new_end_position: point_of(&buf, new_end.min(buf.len())),
                        });
                    } else {
                        cached = None;
                    }
                } else {
                    cached = None;
                }
            }
            // The tree predates the retention window: no honest
            // incremental feed exists, so discard it and force a full
            // reparse.
            None => cached = None,
        }
    }
    let query_text = buf.slice(0, buf.len());
    let current_seq = buf.txn_seq();

    // A cached tree at the current revision already describes this text:
    // querying it directly is the cheap path every landed parse takes.
    if cached.is_some() && edits.is_empty() {
        let state = cached.take().expect("checked above");
        let result = ts_registry::query_parsed(&lang_name, &state.tree, &query_text, query_str)?;
        drop(buf);
        ed.ts_trees.insert(
            buf_id,
            ts_registry::TsTreeState {
                tree: state.tree,
                revision: current_rev,
                txn_seq: current_seq,
            },
        );
        return Some(result);
    }

    let (result, mut state) = ts_registry::highlight_incremental(
        &lang_name,
        buf.rope_handle(),
        &query_text,
        query_str,
        &edits,
        cached,
    )?;
    drop(buf);
    state.revision = current_rev;
    state.txn_seq = current_seq;
    ed.ts_trees.insert(buf_id, state);
    Some(result)
}

fn triples_to_janet(result: &[(usize, usize, String)]) -> Janet {
    unsafe {
        let result_arr = janet_array(result.len().clamp(1, 4096) as i32);
        for (start, end, scope) in result {
            let arr = janet_array(3);
            janet_array_push(arr, conv::integer(*start as i32));
            janet_array_push(arr, conv::integer(*end as i32));
            janet_array_push(arr, conv::string(scope));
            janet_array_push(result_arr, janet_wrap_array(arr));
        }
        janet_wrap_array(result_arr)
    }
}

/// Incremental highlight: reuse the cached parse tree, apply the edits
/// since its revision, reparse only the changed regions, and run the query.
/// Falls back to a full parse when there is no usable cached tree.
///
/// (ts/highlight buf-id query-string) → [[start end scope] ...] or nil
unsafe extern "C-unwind" fn c_ts_highlight(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(query_str) = conv::get_str(argc, argv, 1) else {
            return conv::nil();
        };
        match run_incremental_query(ed, buf_id, &query_str) {
            Some(result) => triples_to_janet(&result),
            None => conv::nil(),
        }
    })
}

/// Incremental arbitrary query: the same cached-tree path as ts/highlight,
/// but capture names come back verbatim (no scope promotion).  This is the
/// general mechanism behind the per-mode query features — folds, and later
/// textobjects or indents — each driven by its own query kind.
///
/// (ts/query-tree buf-id query-string) → [[start end capture] ...] or nil
unsafe extern "C-unwind" fn c_ts_query_tree(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(query_str) = conv::get_str(argc, argv, 1) else {
            return conv::nil();
        };
        match run_incremental_query(ed, buf_id, &query_str) {
            Some(result) => triples_to_janet(&result),
            None => conv::nil(),
        }
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

/// Compile-check a query against a loaded grammar without running it.
/// Accepts either a language id or its `<id>-mode` name.
///
/// (ts/validate-query lang-or-mode query) → bool
unsafe extern "C-unwind" fn c_ts_validate_query(argc: i32, argv: *mut Janet) -> Janet {
    unsafe {
        let Some(mut lang) = conv::get_str(argc, argv, 0) else {
            return conv::boolean(false);
        };
        let Some(query) = conv::get_str(argc, argv, 1) else {
            return conv::boolean(false);
        };
        if !ts_registry::has_language(&lang) {
            if let Some(id) = lang.strip_suffix("-mode") {
                lang = id.to_string();
            }
        }
        conv::boolean(ts_registry::validate_query(&lang, &query).is_ok())
    }
}

/// Check whether a grammar is loaded in the tree-sitter registry.
/// (ts/has-language name) → bool
unsafe extern "C-unwind" fn c_ts_has_language(argc: i32, argv: *mut Janet) -> Janet {
    unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::boolean(false);
        };
        conv::boolean(ts_registry::has_language(&name))
    }
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
            name: c"ts/highlight".as_ptr() as *const _,
            cfun: Some(c_ts_highlight as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Incrementally highlight a buffer with a query".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ts/query-tree".as_ptr() as *const _,
            cfun: Some(c_ts_query_tree as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Run a query against the buffer's cached tree, captures verbatim"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"ts/has-tree?".as_ptr() as *const _,
            cfun: Some(c_ts_has_tree as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Check if a buffer has a tree-sitter language set".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ts/validate-query".as_ptr() as *const _,
            cfun: Some(
                c_ts_validate_query as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Compile-check a query against a loaded grammar".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ts/has-language".as_ptr() as *const _,
            cfun: Some(c_ts_has_language as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Check whether a grammar is loaded in the tree-sitter registry".as_ptr()
                as *const _,
        },
    ]
}
