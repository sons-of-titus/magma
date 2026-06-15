//! Janet API — buffer rendering: highlights, highlight-layers, folds.

use evil_janet::*;
use super::conv;
use super::with_editor;

unsafe fn index_seq(seq: Janet, i: i32) -> Janet {
    if unsafe { janet_checktype(seq, JanetType_JANET_ARRAY) } != 0 {
        let arr = unsafe { &*janet_unwrap_array(seq) };
        unsafe { arr.data.add(i as usize).read() }
    } else if unsafe { janet_checktype(seq, JanetType_JANET_TUPLE) } != 0 {
        let tup = unsafe { janet_unwrap_tuple(seq) };
        unsafe { tup.add(i as usize).read() }
    } else {
        unsafe { janet_wrap_nil() }
    }
}

unsafe fn len_seq(seq: Janet) -> i32 {
    if unsafe { janet_checktype(seq, JanetType_JANET_ARRAY) } != 0 {
        let arr = unsafe { &*janet_unwrap_array(seq) };
        arr.count
    } else if unsafe { janet_checktype(seq, JanetType_JANET_TUPLE) } != 0 {
        let tup = unsafe { janet_unwrap_tuple(seq) };
        let head = unsafe { &*janet_tuple_head(tup) };
        head.length
    } else {
        0
    }
}

unsafe fn parse_highlight_val(v: Janet) -> (usize, usize, String) {
    let start = janet_unwrap_integer(index_seq(v, 0)) as usize;
    let end   = janet_unwrap_integer(index_seq(v, 1)) as usize;
    let face = if len_seq(v) >= 3 {
        let fv = index_seq(v, 2);
        if janet_checktype(fv, JanetType_JANET_STRING) != 0
            || janet_checktype(fv, JanetType_JANET_KEYWORD) != 0
        {
            let ptr = janet_unwrap_string(fv);
            if ptr.is_null() { "highlight".to_string() }
            else { std::ffi::CStr::from_ptr(ptr as *const i8).to_string_lossy().into_owned() }
        } else {
            "highlight".to_string()
        }
    } else {
        "highlight".to_string()
    };
    (start, end, face)
}

unsafe fn parse_highlight_ranges(argc: i32, argv: *mut Janet, arg_idx: i32) -> Vec<(usize, usize, String)> {
    if argc <= arg_idx {
        return Vec::new();
    }
    let outer = argv.add(arg_idx as usize).read();
    let n = len_seq(outer);
    let mut ranges = Vec::with_capacity(n as usize);
    for i in 0..n {
        let inner = index_seq(outer, i);
        let t = parse_highlight_val(inner);
        ranges.push(t);
    }
    ranges
}

unsafe extern "C-unwind" fn c_buffer_set_highlights(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(buf) = ed.buffers.get_mut(key) else { return conv::nil() };
        if argc < 2 { buf.clear_highlights(); return conv::nil(); }
        let ranges = parse_highlight_ranges(argc, argv, 1);
        buf.set_highlights(ranges);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_clear_highlights(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        if let Some(buf) = ed.buffers.get_mut(key) {
            buf.clear_highlights();
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_set_highlights_layer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(layer) = conv::get_str(argc, argv, 1) else {
            return conv::nil();
        };
        let Some(buf) = ed.buffers.get_mut(key) else { return conv::nil() };
        if argc < 3 { buf.clear_highlights_layer(&layer); return conv::nil(); }
        let ranges = parse_highlight_ranges(argc, argv, 2);
        buf.set_highlights_layer(&layer, ranges);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_clear_highlights_layer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(layer) = conv::get_str(argc, argv, 1) else {
            return conv::nil();
        };
        if let Some(buf) = ed.buffers.get_mut(key) {
            buf.clear_highlights_layer(&layer);
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_fold(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let start = conv::get_int(argc, argv, 1).unwrap_or(0) as usize;
        let end = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        if let Some(buf) = ed.buffers.get_mut(key) {
            buf.add_fold(start, end);
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_unfold(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let start = conv::get_int(argc, argv, 1).unwrap_or(0) as usize;
        let end = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        if let Some(buf) = ed.buffers.get_mut(key) {
            buf.remove_fold(start, end);
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_unfold_all(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        if let Some(buf) = ed.buffers.get_mut(key) {
            buf.clear_folds();
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_folds(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let folds = match ed.buffers.get(key) {
            Some(buf) => buf.folds.clone(),
            None => Vec::new(),
        };
        let arr = janet_array(folds.len() as i32);
        for (s, e) in folds {
            let inner = janet_array(2);
            janet_array_push(inner, conv::integer(s as i32));
            janet_array_push(inner, conv::integer(e as i32));
            janet_array_push(arr, janet_wrap_array(inner));
        }
        janet_wrap_array(arr)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"buffer/set-highlights".as_ptr() as *const _,
            cfun: Some(c_buffer_set_highlights as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set highlight byte-ranges for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/clear-highlights".as_ptr() as *const _,
            cfun: Some(c_buffer_clear_highlights as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Clear all highlights for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/set-highlights-layer".as_ptr() as *const _,
            cfun: Some(c_buffer_set_highlights_layer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set highlight ranges for a named layer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/clear-highlights-layer".as_ptr() as *const _,
            cfun: Some(c_buffer_clear_highlights_layer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Clear highlight ranges for a named layer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/fold".as_ptr() as *const _,
            cfun: Some(c_buffer_fold as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Fold a byte range in a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/unfold".as_ptr() as *const _,
            cfun: Some(c_buffer_unfold as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Unfold a byte range in a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/unfold-all".as_ptr() as *const _,
            cfun: Some(c_buffer_unfold_all as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Remove all folds from a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/folds".as_ptr() as *const _,
            cfun: Some(c_buffer_folds as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get all fold ranges for a buffer".as_ptr() as *const _,
        },
    ]
}
