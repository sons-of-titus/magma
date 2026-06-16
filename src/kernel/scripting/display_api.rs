//! Janet API for display-related settings — cursor shape, appearance.
//! Registered as `extern "C-unwind"` functions via evil-janet.

use evil_janet::*;
use super::conv;
use super::with_editor;

/// (editor/set-cursor-shape shape) → nil
/// shape is one of: "block", "beam", "underline"
unsafe extern "C-unwind" fn c_editor_set_cursor_shape(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(shape) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/set-cursor-shape requires a shape name")
        };
        match shape.as_str() {
            "block" | "beam" | "underline" => {
                ed.cursor_shape = shape.clone();
            }
            other => conv::signal_err(&format!(
                "editor/set-cursor-shape: unknown shape '{}' (expected block/beam/underline)",
                other
            )),
        }
        conv::nil()
    })
}

/// (editor/cursor-shape) → string
unsafe extern "C-unwind" fn c_editor_cursor_shape(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        conv::string(&ed.cursor_shape)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/set-cursor-shape".as_ptr() as *const _,
            cfun: Some(c_editor_set_cursor_shape as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set cursor shape (block/beam/underline)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/cursor-shape".as_ptr() as *const _,
            cfun: Some(c_editor_cursor_shape as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return current cursor shape string".as_ptr() as *const _,
        },
    ]
}
