//! Janet API for the native fallback syntax highlighter's rule registry.
//!
//! Registered C functions:
//!   (syntax/define-rules lang keywords comment-marker) → :ok
//!   (syntax/map-extension extension lang)              → :ok
//!   (syntax/set-ts-mode mode enabled)                  → nil
//!
//! Keyword tables and extension maps stay Janet configuration; this is the
//! registration surface into the kernel module that owns scanning and the
//! incremental range cache.

use super::conv;
use super::with_editor;
use evil_janet::*;

/// `(syntax/define-rules lang keywords comment-marker)` → `:ok`
///
/// Replace the fallback rules for `lang`: `keywords` is an array of exact
/// word matches, `comment-marker` a line-comment prefix or nil.  An empty
/// language name, a non-array keyword list, a non-string keyword, or an
/// empty marker signals an error.  Re-registration replaces the previous
/// rules and invalidates cached ranges for that language.
unsafe extern "C-unwind" fn c_syntax_define_rules(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(lang) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("syntax/define-rules: language name required");
        };
        if lang.is_empty() {
            conv::signal_err("syntax/define-rules: language name must not be empty");
        }
        let Some(keywords) = get_keyword_list(argc, argv, 1) else {
            conv::signal_err("syntax/define-rules: keywords must be an array of strings");
        };
        let comment_marker = conv::get_str(argc, argv, 2);
        if comment_marker.as_deref().is_some_and(str::is_empty) {
            conv::signal_err("syntax/define-rules: comment marker must not be empty");
        }
        ed.fallback_syntax
            .define_rules(&lang, keywords, comment_marker);
        conv::keyword("ok")
    })
}

/// `(syntax/map-extension extension lang)` → `:ok`
///
/// Point a path extension (dot omitted) at a fallback language.  Re-mapping
/// an extension replaces its language; mapping to a language without rules
/// is accepted and simply never highlights.
unsafe extern "C-unwind" fn c_syntax_map_extension(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(ext) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("syntax/map-extension: extension required");
        };
        let Some(lang) = conv::get_str(argc, argv, 1) else {
            conv::signal_err("syntax/map-extension: language required");
        };
        if ext.is_empty() || lang.is_empty() {
            conv::signal_err("syntax/map-extension: extension and language must not be empty");
        }
        ed.fallback_syntax.map_extension(&ext, &lang);
        conv::keyword("ok")
    })
}

/// `(syntax/set-ts-mode mode enabled)` → nil
///
/// Record that `mode`'s syntax layer is owned by the tree-sitter lane (or,
/// with enabled = false, returned to the fallback).  Flips clear the layer
/// on that mode's buffers so the active owner starts clean.
unsafe extern "C-unwind" fn c_syntax_set_ts_mode(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(mode) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("syntax/set-ts-mode: mode name required");
        };
        let enabled = conv::get_bool(argc, argv, 1).unwrap_or(false);
        kernel::semantic::fallback_syntax::set_ts_mode(ed, &mode, enabled);
        conv::nil()
    })
}

/// Strict keyword-list parser: accepts a Janet array or tuple (bracket
/// literals are tuples); any non-string element is a validation error,
/// not a silent skip.
unsafe fn get_keyword_list(argc: i32, argv: *mut Janet, i: i32) -> Option<Vec<String>> {
    unsafe {
        if i >= argc {
            return None;
        }
        let v = *argv.add(i as usize);
        let (data, count): (*const Janet, usize) = if janet_checktype(v, JanetType_JANET_ARRAY) != 0
        {
            let arr = janet_unwrap_array(v);
            ((*arr).data, (*arr).count as usize)
        } else if janet_checktype(v, JanetType_JANET_TUPLE) != 0 {
            let tup = janet_unwrap_tuple(v);
            let len = (*janet_tuple_head(tup)).length as usize;
            (tup, len)
        } else {
            return None;
        };
        let mut out = Vec::with_capacity(count);
        for idx in 0..count {
            let mut elem = *data.add(idx);
            let s = conv::get_str(1, &mut elem, 0)?;
            out.push(s);
        }
        Some(out)
    }
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"syntax/define-rules".as_ptr() as *const _,
            cfun: Some(c_syntax_define_rules),
            documentation: c"(syntax/define-rules lang keywords comment-marker) → :ok — replace the fallback keyword/comment rules for a language".as_ptr() as *const _,
        },
        JanetReg {
            name: c"syntax/map-extension".as_ptr() as *const _,
            cfun: Some(c_syntax_map_extension),
            documentation: c"(syntax/map-extension extension lang) → :ok — map a path extension to a fallback language".as_ptr() as *const _,
        },
        JanetReg {
            name: c"syntax/set-ts-mode".as_ptr() as *const _,
            cfun: Some(c_syntax_set_ts_mode),
            documentation: c"(syntax/set-ts-mode mode enabled) → nil — hand a mode's syntax layer to (or take it back from) the tree-sitter lane".as_ptr() as *const _,
        },
    ]
}
