//! Janet API for the GutterProvider architecture (Phase 7).
//!
//! Registered C functions:
//!   (gutter/add-provider name &opt provider-fn)  → nil
//!   (gutter/remove-provider name)                → nil
//!   (gutter/provider-update name buf-id cells)   → nil
//!   (gutter/provider-list)                       → [{:name "…" :width n} …]
//!   (gutter/set-fold-icons open closed face)     → nil
//!   (gutter/set-line-number-format fn-name)      → nil

use evil_janet::*;
use super::{conv, with_editor};
use crate::kernel::render::gutter_providers::JanetProvider;
use crate::kernel::render::surface::Style;
use crate::kernel::event::keys;
use crate::kernel::event::payload::GutterProviderUpdatedPayload;

// ── gutter/add-provider ───────────────────────────────────────────────────────

unsafe extern "C-unwind" fn c_add_provider(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/add-provider: name required")
        };
        let fn_name = conv::get_str(argc, argv, 1);
        let provider = Box::new(JanetProvider { provider_name: name, fn_name });
        ed.gutter.add_provider(provider);
        conv::nil()
    })
}

// ── gutter/remove-provider ────────────────────────────────────────────────────

unsafe extern "C-unwind" fn c_remove_provider(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/remove-provider: name required")
        };
        ed.gutter.remove_provider(&name);
        conv::nil()
    })
}

// ── gutter/provider-update ────────────────────────────────────────────────────
//
// (gutter/provider-update name buf-id cells)
// `cells` is an array of {:line N :text "T" :face "F"} tables.

unsafe extern "C-unwind" fn c_provider_update(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/provider-update: name required")
        };
        let Some(buf_id) = conv::get_int(argc, argv, 1).map(|k| k as usize) else {
            conv::signal_err("gutter/provider-update: buf-id required")
        };
        if argc < 3 { return conv::nil(); }
        let cells_val = *argv.add(2);
        let mut cells: Vec<(usize, String, Style)> = Vec::new();

        if janet_checktype(cells_val, JanetType_JANET_ARRAY) != 0 {
            let arr = janet_unwrap_array(cells_val);
            let count = (*arr).count as usize;
            for i in 0..count {
                let item = *(*arr).data.add(i);
                if janet_checktype(item, JanetType_JANET_TABLE) == 0
                    && janet_checktype(item, JanetType_JANET_STRUCT) == 0 {
                    continue;
                }
                let line_v   = janet_get(item, conv::keyword("line"));
                let text_v   = janet_get(item, conv::keyword("text"));
                let face_v   = janet_get(item, conv::keyword("face"));
                let line = janet_unwrap_integer(line_v) as usize;
                let text = if janet_checktype(text_v, JanetType_JANET_STRING) != 0
                    || janet_checktype(text_v, JanetType_JANET_KEYWORD) != 0
                {
                    let ptr = janet_unwrap_string(text_v);
                    std::ffi::CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned()
                } else { " ".to_string() };
                let face = if janet_checktype(face_v, JanetType_JANET_STRING) != 0
                    || janet_checktype(face_v, JanetType_JANET_KEYWORD) != 0
                {
                    let ptr = janet_unwrap_string(face_v);
                    std::ffi::CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned()
                } else { String::new() };
                let style = ed.resolve_face_style(&face).unwrap_or_else(|| Style {
                    fg: ed.theme_color("fg"),
                    bg: ed.theme_color("line-num-bg"),
                    ..Default::default()
                });
                cells.push((line, text, style));
            }
        }

        ed.gutter.set_provider_cells(&name, buf_id, cells);
        ed.events.emit_typed(keys::events::GUTTER_PROVIDER_UPDATED, GutterProviderUpdatedPayload {
            provider: name,
            buffer: buf_id.to_string(),
        });
        conv::nil()
    })
}

// ── gutter/provider-list ──────────────────────────────────────────────────────

unsafe extern "C-unwind" fn c_provider_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_array(ed.gutter.providers.len() as i32);
        for p in &ed.gutter.providers {
            let tbl = janet_table(2);
            janet_table_put(tbl, conv::keyword("name"),  conv::string(p.name()));
            janet_table_put(tbl, conv::keyword("width"), conv::integer(p.width() as i32));
            janet_array_push(arr, janet_wrap_table(tbl));
        }
        janet_wrap_array(arr)
    })
}

// ── gutter/set-fold-icons ─────────────────────────────────────────────────────

unsafe extern "C-unwind" fn c_set_fold_icons(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let open   = conv::get_str(argc, argv, 0).unwrap_or_else(|| "▾".to_string());
        let closed = conv::get_str(argc, argv, 1).unwrap_or_else(|| "▸".to_string());
        let face   = conv::get_str(argc, argv, 2).unwrap_or_else(|| "fold-face".to_string());
        ed.gutter.fold_icons.open = open;
        ed.gutter.fold_icons.closed = closed;
        ed.gutter.fold_icons.face = face;
        conv::nil()
    })
}

// ── gutter/set-line-number-format ─────────────────────────────────────────────

unsafe extern "C-unwind" fn c_set_line_number_format(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc > 0 {
            let v = *argv.add(0);
            if janet_checktype(v, JanetType_JANET_NIL) != 0 {
                ed.gutter.line_number_fn = None;
            } else {
                ed.gutter.line_number_fn = conv::get_str(argc, argv, 0);
            }
        } else {
            ed.gutter.line_number_fn = None;
        }
        conv::nil()
    })
}

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"gutter/add-provider".as_ptr() as *const _,
            cfun: Some(c_add_provider),
            documentation: c"(gutter/add-provider name &opt provider-fn) → nil — register a named gutter column provider".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/remove-provider".as_ptr() as *const _,
            cfun: Some(c_remove_provider),
            documentation: c"(gutter/remove-provider name) → nil — remove a gutter provider by name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/provider-update".as_ptr() as *const _,
            cfun: Some(c_provider_update),
            documentation: c"(gutter/provider-update name buf-id cells) → nil — push pre-computed cells to a gutter provider".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/provider-list".as_ptr() as *const _,
            cfun: Some(c_provider_list),
            documentation: c"(gutter/provider-list) → [{:name \"…\" :width n} …] — list all registered gutter providers".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/set-fold-icons".as_ptr() as *const _,
            cfun: Some(c_set_fold_icons),
            documentation: c"(gutter/set-fold-icons open closed face) → nil — set fold-open and fold-closed gutter icons".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/set-line-number-format".as_ptr() as *const _,
            cfun: Some(c_set_line_number_format),
            documentation: c"(gutter/set-line-number-format fn-name) → nil — set a custom Janet command to format line numbers".as_ptr() as *const _,
        },
    ]
}
