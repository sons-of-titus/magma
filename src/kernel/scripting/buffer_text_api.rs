//! Janet API — buffer text operations: insert, delete, slice, len, cursor, undo, redo, mark-saved.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

fn emit_buffer_changed(ed: &mut crate::kernel::state::Editor, key: usize) {
    ed.events.emit_typed(keys::events::BUFFER_CHANGED, BufferChangedPayload {
        buffer_id: key.to_string(),
    });
}

unsafe extern "C-unwind" fn c_buffer_insert(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let pos = conv::get_int(argc, argv, 1).unwrap_or(0) as usize;
        let text = conv::get_str(argc, argv, 2).unwrap_or_default();
        debug!("BI: buffer/insert buf={}, pos={}, text_len={}", key, pos, text.len());
        if ed.buffers.get(key).map(|a| a.lock().unwrap().read_only).unwrap_or(false) {
            ed.events.emit_typed(keys::events::BUFFER_READ_ONLY, BufferReadOnlyPayload {
                buffer_id: key.to_string(),
            });
            return conv::nil();
        }
        if let Some(view) = ed.views.get_mut(&key) {
            view.insert(pos, &text);
        }
        emit_buffer_changed(&mut *ed, key);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_delete(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let start = conv::get_int(argc, argv, 1).unwrap_or(0) as usize;
        let end = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        debug!("BD: buffer/delete buf={}, start={}, end={}", key, start, end);
        if ed.buffers.get(key).map(|a| a.lock().unwrap().read_only).unwrap_or(false) {
            ed.events.emit_typed(keys::events::BUFFER_READ_ONLY, BufferReadOnlyPayload {
                buffer_id: key.to_string(),
            });
            return conv::nil();
        }
        if let Some(view) = ed.views.get_mut(&key) {
            view.delete(start, end);
        }
        emit_buffer_changed(&mut *ed, key);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_slice(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let start = conv::get_int(argc, argv, 1).unwrap_or(0) as usize;
        let end = conv::get_int(argc, argv, 2).unwrap_or(0) as usize;
        let s = ed.buffers.get(key)
            .map(|a| a.lock().unwrap().slice(start, end))
            .unwrap_or_default();
        conv::string(&s)
    })
}

unsafe extern "C-unwind" fn c_buffer_len(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let len = ed.buffers.get(key).map(|a| a.lock().unwrap().len()).unwrap_or(0);
        conv::integer(len as i32)
    })
}

unsafe extern "C-unwind" fn c_buffer_cursor(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        match ed.views.get(&key) {
            Some(view) => conv::integer(view.cursor_offset() as i32),
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_buffer_set_cursor(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let pos = conv::get_int(argc, argv, 1).unwrap_or(0) as usize;
        if let Some(view) = ed.views.get_mut(&key) {
            view.set_cursor(pos);
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_undo(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let ok = ed.views.get_mut(&key).map(|v| v.undo()).unwrap_or(false);
        conv::boolean(ok)
    })
}

unsafe extern "C-unwind" fn c_buffer_redo(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let ok = ed.views.get_mut(&key).map(|v| v.redo()).unwrap_or(false);
        conv::boolean(ok)
    })
}

unsafe extern "C-unwind" fn c_buffer_mark_saved(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        if let Some(arc) = ed.buffers.get_mut(key) {
            arc.lock().unwrap().mark_saved();
        }
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"buffer/insert".as_ptr() as *const _,
            cfun: Some(c_buffer_insert as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Insert text at position".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/delete".as_ptr() as *const _,
            cfun: Some(c_buffer_delete as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Delete text from buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/slice".as_ptr() as *const _,
            cfun: Some(c_buffer_slice as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get a substring from buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/len".as_ptr() as *const _,
            cfun: Some(c_buffer_len as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get buffer length".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/cursor".as_ptr() as *const _,
            cfun: Some(c_buffer_cursor as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get cursor position (byte offset)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/set-cursor".as_ptr() as *const _,
            cfun: Some(c_buffer_set_cursor as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set cursor position (byte offset)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/undo".as_ptr() as *const _,
            cfun: Some(c_buffer_undo as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Undo buffer change".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/redo".as_ptr() as *const _,
            cfun: Some(c_buffer_redo as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Redo buffer change".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/mark-saved".as_ptr() as *const _,
            cfun: Some(c_buffer_mark_saved as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Mark buffer as saved (no unsaved changes)".as_ptr() as *const _,
        },
    ]
}
