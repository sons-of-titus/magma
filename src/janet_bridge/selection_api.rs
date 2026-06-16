//! Janet API — selection: get, set, clear.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::state::mode::Selection;
use crate::event::payload::*;
use crate::event::keys;

unsafe extern "C-unwind" fn c_selection_get(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        match &ed.selection {
            Some(sel) => {
                let tbl = janet_table(2);
                janet_table_put(tbl, conv::keyword("anchor"), conv::integer(sel.anchor as i32));
                janet_table_put(tbl, conv::keyword("kind"), conv::string(&sel.kind));
                janet_wrap_table(tbl)
            }
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_selection_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(anchor) = conv::get_int(argc, argv, 0) else { return conv::nil(); };
        let kind = conv::get_str(argc, argv, 1).unwrap_or_else(|| "char".to_string());
        ed.selection = Some(Selection { anchor: anchor as usize, kind });
        ed.events.emit_typed(keys::events::SELECTION_CHANGED, EmptyPayload);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_selection_clear(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        ed.selection = None;
        ed.events.emit_typed(keys::events::SELECTION_CLEARED, EmptyPayload);
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"selection/get".as_ptr() as *const _,
            cfun: Some(c_selection_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the current selection as {:anchor n :kind s}, or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"selection/set".as_ptr() as *const _,
            cfun: Some(c_selection_set as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the visual selection anchor and kind".as_ptr() as *const _,
        },
        JanetReg {
            name: c"selection/clear".as_ptr() as *const _,
            cfun: Some(c_selection_clear as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Clear the visual selection".as_ptr() as *const _,
        },
    ]
}
