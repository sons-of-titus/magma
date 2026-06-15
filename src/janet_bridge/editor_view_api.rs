//! Janet API — editor view: set-running, theme, cursor, buffer navigation, input handling.

use evil_janet::*;
use super::conv;
use super::with_editor;

unsafe extern "C-unwind" fn c_editor_set_running(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc > 0 {
            let v = *argv.add(0);
            ed.running = janet_truthy(v) != 0;
        } else {
            ed.running = false;
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_editor_set_theme(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = match conv::get_str(argc, argv, 0) {
            Some(k) => k,
            None => conv::signal_err("editor/set-theme requires a key"),
        };
        let r = if argc > 1 { conv::get_int(argc, argv, 1).unwrap_or(0) as u8 } else { 0 };
        let g = if argc > 2 { conv::get_int(argc, argv, 2).unwrap_or(0) as u8 } else { 0 };
        let b = if argc > 3 { conv::get_int(argc, argv, 3).unwrap_or(0) as u8 } else { 0 };
        ed.theme.insert(key, (r, g, b));
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_editor_theme(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = match conv::get_str(argc, argv, 0) {
            Some(k) => k,
            None => return conv::nil(),
        };
        let (r, g, b) = ed.theme_color(&key);
        let arr = janet_array(3);
        janet_array_push(arr, conv::integer(r as i32));
        janet_array_push(arr, conv::integer(g as i32));
        janet_array_push(arr, conv::integer(b as i32));
        janet_wrap_array(arr)
    })
}

unsafe extern "C-unwind" fn c_editor_cursor(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let buf_id = crate::input::focused_buffer_id(ed);
        match ed.buffers.get(buf_id) {
            Some(buf) => conv::integer(buf.cursor() as i32),
            None => conv::integer(0),
        }
    })
}

unsafe extern "C-unwind" fn c_editor_focused_buffer(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let id = crate::input::focused_buffer_id(ed);
        conv::integer(id as i32)
    })
}

unsafe extern "C-unwind" fn c_editor_buffer_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_array(ed.buffers.len() as i32);
        for (key, _buf) in &ed.buffers {
            janet_array_push(arr, conv::integer(key as i32));
        }
        janet_wrap_array(arr)
    })
}

unsafe extern "C-unwind" fn c_editor_consume_input(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        ed.input_consumed = true;
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_editor_on_input(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc >= 1 {
            let v = *argv.add(0);
            if janet_checktype(v, JanetType_JANET_NIL) != 0 {
                ed.on_input_fn = None;
            } else if let Some(name) = conv::get_str(argc, argv, 0) {
                ed.on_input_fn = Some(name);
            }
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_editor_on_input_fn(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        match &ed.on_input_fn {
            Some(name) => conv::string(name),
            None => conv::nil(),
        }
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/set-running".as_ptr() as *const _,
            cfun: Some(c_editor_set_running as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the running flag (false to quit)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/set-theme".as_ptr() as *const _,
            cfun: Some(c_editor_set_theme as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set a theme color (key, r, g, b)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/theme".as_ptr() as *const _,
            cfun: Some(c_editor_theme as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get a theme color as [r g b]".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/cursor".as_ptr() as *const _,
            cfun: Some(c_editor_cursor as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return cursor byte offset in focused buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/focused-buffer".as_ptr() as *const _,
            cfun: Some(c_editor_focused_buffer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the slab key of the focused buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/buffer-list".as_ptr() as *const _,
            cfun: Some(c_editor_buffer_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return a list of all buffer slab keys".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/consume-input".as_ptr() as *const _,
            cfun: Some(c_editor_consume_input as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Signal that the current key event is consumed by the on-input interceptor".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/on-input".as_ptr() as *const _,
            cfun: Some(c_editor_on_input as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a raw input interceptor called before keymap dispatch".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/on-input-fn".as_ptr() as *const _,
            cfun: Some(c_editor_on_input_fn as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the currently registered input interceptor name, or nil".as_ptr() as *const _,
        },
    ]
}
