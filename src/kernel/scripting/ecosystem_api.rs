//! Janet API for the plugin ecosystem: module paths, quickfix list, mark ring, magma utilities.

use std::time::SystemTime;

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::task::QuickfixEntry;

// ── Module paths ──────────────────────────────────────────────────────────────

/// (editor/module-path) → [path ...]
unsafe extern "C-unwind" fn c_editor_module_path(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_array(ed.module_paths.len() as i32);
        for path in &ed.module_paths {
            janet_array_push(arr, conv::string(path));
        }
        janet_wrap_array(arr)
    })
}

/// (editor/module-path-add path)
unsafe extern "C-unwind" fn c_editor_module_path_add(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(path) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/module-path-add requires a path")
        };
        if !ed.module_paths.contains(&path) {
            ed.module_paths.push(path);
        }
        conv::nil()
    })
}

// ── Keymap layer listing ──────────────────────────────────────────────────────

/// (keymap/list-layer layer) → [[key cmd] ...]
unsafe extern "C-unwind" fn c_keymap_list_layer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(layer) = conv::get_str(argc, argv, 0) else {
            return janet_wrap_array(janet_array(0));
        };
        let bindings = ed.keymaps.list_layer(&layer);
        let arr = janet_array(bindings.len() as i32);
        for (k, v) in &bindings {
            let pair = janet_array(2);
            janet_array_push(pair, conv::string(k));
            janet_array_push(pair, conv::string(v));
            janet_array_push(arr, janet_wrap_array(pair));
        }
        janet_wrap_array(arr)
    })
}

// ── Quickfix list ─────────────────────────────────────────────────────────────

/// (quickfix/set [{:filename f :line l :col c :message m} ...])
/// Replaces the quickfix list. Accepts arrays or tuples of tables or structs.
unsafe extern "C-unwind" fn c_quickfix_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            ed.quickfix_list.clear();
            ed.quickfix_index = 0;
            return conv::nil();
        }
        let v = *argv.add(0);
        let entries = ds_iter(v)
            .filter_map(|item| parse_qf_entry(item))
            .collect();
        ed.quickfix_list = entries;
        ed.quickfix_index = 0;
        conv::nil()
    })
}

/// (quickfix/get) → [{:filename f :line l :col c :message m} ...]
unsafe extern "C-unwind" fn c_quickfix_get(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_array(ed.quickfix_list.len() as i32);
        for entry in &ed.quickfix_list {
            let tbl = janet_table(4);
            janet_table_put(tbl, conv::keyword("filename"), conv::string(&entry.filename));
            janet_table_put(tbl, conv::keyword("line"), conv::integer(entry.line as i32));
            janet_table_put(tbl, conv::keyword("col"), conv::integer(entry.col as i32));
            janet_table_put(tbl, conv::keyword("message"), conv::string(&entry.message));
            janet_array_push(arr, janet_wrap_table(tbl));
        }
        janet_wrap_array(arr)
    })
}

// ── Mark ring ─────────────────────────────────────────────────────────────────

const MARK_RING_MAX: usize = 100;

/// (editor/mark-ring-push path offset)
unsafe extern "C-unwind" fn c_mark_ring_push(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(path) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/mark-ring-push requires a path")
        };
        let offset = conv::get_int(argc, argv, 1).unwrap_or(0).max(0) as usize;
        if ed.mark_ring.len() >= MARK_RING_MAX {
            ed.mark_ring.remove(0);
        }
        ed.mark_ring.push((path, offset));
        conv::nil()
    })
}

/// (editor/mark-ring-pop) → {:path p :offset n} or nil
unsafe extern "C-unwind" fn c_mark_ring_pop(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        match ed.mark_ring.pop() {
            Some((path, offset)) => {
                let tbl = janet_table(2);
                janet_table_put(tbl, conv::keyword("path"), conv::string(&path));
                janet_table_put(tbl, conv::keyword("offset"), conv::integer(offset as i32));
                janet_wrap_table(tbl)
            }
            None => conv::nil(),
        }
    })
}

/// (editor/mark-ring-peek) → {:path p :offset n} or nil
unsafe extern "C-unwind" fn c_mark_ring_peek(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        match ed.mark_ring.last() {
            Some((path, offset)) => {
                let tbl = janet_table(2);
                janet_table_put(tbl, conv::keyword("path"), conv::string(path));
                janet_table_put(tbl, conv::keyword("offset"), conv::integer(*offset as i32));
                janet_wrap_table(tbl)
            }
            None => conv::nil(),
        }
    })
}

/// (editor/mark-ring-len) → n
unsafe extern "C-unwind" fn c_mark_ring_len(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| conv::integer(ed.mark_ring.len() as i32))
}

// ── Font invalidation ─────────────────────────────────────────────────────────

/// (editor/font-invalidate) — mark the GPU glyph atlas as dirty so it is rebuilt next frame.
unsafe extern "C-unwind" fn c_editor_font_invalidate(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        ed.font_changed = true;
        conv::nil()
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Read a string/keyword field from a Janet struct or table via janet_in.
unsafe fn ds_get_str(ds: Janet, key: &str) -> Option<String> { unsafe {
    let v = janet_in(ds, conv::keyword(key));
    if janet_checktype(v, JanetType_JANET_STRING) != 0
        || janet_checktype(v, JanetType_JANET_KEYWORD) != 0
    {
        let ptr = janet_unwrap_string(v);
        if !ptr.is_null() {
            let cstr = std::ffi::CStr::from_ptr(ptr as *const i8);
            return Some(cstr.to_string_lossy().into_owned());
        }
    }
    None
}}

/// Read an integer field from a Janet struct or table via janet_in.
unsafe fn ds_get_int(ds: Janet, key: &str) -> Option<i32> { unsafe {
    let v = janet_in(ds, conv::keyword(key));
    if janet_checktype(v, JanetType_JANET_NUMBER) != 0 {
        return Some(janet_unwrap_integer(v));
    }
    None
}}

/// Iterate Janet values from either an array or a tuple.
unsafe fn ds_iter(v: Janet) -> Box<dyn Iterator<Item = Janet>> { unsafe {
    if janet_checktype(v, JanetType_JANET_ARRAY) != 0 {
        let arr = janet_unwrap_array(v);
        let count = (*arr).count as usize;
        let data = (*arr).data;
        Box::new((0..count).map(move |i| *data.add(i)))
    } else if janet_checktype(v, JanetType_JANET_TUPLE) != 0 {
        let tup = janet_unwrap_tuple(v);
        let head = &*janet_tuple_head(tup);
        let count = head.length as usize;
        Box::new((0..count).map(move |i| *tup.add(i)))
    } else {
        Box::new(std::iter::empty())
    }
}}

/// Parse a Janet struct/table into a QuickfixEntry.
unsafe fn parse_qf_entry(item: Janet) -> Option<QuickfixEntry> { unsafe {
    let is_ds = janet_checktype(item, JanetType_JANET_TABLE) != 0
        || janet_checktype(item, JanetType_JANET_STRUCT) != 0;
    if !is_ds { return None; }
    let filename = ds_get_str(item, "filename").unwrap_or_default();
    let line = ds_get_int(item, "line").unwrap_or(1).max(1) as usize;
    let col = ds_get_int(item, "col").unwrap_or(1).max(1) as usize;
    let message = ds_get_str(item, "message").unwrap_or_default();
    Some(QuickfixEntry { filename, line, col, message })
}}

// ── magma/time-now ─────────────────────────────────────────────────────────

/// (magma/time-now) → string
unsafe extern "C-unwind" fn c_magma_time_now(_argc: i32, _argv: *mut Janet) -> Janet {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();

    let secs_of_day = total_secs % 86400;
    let hours = secs_of_day / 3600;
    let minutes = (secs_of_day % 3600) / 60;
    let seconds = secs_of_day % 60;

    let mut days = (total_secs / 86400) as i64;
    let mut year = 1970i64;
    loop {
        let days_in_year = if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1usize;
    let mut remaining = days;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md { month = i + 1; break; }
        remaining -= md;
    }
    let day = remaining + 1;

    let s = format!("{year}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02}");
    conv::string(&s)
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"module/path".as_ptr() as *const _,
            cfun: Some(c_editor_module_path as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the list of plugin search directories".as_ptr() as *const _,
        },
        JanetReg {
            name: c"module/path-add".as_ptr() as *const _,
            cfun: Some(c_editor_module_path_add as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Add a directory to the plugin search path".as_ptr() as *const _,
        },
        JanetReg {
            name: c"keymap/list-layer".as_ptr() as *const _,
            cfun: Some(c_keymap_list_layer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return all [[key cmd] ...] pairs in a named keymap layer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"quickfix/set".as_ptr() as *const _,
            cfun: Some(c_quickfix_set as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Replace the quickfix list with [{:filename :line :col :message} ...]".as_ptr() as *const _,
        },
        JanetReg {
            name: c"quickfix/get".as_ptr() as *const _,
            cfun: Some(c_quickfix_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the quickfix list as [{:filename :line :col :message} ...]".as_ptr() as *const _,
        },
        JanetReg {
            name: c"mark-ring/push".as_ptr() as *const _,
            cfun: Some(c_mark_ring_push as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Push a (path offset) position onto the mark ring".as_ptr() as *const _,
        },
        JanetReg {
            name: c"mark-ring/pop".as_ptr() as *const _,
            cfun: Some(c_mark_ring_pop as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Pop the most recent position from the mark ring".as_ptr() as *const _,
        },
        JanetReg {
            name: c"mark-ring/peek".as_ptr() as *const _,
            cfun: Some(c_mark_ring_peek as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Peek at the most recent mark ring position without removing it".as_ptr() as *const _,
        },
        JanetReg {
            name: c"mark-ring/len".as_ptr() as *const _,
            cfun: Some(c_mark_ring_len as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the number of entries in the mark ring".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/invalidate".as_ptr() as *const _,
            cfun: Some(c_editor_font_invalidate as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Mark the GPU glyph atlas dirty so it is rebuilt on the next frame".as_ptr() as *const _,
        },
        JanetReg {
            name: c"magma/time-now".as_ptr() as *const _,
            cfun: Some(c_magma_time_now as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the current time as YYYY-MM-DD HH:MM:SS".as_ptr() as *const _,
        },
    ]
}
