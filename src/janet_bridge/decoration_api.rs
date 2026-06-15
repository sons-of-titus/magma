//! Janet API for buffer decoration layers (Sprint 11b).
//!
//! Decoration layers let extensions render text alongside buffer content
//! without displacing it:
//!   • InlineText — overlaid at a character position
//!   • EndOfLine  — appended after the last character on the line
//!   • LinePrefix — shown in a reserved left-margin column

use evil_janet::*;
use super::{conv, with_editor};

/// (buffer/decor-set-inline buf layer line col text face)
///
/// Insert or replace an `InlineText` decoration at `(line, col)` in the named
/// layer of `buf`.  An existing decoration at the same (line, col) in the same
/// layer is replaced.
unsafe extern "C-unwind" fn c_decor_set_inline(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = conv::get_int(argc, argv, 0).map(|k| k as usize) else {
            conv::signal_err("buffer/decor-set-inline requires a buffer key")
        };
        let layer = conv::get_str(argc, argv, 1).unwrap_or_default();
        let line  = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        let col   = conv::get_int(argc, argv, 3).unwrap_or(0) as usize;
        let text  = conv::get_str(argc, argv, 4).unwrap_or_default();
        let face  = conv::get_str(argc, argv, 5).unwrap_or_default();
        if let Some(buf) = ed.buffers.get_mut(buf_key) {
            buf.decor_set_inline(&layer, line, col, text, face.clone());
            let mut data = std::collections::HashMap::new();
            data.insert("buffer".to_string(), buf_key.to_string());
            data.insert("layer".to_string(), layer);
            ed.events.emit("decoration-changed", data);
        }
        conv::nil()
    })
}

/// (buffer/decor-set-eol buf layer line text face)
///
/// Insert or replace an `EndOfLine` decoration for `line` in the named layer.
unsafe extern "C-unwind" fn c_decor_set_eol(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = conv::get_int(argc, argv, 0).map(|k| k as usize) else {
            conv::signal_err("buffer/decor-set-eol requires a buffer key")
        };
        let layer = conv::get_str(argc, argv, 1).unwrap_or_default();
        let line  = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        let text  = conv::get_str(argc, argv, 3).unwrap_or_default();
        let face  = conv::get_str(argc, argv, 4).unwrap_or_default();
        if let Some(buf) = ed.buffers.get_mut(buf_key) {
            buf.decor_set_eol(&layer, line, text, face);
            let mut data = std::collections::HashMap::new();
            data.insert("buffer".to_string(), buf_key.to_string());
            data.insert("layer".to_string(), layer);
            ed.events.emit("decoration-changed", data);
        }
        conv::nil()
    })
}

/// (buffer/decor-set-prefix buf layer line text face)
///
/// Insert or replace a `LinePrefix` decoration for `line` in the named layer.
/// `render_frame` reserves a margin column wide enough for the longest prefix.
unsafe extern "C-unwind" fn c_decor_set_prefix(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = conv::get_int(argc, argv, 0).map(|k| k as usize) else {
            conv::signal_err("buffer/decor-set-prefix requires a buffer key")
        };
        let layer = conv::get_str(argc, argv, 1).unwrap_or_default();
        let line  = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        let text  = conv::get_str(argc, argv, 3).unwrap_or_default();
        let face  = conv::get_str(argc, argv, 4).unwrap_or_default();
        if let Some(buf) = ed.buffers.get_mut(buf_key) {
            buf.decor_set_prefix(&layer, line, text, face);
            let mut data = std::collections::HashMap::new();
            data.insert("buffer".to_string(), buf_key.to_string());
            data.insert("layer".to_string(), layer);
            ed.events.emit("decoration-changed", data);
        }
        conv::nil()
    })
}

/// (buffer/decor-clear-layer buf layer)
///
/// Remove all decorations in the named layer.  The layer itself is removed.
unsafe extern "C-unwind" fn c_decor_clear_layer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = conv::get_int(argc, argv, 0).map(|k| k as usize) else {
            conv::signal_err("buffer/decor-clear-layer requires a buffer key")
        };
        let layer = conv::get_str(argc, argv, 1).unwrap_or_default();
        if let Some(buf) = ed.buffers.get_mut(buf_key) {
            buf.decor_clear_layer(&layer);
        }
        conv::nil()
    })
}

/// (buffer/decor-clear buf)
///
/// Remove all decoration layers from the buffer.
unsafe extern "C-unwind" fn c_decor_clear(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = conv::get_int(argc, argv, 0).map(|k| k as usize) else {
            conv::signal_err("buffer/decor-clear requires a buffer key")
        };
        if let Some(buf) = ed.buffers.get_mut(buf_key) {
            buf.decor_clear();
        }
        conv::nil()
    })
}

/// (buffer/decor-get buf layer) → [{:type :inline/:eol/:prefix :line n :col n :text "…" :face "…"} …]
///
/// Return all decorations in the named layer as an array of tables.
unsafe extern "C-unwind" fn c_decor_get(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(buf_key) = conv::get_int(argc, argv, 0).map(|k| k as usize) else {
            return conv::nil();
        };
        let layer = conv::get_str(argc, argv, 1).unwrap_or_default();
        let buf = match ed.buffers.get(buf_key) {
            Some(b) => b,
            None => return janet_wrap_array(janet_array(0)),
        };
        let decorations = match buf.decoration_layers.get(&layer) {
            Some(v) => v,
            None    => return janet_wrap_array(janet_array(0)),
        };
        let arr = janet_array(decorations.len() as i32);
        for d in decorations {
            use crate::buffer::Decoration;
            let tbl = janet_table(5);
            match d {
                Decoration::InlineText { line, col, text, face } => {
                    janet_table_put(tbl, conv::keyword("type"),  conv::keyword("inline"));
                    janet_table_put(tbl, conv::keyword("line"),  conv::integer(*line as i32));
                    janet_table_put(tbl, conv::keyword("col"),   conv::integer(*col as i32));
                    janet_table_put(tbl, conv::keyword("text"),  conv::string(text));
                    janet_table_put(tbl, conv::keyword("face"),  conv::string(face));
                }
                Decoration::EndOfLine { line, text, face } => {
                    janet_table_put(tbl, conv::keyword("type"),  conv::keyword("eol"));
                    janet_table_put(tbl, conv::keyword("line"),  conv::integer(*line as i32));
                    janet_table_put(tbl, conv::keyword("col"),   conv::integer(0));
                    janet_table_put(tbl, conv::keyword("text"),  conv::string(text));
                    janet_table_put(tbl, conv::keyword("face"),  conv::string(face));
                }
                Decoration::LinePrefix { line, text, face } => {
                    janet_table_put(tbl, conv::keyword("type"),  conv::keyword("prefix"));
                    janet_table_put(tbl, conv::keyword("line"),  conv::integer(*line as i32));
                    janet_table_put(tbl, conv::keyword("col"),   conv::integer(0));
                    janet_table_put(tbl, conv::keyword("text"),  conv::string(text));
                    janet_table_put(tbl, conv::keyword("face"),  conv::string(face));
                }
            }
            janet_array_push(arr, janet_wrap_table(tbl));
        }
        janet_wrap_array(arr)
    })
}

/// (buffer/decor-count buf layer) → integer
///
/// Return the number of decorations in the named layer without allocating
/// the full list.
unsafe extern "C-unwind" fn c_decor_count(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = conv::get_int(argc, argv, 0).map(|k| k as usize) else {
            return conv::integer(0);
        };
        let layer = conv::get_str(argc, argv, 1).unwrap_or_default();
        let count = ed.buffers.get(buf_key)
            .map(|b| b.decor_count_layer(&layer))
            .unwrap_or(0);
        conv::integer(count as i32)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"buffer/decor-set-inline".as_ptr() as *const _,
            cfun: Some(c_decor_set_inline),
            documentation: c"Set an inline text decoration at (line, col)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/decor-set-eol".as_ptr() as *const _,
            cfun: Some(c_decor_set_eol),
            documentation: c"Set an end-of-line decoration on a line".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/decor-set-prefix".as_ptr() as *const _,
            cfun: Some(c_decor_set_prefix),
            documentation: c"Set a line-prefix decoration in the gutter margin".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/decor-clear-layer".as_ptr() as *const _,
            cfun: Some(c_decor_clear_layer),
            documentation: c"Remove all decorations in a named layer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/decor-clear".as_ptr() as *const _,
            cfun: Some(c_decor_clear),
            documentation: c"Remove all decoration layers from a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/decor-get".as_ptr() as *const _,
            cfun: Some(c_decor_get),
            documentation: c"Return all decorations in a layer as an array".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/decor-count".as_ptr() as *const _,
            cfun: Some(c_decor_count),
            documentation: c"Return the number of decorations in a layer".as_ptr() as *const _,
        },
    ]
}
