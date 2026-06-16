//! Janet API functions for special-buffer primitives (Sprint 2).
//! Registered as `extern "C-unwind"` functions via evil-janet.

use evil_janet::*;
use super::conv;
use super::with_editor;

/// (buffer/find-or-create name) → slab-key
///
/// Returns the slab key of the buffer named `name` if it exists, otherwise
/// creates a new empty buffer with that name.  Guarantees exactly one buffer
/// per name.
unsafe extern "C-unwind" fn c_buffer_find_or_create(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    with_editor(|ed| {
        let name = conv::get_str(argc, argv, 0).unwrap_or_default();
        // Search existing buffers first
        for (key, arc) in &ed.buffers {
            if arc.lock().unwrap().name == name {
                return conv::integer(key as i32);
            }
        }
        // Create a new one
        let key = ed.create_buffer(&name);
        conv::integer(key as i32)
    })
}}

/// (buffer/get-by-name name) → slab-key or nil
///
/// Returns the slab key of the buffer named `name`, or nil if no such buffer
/// exists.  Does not create a buffer.
unsafe extern "C-unwind" fn c_buffer_get_by_name(argc: i32, argv: *mut Janet) -> Janet { unsafe {
    with_editor(|ed| {
        let name = conv::get_str(argc, argv, 0).unwrap_or_default();
        for (key, arc) in &ed.buffers {
            if arc.lock().unwrap().name == name {
                return conv::integer(key as i32);
            }
        }
        conv::nil()
    })
}}

/// (buffer/set-read-only buf bool)
///
/// Mark or unmark a buffer as read-only.  When true, `buffer/insert` and
/// `buffer/delete` are no-ops for that buffer.
unsafe extern "C-unwind" fn c_buffer_set_read_only(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let val = conv::get_bool(argc, argv, 1).unwrap_or(true);
        if let Some(arc) = ed.buffers.get_mut(key) {
            arc.lock().unwrap().read_only = val;
        }
        conv::nil()
    })
}

/// (buffer/read-only? buf) → bool
///
/// Return true if the buffer is marked read-only.
unsafe extern "C-unwind" fn c_buffer_read_only(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let ro = ed.buffers.get(key).map(|a| a.lock().unwrap().read_only).unwrap_or(false);
        conv::boolean(ro)
    })
}

/// (buffer/set-ephemeral buf bool)
///
/// Mark or unmark a buffer as ephemeral.  Ephemeral buffers never receive a
/// "save before closing?" prompt and are excluded from session restore.
unsafe extern "C-unwind" fn c_buffer_set_ephemeral(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let val = conv::get_bool(argc, argv, 1).unwrap_or(true);
        if let Some(arc) = ed.buffers.get_mut(key) {
            arc.lock().unwrap().ephemeral = val;
        }
        conv::nil()
    })
}

/// (buffer/ephemeral? buf) → bool
///
/// Return true if the buffer is marked ephemeral.
unsafe extern "C-unwind" fn c_buffer_ephemeral(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let e = ed.buffers.get(key).map(|a| a.lock().unwrap().ephemeral).unwrap_or(false);
        conv::boolean(e)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"buffer/find-or-create".as_ptr() as *const _,
            cfun: Some(c_buffer_find_or_create as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return or create the buffer with the given name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/get-by-name".as_ptr() as *const _,
            cfun: Some(c_buffer_get_by_name as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return slab key of named buffer or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/set-read-only".as_ptr() as *const _,
            cfun: Some(c_buffer_set_read_only as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set or clear the read-only flag on a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/read-only?".as_ptr() as *const _,
            cfun: Some(c_buffer_read_only as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return true if the buffer is read-only".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/set-ephemeral".as_ptr() as *const _,
            cfun: Some(c_buffer_set_ephemeral as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set or clear the ephemeral flag on a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/ephemeral?".as_ptr() as *const _,
            cfun: Some(c_buffer_ephemeral as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return true if the buffer is ephemeral".as_ptr() as *const _,
        },
    ]
}
