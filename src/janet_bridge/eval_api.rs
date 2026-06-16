//! Janet C function for interactive Janet evaluation (Sprint 12).

use evil_janet::*;
use super::conv;
use super::with_editor;

/// (editor/eval expr) → result-string
///
/// Evaluate a Janet expression in the live runtime and return its printed
/// representation as a string.  On error, returns the error message as a
/// string rather than signalling — callers route both to `*janet-output*`.
unsafe extern "C-unwind" fn c_editor_eval(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|_ed| unsafe {
        let expr = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("editor/eval requires an expression string"),
        };
        match crate::janet_bridge::eval_result(&expr) {
            Ok(s) => conv::string(&s),
            Err(e) => conv::string(&e),
        }
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/eval".as_ptr() as *const _,
            cfun: Some(c_editor_eval as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Evaluate a Janet expression string and return the printed result".as_ptr() as *const _,
        },
    ]
}
