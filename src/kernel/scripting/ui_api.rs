//! Janet API for Sprint 11 UI customisation: surface access, modeline,
//! theme loading, tab-bar toggle, and per-buffer header lines.

use evil_janet::*;
use super::{conv, with_editor, SURFACE_PTR};

// ── Surface ───────────────────────────────────────────────────────────────────

/// (editor/surface-set-cell x y char face-name)
///
/// Write one character to the current render surface.  Only valid inside a
/// `render-frame` event handler.  Coordinates are clamped silently.
unsafe extern "C-unwind" fn c_surface_set_cell(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let x = unsafe { conv::get_int(argc, argv, 0) }.unwrap_or(0) as u16;
        let y = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0) as u16;
        let ch_str = unsafe { conv::get_str(argc, argv, 2) }.unwrap_or_else(|| " ".to_string());
        let ch = ch_str.chars().next().unwrap_or(' ');
        let face = unsafe { conv::get_str(argc, argv, 3) }.unwrap_or_default();
        let style = ed.resolve_face_style(&face);
        SURFACE_PTR.with(|cell| {
            if let Some(ptr) = cell.get() {
                let surf = unsafe { &mut *ptr };
                surf.set_cell(x, y, ch, style);
            }
        });
        conv::nil()
    })
}

/// (editor/surface-set-text x y text face-name)
///
/// Write a string of characters to the current render surface starting at (x, y).
/// Only valid inside a `render-frame` event handler.
unsafe extern "C-unwind" fn c_surface_set_text(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let x = unsafe { conv::get_int(argc, argv, 0) }.unwrap_or(0) as u16;
        let y = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0) as u16;
        let text = unsafe { conv::get_str(argc, argv, 2) }.unwrap_or_default();
        let face = unsafe { conv::get_str(argc, argv, 3) }.unwrap_or_default();
        let style = ed.resolve_face_style(&face);
        SURFACE_PTR.with(|cell| {
            if let Some(ptr) = cell.get() {
                let surf = unsafe { &mut *ptr };
                surf.set_text(x, y, &text, style);
            }
        });
        conv::nil()
    })
}

/// (editor/surface-size) → {:width n :height n}
///
/// Return the dimensions of the current render surface.
unsafe extern "C-unwind" fn c_surface_size(_argc: i32, _argv: *mut Janet) -> Janet {
    SURFACE_PTR.with(|cell| {
        if let Some(ptr) = cell.get() {
            unsafe {
                let surf = &*ptr;
                let tbl = janet_table(2);
                janet_table_put(tbl, conv::keyword("width"),  conv::integer(surf.width as i32));
                janet_table_put(tbl, conv::keyword("height"), conv::integer(surf.height as i32));
                janet_wrap_table(tbl)
            }
        } else {
            conv::nil()
        }
    })
}

// ── Modeline ─────────────────────────────────────────────────────────────────

/// (editor/set-modeline fn-name)
///
/// Set the name of a zero-argument Janet command whose string return value
/// replaces the built-in status bar format.  Pass `nil` to restore the
/// built-in format.
unsafe extern "C-unwind" fn c_set_modeline(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        if argc > 0 && unsafe { janet_checktype(*argv.add(0), JanetType_JANET_NIL) } != 0 {
            ed.modeline_fn = None;
        } else {
            ed.modeline_fn = unsafe { conv::get_str(argc, argv, 0) };
        }
        conv::nil()
    })
}

/// (editor/modeline) → fn-name or nil
///
/// Return the current modeline function name, or nil if using the built-in format.
unsafe extern "C-unwind" fn c_get_modeline(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        match &ed.modeline_fn {
            Some(name) => conv::string(name),
            None => conv::nil(),
        }
    })
}

// ── Theme ─────────────────────────────────────────────────────────────────────

/// (editor/load-theme path)
///
/// Evaluate a Janet theme file that calls `editor/define-face` and
/// `editor/set-theme`.  The file path is resolved as-is.
unsafe extern "C-unwind" fn c_load_theme(argc: i32, argv: *mut Janet) -> Janet {
    let Some(path) = (unsafe { conv::get_str(argc, argv, 0) }) else {
        conv::signal_err("editor/load-theme requires a path")
    };
    match std::fs::read_to_string(&path) {
        Ok(source) => {
            let c_name = match std::ffi::CString::new(path.as_str()) {
                Ok(s) => s,
                Err(_) => conv::signal_err("editor/load-theme: invalid path"),
            };
            let c_source = match std::ffi::CString::new(source.as_str()) {
                Ok(s) => s,
                Err(_) => conv::signal_err("editor/load-theme: source contains null byte"),
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
        Err(e) => conv::signal_err(&format!("editor/load-theme: {e}")),
    }
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

/// (editor/set-tab-bar bool)
///
/// When true, `render_frame` reserves the top row for a tab bar drawn by
/// a `render-frame` event handler.
unsafe extern "C-unwind" fn c_set_tab_bar(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        ed.tab_bar_enabled = unsafe { conv::get_bool(argc, argv, 0) }.unwrap_or(false);
        conv::nil()
    })
}

/// (editor/tab-bar-enabled) → bool
///
/// Return whether the tab bar row reservation is active.
unsafe extern "C-unwind" fn c_tab_bar_enabled(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| conv::boolean(ed.tab_bar_enabled))
}

// ── Buffer header line ────────────────────────────────────────────────────────

/// (buffer/set-header-line buf text)
///
/// Set the pre-rendered text drawn above the buffer content area.  Pass `nil`
/// to clear the header line.
unsafe extern "C-unwind" fn c_buffer_set_header_line(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(buf_key) = (unsafe { conv::get_int(argc, argv, 0) }) else {
            conv::signal_err("buffer/set-header-line requires a buffer key")
        };
        if let Some(buf) = ed.buffers.get_mut(buf_key as usize) {
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
        if let Some(buf) = ed.buffers.get(buf_key as usize) {
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
            name: c"surface/set-cell".as_ptr() as *const _,
            cfun: Some(c_surface_set_cell),
            documentation: c"Write one character to the render surface".as_ptr() as *const _,
        },
        JanetReg {
            name: c"surface/set-text".as_ptr() as *const _,
            cfun: Some(c_surface_set_text),
            documentation: c"Write a string to the render surface".as_ptr() as *const _,
        },
        JanetReg {
            name: c"surface/size".as_ptr() as *const _,
            cfun: Some(c_surface_size),
            documentation: c"Return {:width n :height n} of the current surface".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ui/set-modeline".as_ptr() as *const _,
            cfun: Some(c_set_modeline),
            documentation: c"Set the Janet command used to render the status bar".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ui/modeline".as_ptr() as *const _,
            cfun: Some(c_get_modeline),
            documentation: c"Return the current modeline function name or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ui/load-theme".as_ptr() as *const _,
            cfun: Some(c_load_theme),
            documentation: c"Evaluate a Janet theme file".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ui/set-tab-bar".as_ptr() as *const _,
            cfun: Some(c_set_tab_bar),
            documentation: c"Enable or disable the tab bar row reservation".as_ptr() as *const _,
        },
        JanetReg {
            name: c"ui/tab-bar-enabled".as_ptr() as *const _,
            cfun: Some(c_tab_bar_enabled),
            documentation: c"Return whether the tab bar is enabled".as_ptr() as *const _,
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
