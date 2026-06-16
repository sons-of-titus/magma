//! Janet API functions for the keymap system — registered as `extern "C"` via evil-janet.

use evil_janet::*;
use super::conv;
use super::with_editor;

/// (keymap/set key command &opt layer)
unsafe extern "C-unwind" fn c_keymap_set(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_str(argc, argv, 0);
        let cmd = conv::get_str(argc, argv, 1);
        if let (Some(key), Some(cmd)) = (key, cmd) {
            match conv::get_opt_str(argc, argv, 2) {
                Some(Some(layer)) => ed.keymaps.set_layer(&layer, &key, &cmd),
                _ => ed.keymaps.set(&key, &cmd),
            }
        }
        conv::nil()
    })
}

/// (keymap/unset key &opt layer)
unsafe extern "C-unwind" fn c_keymap_unset(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let Some(key) = conv::get_str(argc, argv, 0) {
            match conv::get_opt_str(argc, argv, 1) {
                Some(Some(layer)) => ed.keymaps.unset_layer(&layer, &key),
                _ => ed.keymaps.unset(&key),
            }
        }
        conv::nil()
    })
}

/// (keymap/describe key) → command-name or nil
unsafe extern "C-unwind" fn c_keymap_describe(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let Some(key) = conv::get_str(argc, argv, 0)
            && let Some(cmd) = ed.keymaps.describe(&key) {
                return conv::string(&cmd);
            }
        conv::nil()
    })
}

/// (keymap/list &opt layer) → array or table
unsafe extern "C-unwind" fn c_keymap_list(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let Some(Some(layer)) = conv::get_opt_str(argc, argv, 0) {
            let bindings = ed.keymaps.list_layer(&layer);
            let arr = janet_wrap_array(janet_array(bindings.len() as i32));
            for (k, v) in &bindings {
                let pair = janet_wrap_array(janet_array(2));
                janet_array_push(janet_unwrap_array(pair), conv::string(k));
                janet_array_push(janet_unwrap_array(pair), conv::string(v));
                janet_array_push(janet_unwrap_array(arr), pair);
            }
            arr
        } else {
            let all = ed.keymaps.list_all();
            let tbl = janet_wrap_table(janet_table(0));
            for (layer, bindings) in &all {
                let arr = janet_wrap_array(janet_array(bindings.len() as i32));
                for (k, v) in bindings {
                    let pair = janet_wrap_array(janet_array(2));
                    janet_array_push(janet_unwrap_array(pair), conv::string(k));
                    janet_array_push(janet_unwrap_array(pair), conv::string(v));
                    janet_array_push(janet_unwrap_array(arr), pair);
                }
                janet_table_put(
                    janet_unwrap_table(tbl),
                    conv::keyword(layer),
                    arr,
                );
            }
            tbl
        }
    })
}

/// (keymap/push-layer name)
unsafe extern "C-unwind" fn c_keymap_push_layer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let Some(name) = conv::get_str(argc, argv, 0) {
            ed.keymaps.push_layer(&name);
        }
        conv::nil()
    })
}

/// (keymap/pop-layer name)
unsafe extern "C-unwind" fn c_keymap_pop_layer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let Some(name) = conv::get_str(argc, argv, 0) {
            ed.keymaps.pop_layer(&name);
        }
        conv::nil()
    })
}

/// (keymap/list-layers) → [name ...]
unsafe extern "C-unwind" fn c_keymap_list_layers(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let layers = ed.keymaps.active_layers();
        let arr = janet_wrap_array(janet_array(layers.len() as i32));
        for name in &layers {
            janet_array_push(janet_unwrap_array(arr), conv::string(name));
        }
        arr
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"keymap/set".as_ptr() as *const _,
            cfun: Some(c_keymap_set as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set a keybinding (key command &opt layer)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"keymap/unset".as_ptr() as *const _,
            cfun: Some(c_keymap_unset as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Remove a keybinding (key &opt layer)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"keymap/describe".as_ptr() as *const _,
            cfun: Some(c_keymap_describe as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Describe what a key is bound to".as_ptr() as *const _,
        },
        JanetReg {
            name: c"keymap/list".as_ptr() as *const _,
            cfun: Some(c_keymap_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List bindings (&opt layer)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"keymap/push-layer".as_ptr() as *const _,
            cfun: Some(c_keymap_push_layer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Activate a keymap layer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"keymap/pop-layer".as_ptr() as *const _,
            cfun: Some(c_keymap_pop_layer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Deactivate a keymap layer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"keymap/list-layers".as_ptr() as *const _,
            cfun: Some(c_keymap_list_layers as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List active layers".as_ptr() as *const _,
        },
    ]
}
