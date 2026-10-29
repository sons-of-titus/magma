//! Janet API for the language registry.
//!
//! Languages are registered by plugins (see builtins/plugin_system.janet);
//! these functions expose the registry to Janet.
//!
//! Registered C functions:
//!   (language/register name &opt syntax-path config-path) → :ok
//!   (language/unregister name)                            → nil
//!   (language/list)                                       → array of tables
//!   (language/for-buffer buf-id)                          → string or nil

use super::conv;
use super::with_editor;
use evil_janet::*;
use kernel::event::keys;
use kernel::event::payload::{LanguageRegisteredPayload, LanguageUnregisteredPayload};
use kernel::semantic::language_registry::LanguageInfo;

// ── C functions ──────────────────────────────────────────────────────────────

/// `(language/register name &opt syntax-path config-path)` → `:ok`
///
/// Register a language.  Optional positional arguments:
///   - syntax-path — path to `syntax.scm`
///   - config-path — path to `config.janet`
///
/// Re-registering an existing language name replaces its entry (the loaded
/// flag is reset to false).
unsafe extern "C-unwind" fn c_language_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let name = match unsafe { conv::get_str(argc, argv, 0) } {
            Some(s) => s,
            None => conv::signal_err("language/register: name string required"),
        };
        let syntax_path = unsafe { conv::get_str(argc, argv, 1) };
        let config_path = unsafe { conv::get_str(argc, argv, 2) };

        let info = LanguageInfo {
            name: name.clone(),
            syntax_path,
            config_path,
            loaded: false,
        };
        ed.language_registry.register(info);
        ed.events.emit_typed(
            keys::events::LANGUAGE_REGISTERED,
            LanguageRegisteredPayload { name },
        );
        conv::keyword("ok")
    })
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

/// `(language/list)` → array of `{:name :loaded :syntax-path :config-path}`
///
/// Return all registered languages sorted alphabetically by name.
unsafe extern "C-unwind" fn c_language_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let langs = ed.language_registry.list();
        let arr = janet_wrap_array(janet_array(langs.len() as i32));
        for info in langs {
            let tbl = janet_wrap_table(janet_table(4));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("name"), conv::string(&info.name));
            janet_table_put(t, conv::keyword("loaded"), conv::boolean(info.loaded));
            let sp = info
                .syntax_path
                .as_deref()
                .map(conv::string)
                .unwrap_or_else(conv::nil);
            janet_table_put(t, conv::keyword("syntax-path"), sp);
            let cp = info
                .config_path
                .as_deref()
                .map(conv::string)
                .unwrap_or_else(conv::nil);
            janet_table_put(t, conv::keyword("config-path"), cp);
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
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
            documentation: c"Register a language: (language/register name &opt syntax-path config-path)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/unregister".as_ptr() as *const _,
            cfun: Some(c_language_unregister as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Unregister a language by name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/list".as_ptr() as *const _,
            cfun: Some(c_language_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return a sorted array of {:name :loaded :syntax-path :config-path} tables".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/for-buffer".as_ptr() as *const _,
            cfun: Some(c_language_for_buffer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the language name for a buffer (from ts_languages), or nil".as_ptr() as *const _,
        },
    ]
}
