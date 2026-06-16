//! Janet API — plugin-state: get, set, del.

use evil_janet::*;
use super::conv;
use super::with_editor;

unsafe extern "C-unwind" fn c_plugin_state_get(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        match conv::get_str(argc, argv, 0).and_then(|k| ed.plugin_state.get(&k).cloned()) {
            Some(v) => conv::string(&v),
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_plugin_state_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let (Some(k), Some(v)) = (conv::get_str(argc, argv, 0), conv::get_str(argc, argv, 1)) {
            ed.plugin_state.insert(k, v);
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_plugin_state_del(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let Some(k) = conv::get_str(argc, argv, 0) {
            ed.plugin_state.remove(&k);
        }
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"plugin-state/get".as_ptr() as *const _,
            cfun: Some(c_plugin_state_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return a plugin state value by key, or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"plugin-state/set".as_ptr() as *const _,
            cfun: Some(c_plugin_state_set as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Store a string value in plugin state".as_ptr() as *const _,
        },
        JanetReg {
            name: c"plugin-state/del".as_ptr() as *const _,
            cfun: Some(c_plugin_state_del as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Delete a plugin state entry by key".as_ptr() as *const _,
        },
    ]
}
