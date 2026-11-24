//! Janet API for snippets — registration and inspection of the
//! language-scoped snippet table plugins and users contribute to.
//!
//! Registered C functions:
//!   (snippet/register trigger body &opt language description) → :ok
//!   (snippet/remove trigger &opt language) → :ok or nil
//!   (snippet/list &opt language) → array of tables

use super::conv;
use super::with_editor;
use evil_janet::*;

/// `(snippet/register trigger body &opt language description)` → `:ok`
///
/// `body` is LSP snippet syntax (`$1`, `${1:placeholder}`, `$0`); a nil
/// language registers the snippet for every buffer.  Re-registering a
/// `(trigger, language)` pair replaces the earlier body.
unsafe extern "C-unwind" fn c_snippet_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let trigger = match conv::get_str(argc, argv, 0) {
            Some(t) => t,
            None => conv::signal_err("snippet/register: trigger string required"),
        };
        let body = match conv::get_str(argc, argv, 1) {
            Some(b) => b,
            None => conv::signal_err("snippet/register: body string required"),
        };
        let language = conv::get_opt_str(argc, argv, 2).flatten();
        let description = conv::get_opt_str(argc, argv, 3).flatten();
        kernel::snippet::register_snippet(ed, trigger, body, language, description);
        conv::keyword("ok")
    })
}

/// `(snippet/remove trigger &opt language)` → `:ok` or nil
unsafe extern "C-unwind" fn c_snippet_remove(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let trigger = match unsafe { conv::get_str(argc, argv, 0) } {
            Some(t) => t,
            None => return conv::nil(),
        };
        let language = unsafe { conv::get_opt_str(argc, argv, 1) }.flatten();
        if ed
            .snippet
            .registry
            .remove(&trigger, language.as_deref())
        {
            conv::keyword("ok")
        } else {
            conv::nil()
        }
    })
}

/// `(snippet/list &opt language)` → array of
/// `{:trigger :body :language :description}` tables
unsafe extern "C-unwind" fn c_snippet_list(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let language = conv::get_opt_str(argc, argv, 0).flatten();
        let snippets = ed.snippet.registry.for_language(language.as_deref());
        let arr = janet_wrap_array(janet_array(snippets.len() as i32));
        for s in snippets {
            let tbl = janet_wrap_table(janet_table(4));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("trigger"), conv::string(&s.trigger));
            janet_table_put(t, conv::keyword("body"), conv::string(&s.body));
            let lang = match &s.language {
                Some(l) => conv::string(l),
                None => conv::nil(),
            };
            janet_table_put(t, conv::keyword("language"), lang);
            let desc = match &s.description {
                Some(d) => conv::string(d),
                None => conv::nil(),
            };
            janet_table_put(t, conv::keyword("description"), desc);
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"snippet/register".as_ptr() as *const _,
            cfun: Some(
                c_snippet_register as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Register a snippet: trigger, LSP-syntax body, optional language and description"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"snippet/remove".as_ptr() as *const _,
            cfun: Some(
                c_snippet_remove as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Remove a snippet by trigger and optional language".as_ptr()
                as *const _,
        },
        JanetReg {
            name: c"snippet/list".as_ptr() as *const _,
            cfun: Some(c_snippet_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List registered snippets for a language (nil = globals only)"
                .as_ptr() as *const _,
        },
    ]
}
