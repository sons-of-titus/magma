//! Janet API for named layout save/restore — Sprint 8.

use evil_janet::*;
use super::conv;
use super::with_editor;

/// (editor/save-layout name)
/// Snapshot the current window tree under the given name.
unsafe extern "C-unwind" fn c_editor_save_layout(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        if let Some(name) = unsafe { conv::get_str(argc, argv, 0) } {
            let windows = ed.windows.windows().to_vec();
            let focused_idx = ed.windows.focused_index();
            ed.saved_layouts.insert(name, (windows, focused_idx));
        }
        conv::nil()
    })
}

/// (editor/restore-layout name)
/// Restore a previously saved window tree.
unsafe extern "C-unwind" fn c_editor_restore_layout(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        if let Some(name) = unsafe { conv::get_str(argc, argv, 0) } {
            if let Some((windows, focused_idx)) = ed.saved_layouts.get(&name).cloned() {
                ed.windows.restore(windows, focused_idx);
            }
        }
        conv::nil()
    })
}

/// (editor/layout-list) → [name ...]
/// Return all saved layout names.
unsafe extern "C-unwind" fn c_editor_layout_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_array(ed.saved_layouts.len() as i32);
        for name in ed.saved_layouts.keys() {
            janet_array_push(arr, conv::string(name));
        }
        janet_wrap_array(arr)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"editor/save-layout".as_ptr() as *const _,
            cfun: Some(c_editor_save_layout as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Save the current window layout under a name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/restore-layout".as_ptr() as *const _,
            cfun: Some(c_editor_restore_layout as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Restore a previously saved window layout".as_ptr() as *const _,
        },
        JanetReg {
            name: c"editor/layout-list".as_ptr() as *const _,
            cfun: Some(c_editor_layout_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List all saved layout names".as_ptr() as *const _,
        },
    ]
}
