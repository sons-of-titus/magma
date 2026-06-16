//! Janet API — register: get, set, yanked-text.

use evil_janet::*;
use super::conv;
use super::with_editor;

unsafe extern "C-unwind" fn c_register_get(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        match ed.registers.get(&name) {
            Some(val) => conv::string(val),
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_register_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("register/set requires a name")
        };
        let Some(value) = conv::get_str(argc, argv, 1) else {
            conv::signal_err("register/set requires a value")
        };
        ed.registers.insert(name, value);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_register_yanked_text(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| match &ed.yanked_text {
        Some(t) => conv::string(t),
        None => conv::nil(),
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"register/get".as_ptr() as *const _,
            cfun: Some(c_register_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get the value of a named register".as_ptr() as *const _,
        },
        JanetReg {
            name: c"register/set".as_ptr() as *const _,
            cfun: Some(c_register_set as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the value of a named register".as_ptr() as *const _,
        },
        JanetReg {
            name: c"register/yanked-text".as_ptr() as *const _,
            cfun: Some(c_register_yanked_text as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the last yanked/deleted text".as_ptr() as *const _,
        },
    ]
}
