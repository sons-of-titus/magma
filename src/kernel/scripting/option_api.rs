//! Janet API — option: list, get, set, get-local, set-local.

use evil_janet::*;
use super::conv;
use super::with_editor;

unsafe extern "C-unwind" fn c_option_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let tbl = janet_table(ed.options.len() as i32);
        for (k, v) in &ed.options {
            janet_table_put(tbl, conv::keyword(k), conv::string(v));
        }
        janet_wrap_table(tbl)
    })
}

unsafe extern "C-unwind" fn c_option_get(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        match ed.options.get(&name) {
            Some(val) => conv::string(val),
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_option_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("option/set requires a name")
        };
        let Some(value) = conv::get_str(argc, argv, 1) else {
            conv::signal_err("option/set requires a value")
        };
        ed.options.insert(name, value);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_option_get_local(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(key) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        let buf_id = crate::kernel::input::focused_buffer_id(ed);
        if let Some(buf) = ed.buffers.get(buf_id)
            && let Some(val) = buf.local_options.get(&key) {
                return conv::string(val);
            }
        match ed.options.get(&key) {
            Some(val) => conv::string(val),
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_option_set_local(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(key) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("option/set-local requires a key")
        };
        let Some(value) = conv::get_str(argc, argv, 1) else {
            conv::signal_err("option/set-local requires a value")
        };
        let buf_id = crate::kernel::input::focused_buffer_id(ed);
        if let Some(buf) = ed.buffers.get_mut(buf_id) {
            buf.local_options.insert(key, value);
        }
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"option/list".as_ptr() as *const _,
            cfun: Some(c_option_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return all editor options as a table".as_ptr() as *const _,
        },
        JanetReg {
            name: c"option/get".as_ptr() as *const _,
            cfun: Some(c_option_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get a single editor option".as_ptr() as *const _,
        },
        JanetReg {
            name: c"option/set".as_ptr() as *const _,
            cfun: Some(c_option_set as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set a single editor option".as_ptr() as *const _,
        },
        JanetReg {
            name: c"option/get-local".as_ptr() as *const _,
            cfun: Some(c_option_get_local as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get a buffer-local option (falls back to global)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"option/set-local".as_ptr() as *const _,
            cfun: Some(c_option_set_local as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set a buffer-local option on the focused buffer".as_ptr() as *const _,
        },
    ]
}
