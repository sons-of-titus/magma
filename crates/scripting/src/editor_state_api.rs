//! Janet API — editor state: mode name and mode detail.

use super::conv;
use super::with_editor;
use evil_janet::*;

unsafe extern "C-unwind" fn c_editor_mode(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| conv::string(&ed.mode_name()))
}

unsafe extern "C-unwind" fn c_editor_mode_detail(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let tbl = janet_table(0);
        let mode_name = ed.mode_name();
        janet_table_put(tbl, conv::keyword("name"), conv::keyword(&mode_name));
        if let Some(sel) = &ed.selection {
            janet_table_put(
                tbl,
                conv::keyword("anchor"),
                conv::integer(sel.anchor as i32),
            );
            janet_table_put(
                tbl,
                conv::keyword("selection-kind"),
                conv::keyword(&sel.kind),
            );
        }
        janet_wrap_table(tbl)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/mode".as_ptr() as *const _,
            cfun: Some(c_editor_mode as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the current mode name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/mode-detail".as_ptr() as *const _,
            cfun: Some(
                c_editor_mode_detail as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Return a table with mode details".as_ptr() as *const _,
        },
    ]
}
