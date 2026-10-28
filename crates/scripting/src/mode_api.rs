//! Janet API for reading and writing mode state (command-line input, completions).

use super::conv;
use super::with_editor;
use evil_janet::*;

/// Extract a Vec<String> from a Janet array or tuple of strings/keywords.
unsafe fn janet_string_array(v: Janet) -> Vec<String> {
    let n = if janet_checktype(v, JanetType_JANET_ARRAY) != 0 {
        (&*janet_unwrap_array(v)).count
    } else if janet_checktype(v, JanetType_JANET_TUPLE) != 0 {
        (&*janet_tuple_head(janet_unwrap_tuple(v))).length
    } else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        let elem = if janet_checktype(v, JanetType_JANET_ARRAY) != 0 {
            (&*janet_unwrap_array(v)).data.add(i as usize).read()
        } else {
            janet_unwrap_tuple(v).add(i as usize).read()
        };
        if janet_checktype(elem, JanetType_JANET_STRING) != 0
            || janet_checktype(elem, JanetType_JANET_KEYWORD) != 0
        {
            let ptr = janet_unwrap_string(elem);
            if !ptr.is_null() {
                let s = std::ffi::CStr::from_ptr(ptr as *const i8)
                    .to_string_lossy()
                    .into_owned();
                out.push(s);
            }
        }
    }
    out
}

/// (editor/set-completions items &opt idx) → nil
/// Populate the completion popup with `items` (array of strings) and select index `idx`.
/// Shows the popup immediately; dismissed automatically when the user types a character.
unsafe extern "C-unwind" fn c_editor_set_completions(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            return conv::nil();
        }
        let items = janet_string_array(*argv.add(0));
        let idx = conv::get_int(argc, argv, 1).unwrap_or(0).max(0) as usize;
        let idx = idx.min(items.len().saturating_sub(1));
        ed.completion.visible = !items.is_empty();
        ed.completion.idx = idx;
        ed.completion.items = items
            .into_iter()
            .map(kernel::state::completion::CompletionItem::word)
            .collect();
        conv::nil()
    })
}

/// (editor/clear-completions) → nil
/// Dismiss the completion popup and discard the candidate list.
unsafe extern "C-unwind" fn c_editor_clear_completions(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        ed.completion.visible = false;
        ed.completion.items.clear();
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/set-completions".as_ptr() as *const _,
            cfun: Some(
                c_editor_set_completions as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Show a completion popup with the given items and selected index"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/clear-completions".as_ptr() as *const _,
            cfun: Some(
                c_editor_clear_completions as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Dismiss the completion popup".as_ptr() as *const _,
        },
    ]
}
