//! Janet API for face (named style) registry, scope→face mapping,
//! and the make-style helper (Sprint 6).

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::render::surface::Style;
use crate::event::payload::*;
use crate::event::keys;

/// Look up a keyword key in a struct or table via janet_in.
unsafe fn lookup(ds: Janet, key_name: &str) -> Janet {
    janet_in(ds, conv::keyword(key_name))
}

/// Extract an RGB triple from a Janet array or tuple value.
unsafe fn rgb_from_val(v: Janet) -> Option<(u8, u8, u8)> {
    if janet_checktype(v, JanetType_JANET_ARRAY) != 0 {
        let arr = &*janet_unwrap_array(v);
        if arr.count < 3 { return None; }
        let r = janet_unwrap_integer(arr.data.add(0).read()) as u8;
        let g = janet_unwrap_integer(arr.data.add(1).read()) as u8;
        let b = janet_unwrap_integer(arr.data.add(2).read()) as u8;
        Some((r, g, b))
    } else if janet_checktype(v, JanetType_JANET_TUPLE) != 0 {
        let tup = janet_unwrap_tuple(v);
        let head = &*janet_tuple_head(tup);
        if head.length < 3 { return None; }
        let r = janet_unwrap_integer(tup.add(0).read()) as u8;
        let g = janet_unwrap_integer(tup.add(1).read()) as u8;
        let b = janet_unwrap_integer(tup.add(2).read()) as u8;
        Some((r, g, b))
    } else {
        None
    }
}

unsafe fn arr_from_ds(ds: Janet, key: &str) -> Option<(u8, u8, u8)> {
    let v = lookup(ds, key);
    rgb_from_val(v)
}

unsafe fn bool_from_ds(ds: Janet, key: &str) -> Option<bool> {
    let v = lookup(ds, key);
    if janet_checktype(v, JanetType_JANET_BOOLEAN) == 0 { return None; }
    Some(janet_unwrap_boolean(v) != 0)
}

unsafe fn parse_style(v: Janet) -> Style {
    let is_ds = janet_checktype(v, JanetType_JANET_STRUCT) != 0
        || janet_checktype(v, JanetType_JANET_TABLE) != 0;
    if !is_ds { return Style::default(); }
    let default = Style::default();
    Style {
        fg: arr_from_ds(v, "fg").unwrap_or(default.fg),
        bg: arr_from_ds(v, "bg").unwrap_or(default.bg),
        bold: bool_from_ds(v, "bold").unwrap_or(default.bold),
        italic: bool_from_ds(v, "italic").unwrap_or(default.italic),
        underline: bool_from_ds(v, "underline").unwrap_or(default.underline),
        strikethrough: bool_from_ds(v, "strikethrough").unwrap_or(default.strikethrough),
        dim: bool_from_ds(v, "dim").unwrap_or(default.dim),
    }
}

unsafe fn style_to_table(s: &Style) -> Janet {
    let fg_arr = janet_array(3);
    janet_array_push(fg_arr, conv::integer(s.fg.0 as i32));
    janet_array_push(fg_arr, conv::integer(s.fg.1 as i32));
    janet_array_push(fg_arr, conv::integer(s.fg.2 as i32));
    let bg_arr = janet_array(3);
    janet_array_push(bg_arr, conv::integer(s.bg.0 as i32));
    janet_array_push(bg_arr, conv::integer(s.bg.1 as i32));
    janet_array_push(bg_arr, conv::integer(s.bg.2 as i32));
    let tbl = janet_table(7);
    janet_table_put(tbl, conv::keyword("fg"), janet_wrap_array(fg_arr));
    janet_table_put(tbl, conv::keyword("bg"), janet_wrap_array(bg_arr));
    janet_table_put(tbl, conv::keyword("bold"), conv::boolean(s.bold));
    janet_table_put(tbl, conv::keyword("italic"), conv::boolean(s.italic));
    janet_table_put(tbl, conv::keyword("underline"), conv::boolean(s.underline));
    janet_table_put(tbl, conv::keyword("strikethrough"), conv::boolean(s.strikethrough));
    janet_table_put(tbl, conv::keyword("dim"), conv::boolean(s.dim));
    janet_wrap_table(tbl)
}

/// (editor/define-face name {:fg [r g b] :bg [r g b] :bold bool :italic bool
///                         :underline bool :strikethrough bool :dim bool})
unsafe extern "C-unwind" fn c_editor_define_face(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/define-face requires a face name")
        };
        let style = if argc > 1 { parse_style(*argv.add(1)) } else { Style::default() };
        ed.faces.insert(name.clone(), style);
        ed.events.emit_typed(keys::events::FACE_CHANGED, FaceChangedPayload { face: name });
        conv::nil()
    })
}

/// (editor/face name) → {:fg [r g b] :bg [r g b] :bold bool …} or nil
unsafe extern "C-unwind" fn c_editor_face(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        match ed.faces.get(&name) {
            Some(s) => style_to_table(s),
            None => conv::nil(),
        }
    })
}

/// (editor/make-style {:fg [r g b] :bg [r g b] :bold bool …}) → integer handle
unsafe extern "C-unwind" fn c_editor_make_style(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            conv::signal_err("editor/make-style requires a style table")
        }
        let style = parse_style(*argv.add(0));
        let id = ed.next_buffer_id as i32;
        ed.faces.insert(format!("__style_{}", id), style);
        conv::integer(id)
    })
}

/// (editor/scope-face scope-prefix face-name)
unsafe extern "C-unwind" fn c_editor_scope_face(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(scope) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/scope-face requires a scope prefix")
        };
        let Some(face) = conv::get_str(argc, argv, 1) else {
            conv::signal_err("editor/scope-face requires a face name")
        };
        ed.scope_faces.push((scope, face));
        ed.scope_faces.sort_by_key(|b| std::cmp::Reverse(b.0.len()));
        conv::nil()
    })
}

/// (editor/resolve-scope scope) → face-name or nil
unsafe extern "C-unwind" fn c_editor_resolve_scope(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(scope) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        for (prefix, face) in &ed.scope_faces {
            if scope.starts_with(prefix) {
                return conv::string(face);
            }
        }
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"face/define".as_ptr() as *const _,
            cfun: Some(c_editor_define_face as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Define or redefine a named face (style)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"face/get".as_ptr() as *const _,
            cfun: Some(c_editor_face as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return a face's style table, or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"face/make-style".as_ptr() as *const _,
            cfun: Some(c_editor_make_style as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Create a one-off style and return an integer handle".as_ptr() as *const _,
        },
        JanetReg {
            name: c"face/scope-face".as_ptr() as *const _,
            cfun: Some(c_editor_scope_face as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Map a scope prefix to a face name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"face/resolve-scope".as_ptr() as *const _,
            cfun: Some(c_editor_resolve_scope as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Resolve a scope string to the best-matching face name".as_ptr() as *const _,
        },
    ]
}
