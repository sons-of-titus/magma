//! Janet API for picker sources — plugins contribute search surfaces to
//! the frontend's picker overlay.
//!
//! Registered C functions:
//!   (picker/register-source id title placeholder) → :ok
//!   (picker/push-items items)                     → :ok
//!   (picker/sources)                              → array of tables

use super::conv;
use super::jtable;
use super::with_editor;
use evil_janet::*;

/// `(picker/register-source id title placeholder)` → `:ok`
///
/// Declare a search surface.  When the picker opens on it, the frontend
/// emits `picker-source-query` with `{:source id :query …}`; answer with
/// `(picker/push-items …)` — entry tables of `:title :detail :meta` and a
/// `:payload` of `"path␟line"` (open at line) or `"cmd:<command line>"`.
unsafe extern "C-unwind" fn c_picker_register_source(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = match conv::get_str(argc, argv, 0) {
            Some(v) => v,
            None => conv::signal_err("picker/register-source: id string required"),
        };
        let title = conv::get_str(argc, argv, 1).unwrap_or_else(|| id.clone());
        let placeholder = conv::get_str(argc, argv, 2).unwrap_or_default();
        let owner = match super::extension_api::current_context() {
            super::extension_api::ExecContext::Extension(name) => Some(name),
            _ => None,
        };
        ed.picker_sources.register(kernel::state::picker::PickerSource {
            id,
            title,
            placeholder,
            owner,
        });
        super::extension_api::record_if_plugin(
            ed,
            kernel::state::contributions::Contribution::PickerSource(
                conv::get_str(argc, argv, 0).unwrap_or_default(),
            ),
        );
        conv::keyword("ok")
    })
}

/// `(picker/push-items items)` → `:ok`
///
/// Answer the current `picker-source-query`: `items` is an array of
/// `{:title :detail :meta :payload}` tables.
unsafe extern "C-unwind" fn c_picker_push_items(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        ed.picker_state.items.clear();
        if argc < 1 {
            return conv::keyword("ok");
        }
        let v = *argv.add(0);
        let is_array = janet_checktype(v, JanetType_JANET_ARRAY) != 0;
        let is_tuple = janet_checktype(v, JanetType_JANET_TUPLE) != 0;
        if !is_array && !is_tuple {
            // A non-sequence answer is an empty answer.
            return conv::keyword("ok");
        }
        let count = janet_length(v);
        for i in 0..count {
            let entry = if is_array {
                *(*janet_unwrap_array(v)).data.add(i as usize)
            } else {
                *janet_unwrap_tuple(v).add(i as usize)
            };
            if jtable::is_spec(entry) {
                let get = |key: &str| -> String {
                    jtable::get_str(entry, key).unwrap_or_default()
                };
                ed.picker_state.items.push(kernel::state::picker::PickerEntry {
                    title: get("title"),
                    detail: get("detail"),
                    meta: get("meta"),
                    payload: get("payload"),
                });
            }
        }
        conv::keyword("ok")
    })
}

/// `(picker/sources)` → array of `{:id :title :placeholder}` tables
unsafe extern "C-unwind" fn c_picker_sources(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let sources = ed.picker_sources.list();
        let arr = janet_wrap_array(janet_array(sources.len() as i32));
        for s in sources {
            let tbl = janet_wrap_table(janet_table(3));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("id"), conv::string(&s.id));
            janet_table_put(t, conv::keyword("title"), conv::string(&s.title));
            janet_table_put(
                t,
                conv::keyword("placeholder"),
                conv::string(&s.placeholder),
            );
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"picker/register-source".as_ptr() as *const _,
            cfun: Some(
                c_picker_register_source
                    as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Register a picker search surface by id, title and placeholder"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"picker/push-items".as_ptr() as *const _,
            cfun: Some(
                c_picker_push_items as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Answer the current picker-source-query with an array of entry tables"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"picker/sources".as_ptr() as *const _,
            cfun: Some(
                c_picker_sources as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"List registered picker sources".as_ptr() as *const _,
        },
    ]
}
