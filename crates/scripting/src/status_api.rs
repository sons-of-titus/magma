//! Janet API for plugin-contributed status-bar segments.
//!
//! Registered C functions:
//!   (status/set-segment id text &opt kind) → :ok

use super::conv;
use super::with_editor;
use evil_janet::*;

/// `(status/set-segment id text &opt kind)` → `:ok`
///
/// Set or update a status pill.  An empty `text` removes the segment —
/// plugins clear their own indicators on teardown, crash, or idle.
unsafe extern "C-unwind" fn c_status_set_segment(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = match conv::get_str(argc, argv, 0) {
            Some(v) => v,
            None => conv::signal_err("status/set-segment: id string required"),
        };
        let text = conv::get_str(argc, argv, 1).unwrap_or_default();
        let kind = conv::get_str(argc, argv, 2).unwrap_or_else(|| "info".to_string());
        kernel::state::status_segment::set(
            &mut ed.status_segments,
            kernel::state::status_segment::StatusSegment { id, text, kind },
        );
        conv::keyword("ok")
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![JanetReg {
        name: c"status/set-segment".as_ptr() as *const _,
        cfun: Some(
            c_status_set_segment as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
        ),
        documentation: c"Set a status-bar segment: id, text (empty removes it), optional kind (info|warn|error)"
            .as_ptr() as *const _,
    }]
}
