//! Janet API for the language registry.
//!
//! Languages are registered by plugins (see builtins/language.janet and
//! builtins/plugin_system.janet); these functions expose the registry to
//! Janet.
//!
//! Registered C functions:
//!   (language/register name &opt syntax-path config-path) → :ok
//!   (language/register spec-table)                        → info table
//!   (language/unregister name)                            → nil
//!   (language/list)                                       → array of tables
//!   (language/for-buffer buf-id)                          → string or nil
//!
//! The table form declares a language fully:
//! `{:id "nova" :extensions ["nova"] :grammar "/abs/lib.so"
//!   :queries {:highlights "/abs/syntax.scm"}
//!   :comments {:line "//" :block ["/*" "*/"]}
//!   :indentation {:size 4 :style "spaces"} :plugin "nova"}` — a plugin
//! grammar shared library is loaded and ABI-validated at registration;
//! failures warn but do not abort (fallback highlighting still applies).

use super::conv;
use super::jtable;
use super::with_editor;
use evil_janet::*;
use kernel::event::keys;
use kernel::event::payload::{LanguageRegisteredPayload, LanguageUnregisteredPayload};
use kernel::semantic::language_registry::LanguageInfo;
use kernel::semantic::ts_registry;

// ── C functions ──────────────────────────────────────────────────────────────

/// `(language/register name &opt syntax-path config-path)` → `:ok`
/// `(language/register spec-table)` → `{:id :grammar-loaded :extensions}`
unsafe extern "C-unwind" fn c_language_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc >= 1 && jtable::is_spec(*argv.add(0)) {
            return register_from_spec(ed, *argv.add(0));
        }
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("language/register: name string or spec table required"),
        };
        let syntax_path = conv::get_str(argc, argv, 1);
        let config_path = conv::get_str(argc, argv, 2);

        let info = LanguageInfo {
            name: name.clone(),
            syntax_path,
            config_path,
            ..LanguageInfo::new(name.clone())
        };
        ed.language_registry.register(info);
        ed.events.emit_typed(
            keys::events::LANGUAGE_REGISTERED,
            LanguageRegisteredPayload { name },
        );
        conv::keyword("ok")
    })
}

unsafe fn register_from_spec(ed: &mut kernel::state::Editor, spec: Janet) -> Janet {
    unsafe {
        if !jtable::is_spec(spec) {
            conv::signal_err("language/register: spec must be a table or struct");
        }
        let Some(id) = jtable::get_str(spec, "id") else {
            conv::signal_err("language/register: :id is required");
        };
        let extensions = jtable::get_str_array(spec, "extensions").unwrap_or_default();
        // `:queries {:highlights ...}` is the full form; `:syntax` is the
        // conventional shorthand language/define passes through.
        let highlights = jtable::get_spec(spec, "queries")
            .and_then(|q| jtable::get_str(q, "highlights"))
            .or_else(|| jtable::get_str(spec, "syntax"));
        let (comment_line, comment_block) = match jtable::get_spec(spec, "comments") {
            Some(c) => (
                jtable::get_str(c, "line"),
                jtable::get_str_array(c, "block").and_then(|b| match b.as_slice() {
                    [open, close] => Some((open.clone(), close.clone())),
                    _ => None,
                }),
            ),
            None => (None, None),
        };
        let (indent_size, indent_style) = match jtable::get_spec(spec, "indentation") {
            Some(i) => {
                let size = jtable::get(i, "size");
                let size = if janet_checktype(size, JanetType_JANET_NUMBER) != 0 {
                    Some(janet_unwrap_integer(size).max(0) as u32)
                } else {
                    None
                };
                let style = jtable::get_str(i, "style");
                (size, style)
            }
            None => (None, None),
        };

        let mut grammar_loaded = false;
        let grammar_path = jtable::get_str(spec, "grammar");
        if let Some(path) = grammar_path.as_deref() {
            grammar_loaded = load_grammar(
                ed,
                &id,
                path,
                jtable::get_str(spec, "grammar-symbol").as_deref(),
            );
        }

        let mut info = LanguageInfo {
            name: id.clone(),
            syntax_path: highlights.clone(),
            extensions,
            grammar_path,
            grammar_loaded,
            comment_line,
            comment_block,
            indent_size,
            indent_style,
            owner: jtable::get_str(spec, "plugin"),
            ..LanguageInfo::new(id.clone())
        };
        // The positional form's sibling config.janet convention applies to
        // the table form too when the plugin declares :config.
        info.config_path = jtable::get_str(spec, "config");
        ed.language_registry.register(info);
        ed.events.emit_typed(
            keys::events::LANGUAGE_REGISTERED,
            LanguageRegisteredPayload { name: id.clone() },
        );

        if grammar_loaded && let Some(path) = highlights {
            match std::fs::read_to_string(&path) {
                Ok(src) => {
                    if let Err(e) = ts_registry::validate_query(&id, &src) {
                        ed.show_message(
                            kernel::state::MessageKind::Error,
                            format!("[language] {id}: highlight query failed to compile: {e}"),
                        );
                    }
                }
                Err(e) => ed.show_message(
                    kernel::state::MessageKind::Error,
                    format!("[language] {id}: cannot read highlight query {path}: {e}"),
                ),
            }
        }

        let result = janet_wrap_table(janet_table(3));
        janet_table_put(
            janet_unwrap_table(result),
            conv::keyword("id"),
            conv::string(&id),
        );
        janet_table_put(
            janet_unwrap_table(result),
            conv::keyword("grammar-loaded"),
            conv::boolean(grammar_loaded),
        );
        result
    }
}

/// Load a plugin-provided grammar library into the tree-sitter registry.
/// `symbol` overrides the conventional `tree_sitter_<id>` export name for
/// grammars whose C entry point deviates.  Failure warns and returns
/// false — the language still registers and fallback highlighting
/// applies; a bad grammar must not block plugin loading or take the
/// editor down.
unsafe fn load_grammar(
    ed: &mut kernel::state::Editor,
    id: &str,
    path: &str,
    symbol: Option<&str>,
) -> bool {
    if ts_registry::has_language(id) {
        return true;
    }
    match ts_registry::load_grammar_from_lib(path, id, symbol) {
        Ok(lang) => {
            ts_registry::register_language(id, lang);
            true
        }
        Err(e) => {
            ed.show_message(
                kernel::state::MessageKind::Error,
                format!("[language] {id}: grammar {path} failed to load: {e}"),
            );
            false
        }
    }
}

/// `(language/unregister name)` → nil
///
/// Remove a language from the registry.
unsafe extern "C-unwind" fn c_language_unregister(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let name = match unsafe { conv::get_str(argc, argv, 0) } {
            Some(s) => s,
            None => return conv::nil(),
        };
        if ed.language_registry.unregister(&name) {
            ed.events.emit_typed(
                keys::events::LANGUAGE_UNREGISTERED,
                LanguageUnregisteredPayload { name },
            );
        }
        conv::nil()
    })
}

/// `(language/list)` → array of `{:name :loaded :syntax-path :config-path
/// :extensions :grammar-loaded :owner}` tables
unsafe extern "C-unwind" fn c_language_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let langs = ed.language_registry.list();
        let arr = janet_wrap_array(janet_array(langs.len() as i32));
        for info in langs {
            let tbl = janet_wrap_table(janet_table(8));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("name"), conv::string(&info.name));
            janet_table_put(t, conv::keyword("loaded"), conv::boolean(info.loaded));
            put_opt(t, "syntax-path", info.syntax_path.as_deref());
            put_opt(t, "config-path", info.config_path.as_deref());
            put_opt(t, "grammar-path", info.grammar_path.as_deref());
            janet_table_put(
                t,
                conv::keyword("grammar-loaded"),
                conv::boolean(info.grammar_loaded),
            );
            let exts = janet_array(info.extensions.len().max(1) as i32);
            for e in &info.extensions {
                janet_array_push(exts, conv::string(e));
            }
            janet_table_put(t, conv::keyword("extensions"), janet_wrap_array(exts));
            put_opt(t, "plugin", info.owner.as_deref());
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

unsafe fn put_opt(t: *mut JanetTable, key: &str, value: Option<&str>) {
    unsafe {
        let v = match value {
            Some(s) => conv::string(s),
            None => conv::nil(),
        };
        janet_table_put(t, conv::keyword(key), v);
    }
}

/// `(language/for-buffer buf-id)` → string or nil
///
/// Return the language name associated with the buffer (from `ts_languages`),
/// or nil if no language has been set for it.
unsafe extern "C-unwind" fn c_language_for_buffer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let buf_id = unsafe { conv::get_int(argc, argv, 0) }.unwrap_or(-1) as usize;
        match ed.ts_languages.get(&buf_id) {
            Some(lang) => conv::string(lang),
            None => conv::nil(),
        }
    })
}

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"language/register".as_ptr() as *const _,
            cfun: Some(c_language_register as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a language: (language/register name syntax-path config-path) or (language/register spec) with :id :extensions :grammar :queries :comments :indentation".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/unregister".as_ptr() as *const _,
            cfun: Some(c_language_unregister as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Unregister a language by name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/list".as_ptr() as *const _,
            cfun: Some(c_language_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return a sorted array of {:name :loaded :syntax-path :config-path :extensions :grammar-loaded :plugin} tables".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/for-buffer".as_ptr() as *const _,
            cfun: Some(c_language_for_buffer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the language name for a buffer (from ts_languages), or nil".as_ptr() as *const _,
        },
    ]
}
