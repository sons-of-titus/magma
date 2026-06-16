//! Janet API — modality: set-mode, mode-name, mode-accepts-text?.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::state::mode::Selection;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

unsafe extern "C-unwind" fn c_editor_set_mode(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else { return conv::nil(); };
        let old_name = ed.editor_mode.name.clone();
        let accepts_text = if argc >= 2 {
            let opts = *argv.add(1);
            let accepts_key = conv::keyword("accepts-text");
            let val = janet_in(opts, accepts_key);
            if janet_checktype(val, JanetType_JANET_BOOLEAN) != 0 {
                janet_unwrap_boolean(val) != 0
            } else {
                ed.editor_mode.accepts_text
            }
        } else {
            name == "insert" || name == "replace"
        };
        ed.editor_mode.name = name.clone();
        ed.editor_mode.accepts_text = accepts_text;
        match name.as_str() {
            "visual" => {
                if ed.selection.is_none() {
                    let buf_id = crate::kernel::input::focused_buffer_id(ed);
                    let anchor = ed.views.get(&buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
                    ed.selection = Some(Selection { anchor, kind: "char".into() });
                }
            }
            "visual-line" => {
                let buf_id = crate::kernel::input::focused_buffer_id(ed);
                let anchor = ed.views.get(&buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
                ed.selection = Some(Selection { anchor, kind: "line".into() });
            }
            "visual-block" => {
                let buf_id = crate::kernel::input::focused_buffer_id(ed);
                let anchor = ed.views.get(&buf_id).map(|v| v.cursor_offset()).unwrap_or(0);
                ed.selection = Some(Selection { anchor, kind: "block".into() });
            }
            "normal" | "insert" | "replace" | "command" | "cmdline" | "search" => {
                ed.selection = None;
            }
            _ => {}
        }
        ed.events.emit_typed(keys::events::MODE_CHANGED, ModeChangedPayload { from: old_name, to: name });
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_editor_mode_name(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| conv::string(&ed.editor_mode.name))
}

unsafe extern "C-unwind" fn c_editor_mode_accepts_text(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| conv::boolean(ed.editor_mode.accepts_text))
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/set-mode".as_ptr() as *const _,
            cfun: Some(c_editor_set_mode as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the current editor mode name and accepts-text policy".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/mode-name".as_ptr() as *const _,
            cfun: Some(c_editor_mode_name as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the current mode name string".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/mode-accepts-text?".as_ptr() as *const _,
            cfun: Some(c_editor_mode_accepts_text as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return true if unbound keys insert text in the current mode".as_ptr() as *const _,
        },
    ]
}
