//! Janet API — clipboard: get, set.

use evil_janet::*;
use super::conv;
use super::with_editor;

unsafe extern "C-unwind" fn c_clipboard_get(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| match ed.clipboard.get_text() {
        Some(t) => conv::string(&t),
        None => conv::nil(),
    })
}

unsafe extern "C-unwind" fn c_clipboard_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(text) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        ed.clipboard.set_text(&text);
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"clipboard/get".as_ptr() as *const _,
            cfun: Some(c_clipboard_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get system clipboard text".as_ptr() as *const _,
        },
        JanetReg {
            name: c"clipboard/set".as_ptr() as *const _,
            cfun: Some(c_clipboard_set as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set system clipboard text".as_ptr() as *const _,
        },
    ]
}
