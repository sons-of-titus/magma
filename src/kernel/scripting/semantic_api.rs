//! Janet API for the Semantic Engine.
//!
//! Registered C functions:
//!   (semantic/symbols buf-id)                   → array of [name kind file line col]
//!   (semantic/definitions name)                  → array of [name kind file line col]
//!   (semantic/references name)                   → array of [name file line col]
//!   (semantic/documentation symbol)              → string or nil
//!   (semantic/index-buffer buf-id)               → nil
//!   (semantic/register-provider lang type)       → nil
//!   (semantic/diagnostics buf-id)               → array of [line col severity message]

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::semantic::{LspLanguageProvider, TreesitterLanguageProvider};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Push a symbol entry `[name kind file line col]` onto `arr`.
unsafe fn push_symbol_entry(
    arr: *mut JanetArray,
    name: &str,
    kind: &str,
    file: &str,
    line: usize,
    col: usize,
) {
    unsafe {
        let entry = janet_array(5);
        janet_array_push(entry, conv::string(name));
        janet_array_push(entry, conv::string(kind));
        janet_array_push(entry, conv::string(file));
        janet_array_push(entry, conv::integer(line as i32));
        janet_array_push(entry, conv::integer(col as i32));
        janet_array_push(arr, janet_wrap_array(entry));
    }
}

// ── C functions ──────────────────────────────────────────────────────────────

/// (semantic/symbols buf-id) → [[name kind file line col] ...]
/// Query live symbols from the provider for the buffer's language.
unsafe extern "C-unwind" fn c_semantic_symbols(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(lang) = ed.ts_languages.get(&buf_id).cloned() else {
            return janet_wrap_array(janet_array(0));
        };
        let text = match ed.buffers.get(buf_id) {
            Some(arc) => { let b = arc.lock().unwrap(); b.slice(0, b.len()).to_string() }
            None => return janet_wrap_array(janet_array(0)),
        };
        let symbols = ed.semantic.symbols_for_buffer(&lang, &text);
        let arr = janet_array(symbols.len() as i32);
        for sym in &symbols {
            push_symbol_entry(arr, &sym.name, sym.kind.as_str(), &sym.file, sym.line, sym.column);
        }
        janet_wrap_array(arr)
    })
}

/// (semantic/definitions name) → [[name kind file line col] ...]
/// Look up symbol definitions from the index.
unsafe extern "C-unwind" fn c_semantic_definitions(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return janet_wrap_array(janet_array(0));
        };
        let defs = ed.semantic.definitions(&name);
        let arr = janet_array(defs.len() as i32);
        for sym in &defs {
            push_symbol_entry(arr, &sym.name, sym.kind.as_str(), &sym.file, sym.line, sym.column);
        }
        janet_wrap_array(arr)
    })
}

/// (semantic/references name) → [[name file line col] ...]
unsafe extern "C-unwind" fn c_semantic_references(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return janet_wrap_array(janet_array(0));
        };
        let refs = ed.semantic.references(&name);
        let arr = janet_array(refs.len() as i32);
        for r in &refs {
            let entry = janet_array(4);
            janet_array_push(entry, conv::string(&r.name));
            janet_array_push(entry, conv::string(&r.file));
            janet_array_push(entry, conv::integer(r.line as i32));
            janet_array_push(entry, conv::integer(r.column as i32));
            janet_array_push(arr, janet_wrap_array(entry));
        }
        janet_wrap_array(arr)
    })
}

/// (semantic/documentation symbol) → string or nil
unsafe extern "C-unwind" fn c_semantic_documentation(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        match ed.semantic.documentation(&name) {
            Some(doc) => conv::string(&doc),
            None => conv::nil(),
        }
    })
}

/// (semantic/index-buffer buf-id) → nil
/// Re-index the buffer's symbols (using its registered tree-sitter language).
unsafe extern "C-unwind" fn c_semantic_index_buffer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(lang) = ed.ts_languages.get(&buf_id).cloned() else {
            return conv::nil();
        };
        let (text, path) = match ed.buffers.get(buf_id) {
            Some(arc) => {
                let b = arc.lock().unwrap();
                (b.slice(0, b.len()).to_string(), b.path.clone().unwrap_or_default())
            }
            None => return conv::nil(),
        };
        ed.semantic.index_buffer(&lang, &path, &text);
        conv::nil()
    })
}

/// (semantic/register-provider lang type) → nil
/// `type` is "lsp" or "treesitter".
unsafe extern "C-unwind" fn c_semantic_register_provider(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(lang) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        let provider_type = conv::get_str(argc, argv, 1).unwrap_or_else(|| "lsp".to_string());
        let provider: Box<dyn crate::kernel::semantic::LanguageProvider> = match provider_type.as_str() {
            "treesitter" | "ts" =>
                Box::new(TreesitterLanguageProvider::new(lang.clone())),
            _ =>
                Box::new(LspLanguageProvider::new(lang.clone())),
        };
        ed.semantic.register_provider(&lang, provider);
        conv::nil()
    })
}

/// (semantic/diagnostics buf-id) → [[line col severity message] ...]
/// Return typed diagnostics for the buffer's file path.
unsafe extern "C-unwind" fn c_semantic_diagnostics(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let buf_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let path = match ed.buffers.get(buf_id) {
            Some(arc) => {
                let b = arc.lock().unwrap();
                b.path.clone().unwrap_or_default()
            }
            None => return janet_wrap_array(janet_array(0)),
        };
        let diags = ed.semantic.diagnostics_for_path(&path);
        let arr = janet_array(diags.len() as i32);
        for d in &diags {
            let entry = janet_array(4);
            janet_array_push(entry, conv::integer(d.line as i32));
            janet_array_push(entry, conv::integer(d.column as i32));
            janet_array_push(entry, conv::string(d.severity.as_str()));
            janet_array_push(entry, conv::string(&d.message));
            janet_array_push(arr, janet_wrap_array(entry));
        }
        janet_wrap_array(arr)
    })
}

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"semantic/symbols".as_ptr() as *const _,
            cfun: Some(c_semantic_symbols as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return live symbols for a buffer from its language provider".as_ptr() as *const _,
        },
        JanetReg {
            name: c"semantic/definitions".as_ptr() as *const _,
            cfun: Some(c_semantic_definitions as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Look up symbol definitions from the index".as_ptr() as *const _,
        },
        JanetReg {
            name: c"semantic/references".as_ptr() as *const _,
            cfun: Some(c_semantic_references as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Look up symbol references from the index".as_ptr() as *const _,
        },
        JanetReg {
            name: c"semantic/documentation".as_ptr() as *const _,
            cfun: Some(c_semantic_documentation as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return documentation for a symbol from the index".as_ptr() as *const _,
        },
        JanetReg {
            name: c"semantic/index-buffer".as_ptr() as *const _,
            cfun: Some(c_semantic_index_buffer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Index the buffer's symbols into the semantic engine".as_ptr() as *const _,
        },
        JanetReg {
            name: c"semantic/register-provider".as_ptr() as *const _,
            cfun: Some(c_semantic_register_provider as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a language provider: (semantic/register-provider lang-id type) where type is lsp or treesitter".as_ptr() as *const _,
        },
        JanetReg {
            name: c"semantic/diagnostics".as_ptr() as *const _,
            cfun: Some(c_semantic_diagnostics as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return typed diagnostics for a buffer's file".as_ptr() as *const _,
        },
    ]
}
