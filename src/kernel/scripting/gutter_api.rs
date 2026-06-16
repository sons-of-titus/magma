//! Janet API for the named-column gutter system.

use evil_janet::*;
use super::{conv, with_editor};
use crate::kernel::state::{GutterColumn, GutterSign};
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

// ── Column registry ───────────────────────────────────────────────────────────

unsafe extern "C-unwind" fn c_define_column(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/define-column: name required")
        };
        let width = conv::get_int(argc, argv, 1).unwrap_or(1) as usize;
        let face = conv::get_str(argc, argv, 2).unwrap_or_else(|| "gutter-bg".to_string());
        if let Some(col) = ed.gutter.columns.iter_mut().find(|c| c.name == name) {
            col.width = width;
            col.face = face;
        } else {
            ed.gutter.columns.push(GutterColumn { name, width, visible: true, face });
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_show_column(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/show-column: name required")
        };
        if let Some(col) = ed.gutter.columns.iter_mut().find(|c| c.name == name) {
            col.visible = true;
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_hide_column(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/hide-column: name required")
        };
        if let Some(col) = ed.gutter.columns.iter_mut().find(|c| c.name == name) {
            col.visible = false;
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_set_column_face(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/set-column-face: name required")
        };
        let face = conv::get_str(argc, argv, 1).unwrap_or_default();
        if let Some(col) = ed.gutter.columns.iter_mut().find(|c| c.name == name) {
            col.face = face;
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_column_list(argc: i32, argv: *mut Janet) -> Janet {
    let _ = (argc, argv);
    with_editor(|ed| unsafe {
        let arr = janet_array(ed.gutter.columns.len() as i32);
        for col in &ed.gutter.columns {
            let tbl = janet_table(4);
            janet_table_put(tbl, conv::keyword("name"),    conv::string(&col.name));
            janet_table_put(tbl, conv::keyword("width"),   conv::integer(col.width as i32));
            janet_table_put(tbl, conv::keyword("visible"), conv::boolean(col.visible));
            janet_table_put(tbl, conv::keyword("face"),    conv::string(&col.face));
            janet_array_push(arr, janet_wrap_table(tbl));
        }
        janet_wrap_array(arr)
    })
}

// ── Column sign API ───────────────────────────────────────────────────────────

unsafe extern "C-unwind" fn c_sign_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(col_name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/sign-set: column name required")
        };
        let Some(buf_key) = conv::get_int(argc, argv, 1).map(|k| k as usize) else {
            conv::signal_err("gutter/sign-set: buffer key required")
        };
        let line = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        let text = conv::get_str(argc, argv, 3).unwrap_or_default();
        let face = conv::get_str(argc, argv, 4).unwrap_or_default();
        let priority = conv::get_int(argc, argv, 5).unwrap_or(0);

        let line_map = ed.gutter.column_signs
            .entry((col_name.clone(), buf_key))
            .or_default();
        let signs = line_map.entry(line).or_default();
        if let Some(existing) = signs.iter_mut().find(|s| s.priority == priority) {
            existing.text = text;
            existing.face = face;
        } else {
            signs.push(GutterSign { face, text, priority });
        }

        ed.events.emit_typed(keys::events::GUTTER_SIGN_CHANGED, GutterSignChangedPayload {
            column: col_name,
            buffer: buf_key.to_string(),
            line: line.to_string(),
        });
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_sign_clear(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(col_name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/sign-clear: column name required")
        };
        let Some(buf_key) = conv::get_int(argc, argv, 1).map(|k| k as usize) else {
            conv::signal_err("gutter/sign-clear: buffer key required")
        };
        ed.gutter.column_signs.remove(&(col_name, buf_key));
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_sign_clear_line(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(col_name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("gutter/sign-clear-line: column name required")
        };
        let Some(buf_key) = conv::get_int(argc, argv, 1).map(|k| k as usize) else {
            conv::signal_err("gutter/sign-clear-line: buffer key required")
        };
        let line = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        if let Some(line_map) = ed.gutter.column_signs.get_mut(&(col_name, buf_key)) {
            line_map.remove(&line);
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_signs(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let col_name = conv::get_str(argc, argv, 0).unwrap_or_default();
        let buf_key = conv::get_int(argc, argv, 1).unwrap_or(0) as usize;
        let empty = std::collections::HashMap::new();
        let line_map = ed.gutter.column_signs
            .get(&(col_name, buf_key))
            .unwrap_or(&empty);
        let total: usize = line_map.values().map(|v| v.len()).sum();
        let arr = janet_array(total as i32);
        let mut lines: Vec<usize> = line_map.keys().copied().collect();
        lines.sort_unstable();
        for line in lines {
            if let Some(signs) = line_map.get(&line) {
                for sign in signs {
                    let tbl = janet_table(4);
                    janet_table_put(tbl, conv::keyword("line"),     conv::integer(line as i32));
                    janet_table_put(tbl, conv::keyword("text"),     conv::string(&sign.text));
                    janet_table_put(tbl, conv::keyword("face"),     conv::string(&sign.face));
                    janet_table_put(tbl, conv::keyword("priority"), conv::integer(sign.priority));
                    janet_array_push(arr, janet_wrap_table(tbl));
                }
            }
        }
        janet_wrap_array(arr)
    })
}

// ── Line-number format ────────────────────────────────────────────────────────

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

// ── Fold icons ────────────────────────────────────────────────────────────────

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

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"gutter/define-column".as_ptr() as *const _,
            cfun: Some(c_define_column),
            documentation: c"Register or update a named gutter column".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/show-column".as_ptr() as *const _,
            cfun: Some(c_show_column),
            documentation: c"Make a gutter column visible".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/hide-column".as_ptr() as *const _,
            cfun: Some(c_hide_column),
            documentation: c"Hide a gutter column".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/set-column-face".as_ptr() as *const _,
            cfun: Some(c_set_column_face),
            documentation: c"Set the background face for a gutter column".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/column-list".as_ptr() as *const _,
            cfun: Some(c_column_list),
            documentation: c"Return all gutter columns in render order".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/sign-set".as_ptr() as *const _,
            cfun: Some(c_sign_set),
            documentation: c"Register a sign in a named gutter column".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/sign-clear".as_ptr() as *const _,
            cfun: Some(c_sign_clear),
            documentation: c"Remove all signs in a column for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/sign-clear-line".as_ptr() as *const _,
            cfun: Some(c_sign_clear_line),
            documentation: c"Remove signs on one line in a column".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/signs".as_ptr() as *const _,
            cfun: Some(c_signs),
            documentation: c"Return all signs in a column for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/set-line-number-format".as_ptr() as *const _,
            cfun: Some(c_set_line_number_format),
            documentation: c"Set the Janet command used to format line numbers".as_ptr() as *const _,
        },
        JanetReg {
            name: c"gutter/set-fold-icons".as_ptr() as *const _,
            cfun: Some(c_set_fold_icons),
            documentation: c"Set the fold-open and fold-closed gutter icons".as_ptr() as *const _,
        },
    ]
}
