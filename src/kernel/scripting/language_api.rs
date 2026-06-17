//! Janet API for the Language Provider Ecosystem (Phase 12).
//!
//! Registered C functions:
//!   (language/register name &opt provider-path syntax-path config-path) → :ok
//!   (language/unregister name)                                           → nil
//!   (language/list)                                                      → array of tables
//!   (language/for-buffer buf-id)                                         → string or nil
//!   (language/load name)                                                 → :ok or :error

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::semantic::language_registry::LanguageInfo;
use crate::kernel::event::keys;
use crate::kernel::event::payload::{
    LanguageRegisteredPayload, LanguageLoadedPayload, LanguageUnregisteredPayload,
};

// ── C functions ──────────────────────────────────────────────────────────────

/// `(language/register name &opt provider-path syntax-path config-path)` → `:ok`
///
/// Register a language.  Optional positional arguments:
///   - provider-path — path to `provider.janet`
///   - syntax-path   — path to `syntax.scm`
///   - config-path   — path to `config.janet`
///
/// Re-registering an existing language name replaces its entry (the loaded
/// flag is reset to false).
unsafe extern "C-unwind" fn c_language_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let name = match unsafe { conv::get_str(argc, argv, 0) } {
            Some(s) => s,
            None => conv::signal_err("language/register: name string required"),
        };
        let provider_path = unsafe { conv::get_str(argc, argv, 1) };
        let syntax_path   = unsafe { conv::get_str(argc, argv, 2) };
        let config_path   = unsafe { conv::get_str(argc, argv, 3) };

        let info = LanguageInfo {
            name: name.clone(),
            provider_path,
            syntax_path,
            config_path,
            loaded: false,
        };
        ed.language_registry.register(info);
        ed.events.emit_typed(keys::events::LANGUAGE_REGISTERED, LanguageRegisteredPayload {
            name,
        });
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
            ed.events.emit_typed(keys::events::LANGUAGE_UNREGISTERED, LanguageUnregisteredPayload {
                name,
            });
        }
        conv::nil()
    })
}

/// `(language/list)` → array of `{:name :loaded :provider-path :syntax-path :config-path}`
///
/// Return all registered languages sorted alphabetically by name.
unsafe extern "C-unwind" fn c_language_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let langs = ed.language_registry.list();
        let arr = janet_wrap_array(janet_array(langs.len() as i32));
        for info in langs {
            let tbl = janet_wrap_table(janet_table(5));
            let t   = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("name"),   conv::string(&info.name));
            janet_table_put(t, conv::keyword("loaded"), conv::boolean(info.loaded));
            let pp = info.provider_path.as_deref().map(conv::string).unwrap_or_else(conv::nil);
            janet_table_put(t, conv::keyword("provider-path"), pp);
            let sp = info.syntax_path.as_deref().map(conv::string).unwrap_or_else(conv::nil);
            janet_table_put(t, conv::keyword("syntax-path"), sp);
            let cp = info.config_path.as_deref().map(conv::string).unwrap_or_else(conv::nil);
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
            None       => conv::nil(),
        }
    })
}

/// `(language/load name)` → `:ok` or `:error`
///
/// Load the `provider.janet` for the named language into the Janet VM.
/// The file is read from disk and evaluated via `janet_dostring` (safe to
/// call from a C callback since it does not use `janet_continue`).
/// Marks the language as loaded on success.
unsafe extern "C-unwind" fn c_language_load(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let name = match unsafe { conv::get_str(argc, argv, 0) } {
            Some(s) => s,
            None => conv::signal_err("language/load: name string required"),
        };

        // Clone the provider path so we can drop the borrow on ed before
        // calling eval_result (which may re-enter through with_editor).
        let provider_path = ed.language_registry.get(&name)
            .and_then(|i| i.provider_path.clone());

        let content = match provider_path {
            None       => return conv::keyword("error"),
            Some(path) => match std::fs::read_to_string(&path) {
                Ok(c)  => c,
                Err(_) => return conv::keyword("error"),
            },
        };

        // eval_result calls janet_dostring — safe from within a C callback.
        // Re-entrant access to ed via EDITOR_PTR is the established pattern;
        // see the reentrancy note in kernel/scripting/mod.rs.
        match crate::kernel::scripting::eval_result(&content) {
            Ok(_) => {
                ed.language_registry.mark_loaded(&name);
                ed.events.emit_typed(keys::events::LANGUAGE_LOADED, LanguageLoadedPayload {
                    name,
                });
                conv::keyword("ok")
            }
            Err(_) => conv::keyword("error"),
        }
    })
}

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"language/register".as_ptr() as *const _,
            cfun: Some(c_language_register as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a language: (language/register name &opt provider-path syntax-path config-path)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/unregister".as_ptr() as *const _,
            cfun: Some(c_language_unregister as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Unregister a language by name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/list".as_ptr() as *const _,
            cfun: Some(c_language_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return a sorted array of {:name :loaded :provider-path :syntax-path :config-path} tables".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/for-buffer".as_ptr() as *const _,
            cfun: Some(c_language_for_buffer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the language name for a buffer (from ts_languages), or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"language/load".as_ptr() as *const _,
            cfun: Some(c_language_load as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Load a language's provider.janet from disk; returns :ok or :error".as_ptr() as *const _,
        },
    ]
}
