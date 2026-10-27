//! Janet API for UI customisation: theme loading and per-buffer header lines.

use evil_janet::*;
use super::{conv, with_editor};

// ── Theme ─────────────────────────────────────────────────────────────────────

/// (ui/load-theme path)
///
/// Evaluate a Janet theme file that calls `face/define` and theme setup
/// functions.  The file path is resolved as-is.
unsafe extern "C-unwind" fn c_load_theme(argc: i32, argv: *mut Janet) -> Janet {
    let Some(path) = (unsafe { conv::get_str(argc, argv, 0) }) else {
        conv::signal_err("ui/load-theme requires a path")
    };
    match std::fs::read_to_string(&path) {
        Ok(source) => {
            let c_name = match std::ffi::CString::new(path.as_str()) {
                Ok(s) => s,
                Err(_) => conv::signal_err("ui/load-theme: invalid path"),
            };
            let c_source = match std::ffi::CString::new(source.as_str()) {
                Ok(s) => s,
                Err(_) => conv::signal_err("ui/load-theme: source contains null byte"),
            };
            unsafe {
                let mut result = std::mem::MaybeUninit::<Janet>::zeroed();
                janet_dostring(
                    janet_core_env(std::ptr::null_mut()),
                    c_source.as_ptr(),
                    c_name.as_ptr(),
                    result.as_mut_ptr(),
                );
            }
            conv::nil()
        }
        Err(e) => conv::signal_err(&format!("ui/load-theme: {e}")),
    }
}

// ── Buffer header line ────────────────────────────────────────────────────────

/// (buffer/set-header-line buf text)
///
/// Set the text drawn above the buffer content area.  Pass `nil`
/// to clear the header line.
unsafe extern "C-unwind" fn c_buffer_set_header_line(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = (unsafe { conv::get_int(argc, argv, 0) }) else {
            conv::signal_err("buffer/set-header-line requires a buffer key")
        };
        if let Some(arc) = ed.buffers.get_mut(buf_key as usize) {
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            if argc > 1 && unsafe { janet_checktype(*argv.add(1), JanetType_JANET_NIL) } != 0 {
                buf.header_line = None;
            } else {
                buf.header_line = unsafe { conv::get_str(argc, argv, 1) };
            }
        }
        conv::nil()
    })
}

/// (buffer/header-line buf) → text or nil
///
/// Return the current header line text for the buffer, or nil if none is set.
unsafe extern "C-unwind" fn c_buffer_header_line(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = (unsafe { conv::get_int(argc, argv, 0) }) else { return conv::nil(); };
        if let Some(arc) = ed.buffers.get(buf_key as usize) {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            match &buf.header_line {
                Some(text) => conv::string(text),
                None => conv::nil(),
            }
        } else {
            conv::nil()
        }
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"ui/load-theme".as_ptr() as *const _,
            cfun: Some(c_load_theme),
            documentation: c"Evaluate a Janet theme file".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/set-header-line".as_ptr() as *const _,
            cfun: Some(c_buffer_set_header_line),
            documentation: c"Set or clear the header line text for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/header-line".as_ptr() as *const _,
            cfun: Some(c_buffer_header_line),
            documentation: c"Return the header line text for a buffer, or nil".as_ptr() as *const _,
        },
    ]
}
