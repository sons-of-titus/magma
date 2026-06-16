//! Janet API for font configuration — family, size, fallbacks, glyph-width overrides.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::event::payload::*;
use crate::event::keys;

fn emit_font_changed(ed: &mut crate::state::Editor) {
    ed.font_changed = true;
    ed.events.emit_typed(keys::events::FONT_CHANGED, EmptyPayload);
}

/// (editor/set-font family size) → nil
unsafe extern "C-unwind" fn c_editor_set_font(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(family) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/set-font requires a family name")
        };
        let size = if argc > 1 {
            let v = *argv.add(1);
            if janet_checktype(v, JanetType_JANET_NUMBER) != 0 {
                janet_unwrap_number(v) as f32
            } else {
                ed.font_config.size
            }
        } else {
            ed.font_config.size
        };
        ed.font_config.family = family;
        ed.font_config.size = size.max(4.0);
        emit_font_changed(ed);
        conv::nil()
    })
}

/// (editor/font-size) → number
unsafe extern "C-unwind" fn c_editor_font_size(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        janet_wrap_number(ed.font_config.size as f64)
    })
}

/// (editor/set-font-size n) → nil
unsafe extern "C-unwind" fn c_editor_set_font_size(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            conv::signal_err("editor/set-font-size requires a number")
        }
        let v = *argv.add(0);
        if janet_checktype(v, JanetType_JANET_NUMBER) == 0 {
            conv::signal_err("editor/set-font-size requires a number")
        }
        ed.font_config.size = (janet_unwrap_number(v) as f32).max(4.0);
        emit_font_changed(ed);
        conv::nil()
    })
}

/// (editor/load-font path alias) → nil
unsafe extern "C-unwind" fn c_editor_load_font(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(path) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/load-font requires a path")
        };
        let Some(alias) = conv::get_str(argc, argv, 1) else {
            conv::signal_err("editor/load-font requires an alias")
        };
        match std::fs::read(&path) {
            Ok(bytes) => {
                ed.font_config.loaded_fonts.insert(alias, bytes);
                emit_font_changed(ed);
                conv::nil()
            }
            Err(e) => conv::signal_err(&format!("editor/load-font: cannot read {path}: {e}")),
        }
    })
}

/// (editor/set-font-fallback [alias …]) → nil
unsafe extern "C-unwind" fn c_editor_set_font_fallback(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            conv::signal_err("editor/set-font-fallback requires an array")
        }
        let v = *argv.add(0);
        let mut aliases: Vec<String> = Vec::new();
        if janet_checktype(v, JanetType_JANET_ARRAY) != 0 {
            let arr = &*janet_unwrap_array(v);
            for i in 0..arr.count as usize {
                let elem = arr.data.add(i).read();
                if janet_checktype(elem, JanetType_JANET_STRING) != 0
                    || janet_checktype(elem, JanetType_JANET_KEYWORD) != 0
                {
                    let ptr = janet_unwrap_string(elem);
                    if !ptr.is_null() {
                        let s = std::ffi::CStr::from_ptr(ptr as *const i8)
                            .to_string_lossy()
                            .into_owned();
                        aliases.push(s);
                    }
                }
            }
        } else if janet_checktype(v, JanetType_JANET_TUPLE) != 0 {
            let tup = janet_unwrap_tuple(v);
            let head = &*janet_tuple_head(tup);
            for i in 0..head.length as usize {
                let elem = tup.add(i).read();
                if janet_checktype(elem, JanetType_JANET_STRING) != 0
                    || janet_checktype(elem, JanetType_JANET_KEYWORD) != 0
                {
                    let ptr = janet_unwrap_string(elem);
                    if !ptr.is_null() {
                        let s = std::ffi::CStr::from_ptr(ptr as *const i8)
                            .to_string_lossy()
                            .into_owned();
                        aliases.push(s);
                    }
                }
            }
        } else {
            conv::signal_err("editor/set-font-fallback: argument must be an array or tuple")
        }
        ed.font_config.fallback = aliases;
        emit_font_changed(ed);
        conv::nil()
    })
}

/// (editor/set-font-context context {:family … :size …}) → nil
unsafe extern "C-unwind" fn c_editor_set_font_context(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(ctx_name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("editor/set-font-context requires a context name")
        };
        if argc < 2 {
            conv::signal_err("editor/set-font-context requires a config table")
        }
        let ds = *argv.add(1);
        let family = {
            let fv = janet_in(ds, conv::keyword("family"));
            if janet_checktype(fv, JanetType_JANET_STRING) != 0
                || janet_checktype(fv, JanetType_JANET_KEYWORD) != 0
            {
                let ptr = janet_unwrap_string(fv);
                if ptr.is_null() {
                    None
                } else {
                    Some(
                        std::ffi::CStr::from_ptr(ptr as *const i8)
                            .to_string_lossy()
                            .into_owned(),
                    )
                }
            } else {
                None
            }
        };
        let size = {
            let sv = janet_in(ds, conv::keyword("size"));
            if janet_checktype(sv, JanetType_JANET_NUMBER) != 0 {
                Some(janet_unwrap_number(sv) as f32)
            } else {
                None
            }
        };
        ed.font_config
            .context_overrides
            .insert(ctx_name, crate::state::ContextFont { family, size });
        emit_font_changed(ed);
        conv::nil()
    })
}

/// (editor/set-ligatures bool) → nil
unsafe extern "C-unwind" fn c_editor_set_ligatures(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(b) = conv::get_bool(argc, argv, 0) else {
            conv::signal_err("editor/set-ligatures requires a boolean")
        };
        ed.font_config.ligatures = b;
        emit_font_changed(ed);
        conv::nil()
    })
}

/// (editor/set-glyph-width char-or-codepoint width-in-cells) → nil
unsafe extern "C-unwind" fn c_editor_set_glyph_width(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 2 {
            conv::signal_err("editor/set-glyph-width requires char and width")
        }
        let v0 = *argv.add(0);
        let ch = if janet_checktype(v0, JanetType_JANET_NUMBER) != 0 {
            char::from_u32(janet_unwrap_number(v0) as u32).unwrap_or('\0')
        } else if janet_checktype(v0, JanetType_JANET_STRING) != 0
            || janet_checktype(v0, JanetType_JANET_KEYWORD) != 0
        {
            let ptr = janet_unwrap_string(v0);
            if ptr.is_null() {
                '\0'
            } else {
                std::ffi::CStr::from_ptr(ptr as *const i8)
                    .to_str()
                    .ok()
                    .and_then(|s| s.chars().next())
                    .unwrap_or('\0')
            }
        } else {
            conv::signal_err("editor/set-glyph-width: first arg must be char string or codepoint")
        };
        let v1 = *argv.add(1);
        if janet_checktype(v1, JanetType_JANET_NUMBER) == 0 {
            conv::signal_err("editor/set-glyph-width: second arg must be a width integer")
        }
        let w = (janet_unwrap_number(v1) as i32).max(0).min(255) as u8;
        if ch != '\0' {
            ed.font_config.glyph_widths.insert(ch, w);
        }
        conv::nil()
    })
}

/// (editor/set-glyph-width-range start end width) → nil
/// start and end are integer codepoints (inclusive).
unsafe extern "C-unwind" fn c_editor_set_glyph_width_range(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 3 {
            conv::signal_err("editor/set-glyph-width-range requires start end width")
        }
        let v0 = *argv.add(0);
        let v1 = *argv.add(1);
        let v2 = *argv.add(2);
        if janet_checktype(v0, JanetType_JANET_NUMBER) == 0
            || janet_checktype(v1, JanetType_JANET_NUMBER) == 0
            || janet_checktype(v2, JanetType_JANET_NUMBER) == 0
        {
            conv::signal_err("editor/set-glyph-width-range: all args must be integers")
        }
        let start = janet_unwrap_number(v0) as u32;
        let end   = janet_unwrap_number(v1) as u32;
        let w     = (janet_unwrap_number(v2) as i32).max(0).min(255) as u8;
        for cp in start..=end {
            if let Some(c) = char::from_u32(cp) {
                ed.font_config.glyph_widths.insert(c, w);
            }
        }
        conv::nil()
    })
}

const NERD_FONT_RANGES: &[(u32, u32)] = &[
    (0xE000, 0xF8FF), // Private Use Area (primary Nerd Font icons)
    (0x23FB, 0x23FE), // IEC Power Symbols
    (0x2665, 0x2665), // Black Heart Suit ♥
    (0x26A1, 0x26A1), // High Voltage Sign ⚡
];

/// (editor/set-nerd-font bool) → nil
/// When true, pre-populates glyph-width overrides for Nerd Font unicode ranges.
unsafe extern "C-unwind" fn c_editor_set_nerd_font(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(enabled) = conv::get_bool(argc, argv, 0) else {
            conv::signal_err("editor/set-nerd-font requires a boolean")
        };
        if enabled {
            for &(start, end) in NERD_FONT_RANGES {
                for cp in start..=end {
                    if let Some(c) = char::from_u32(cp) {
                        ed.font_config.glyph_widths.insert(c, 2);
                    }
                }
            }
        } else {
            for &(start, end) in NERD_FONT_RANGES {
                for cp in start..=end {
                    if let Some(c) = char::from_u32(cp) {
                        ed.font_config.glyph_widths.remove(&c);
                    }
                }
            }
        }
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"font/set".as_ptr() as *const _,
            cfun: Some(c_editor_set_font as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set primary font family and optional size".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/size".as_ptr() as *const _,
            cfun: Some(c_editor_font_size as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the current font size as a number".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/set-size".as_ptr() as *const _,
            cfun: Some(c_editor_set_font_size as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set font size; minimum 4".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/load".as_ptr() as *const _,
            cfun: Some(c_editor_load_font as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Load a .ttf/.otf font file from disk under an alias".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/set-fallback".as_ptr() as *const _,
            cfun: Some(c_editor_set_font_fallback as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the ordered fallback font alias list".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/set-context".as_ptr() as *const _,
            cfun: Some(c_editor_set_font_context as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set per-context font override (:prose, :code, :status, …)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/set-ligatures".as_ptr() as *const _,
            cfun: Some(c_editor_set_ligatures as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Enable or disable font ligatures".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/set-glyph-width".as_ptr() as *const _,
            cfun: Some(c_editor_set_glyph_width as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Override display-column width for one character or codepoint".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/set-glyph-width-range".as_ptr() as *const _,
            cfun: Some(c_editor_set_glyph_width_range as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Override display-column width for an inclusive codepoint range".as_ptr() as *const _,
        },
        JanetReg {
            name: c"font/set-nerd-font".as_ptr() as *const _,
            cfun: Some(c_editor_set_nerd_font as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Pre-populate Nerd Font glyph-width overrides (width=2)".as_ptr() as *const _,
        },
    ]
}
