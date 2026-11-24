//! Janet API for icon packs — install glyph packs, per-extension
//! overrides, and VS Code icon-theme loading.

use super::conv;
use super::with_editor;
use evil_janet::*;
use kernel::icons::{IconKind, IconPack};
use std::collections::HashMap;

/// Extract one icon spec from a Janet value: a codepoint integer, or a
/// `[codepoint [r g b]]` tuple for a colored glyph.
unsafe fn icon_from_val(v: Janet) -> Option<IconKind> {
    unsafe {
        if janet_checktype(v, JanetType_JANET_NUMBER) != 0 {
            let cp = janet_unwrap_number(v) as u32;
            char::from_u32(cp).map(IconKind::glyph)
        } else if janet_checktype(v, JanetType_JANET_TUPLE) != 0
            || janet_checktype(v, JanetType_JANET_ARRAY) != 0
        {
            let cp = janet_in(v, conv::integer(0));
            let rgb = janet_in(v, conv::integer(1));
            if janet_checktype(cp, JanetType_JANET_NUMBER) == 0 {
                return None;
            }
            let glyph = char::from_u32(janet_unwrap_number(cp) as u32)?;
            let color = if janet_checktype(rgb, JanetType_JANET_TUPLE) != 0
                || janet_checktype(rgb, JanetType_JANET_ARRAY) != 0
            {
                let mut c = [0u8; 3];
                for (i, slot) in c.iter_mut().enumerate() {
                    let part = janet_in(rgb, conv::integer(i as i32));
                    if janet_checktype(part, JanetType_JANET_NUMBER) != 0 {
                        *slot = janet_unwrap_number(part).clamp(0.0, 255.0) as u8;
                    }
                }
                Some((c[0], c[1], c[2]))
            } else {
                None
            };
            Some(IconKind::Glyph { glyph, color })
        } else {
            None
        }
    }
}

/// Copy a Janet table of `{key icon-spec}` into a Rust map.  Keys may be
/// strings or keywords; extension keys lose leading dots.
unsafe fn string_of(v: Janet) -> Option<String> {
    unsafe {
        if janet_checktype(v, JanetType_JANET_STRING) == 0
            && janet_checktype(v, JanetType_JANET_KEYWORD) == 0
        {
            return None;
        }
        let ptr = janet_unwrap_string(v);
        if ptr.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(ptr as *const std::ffi::c_char)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

unsafe fn copy_map(ds: Janet, out: &mut HashMap<String, IconKind>, strip_dots: bool) {
    unsafe {
        if janet_checktype(ds, JanetType_JANET_TABLE) == 0 {
            return;
        }
        let tbl = &*janet_unwrap_table(ds);
        for i in 0..tbl.capacity {
            let kv = &*tbl.data.add(i as usize);
            if janet_checktype(kv.key, JanetType_JANET_NIL) != 0 {
                continue;
            }
            let Some(key) = string_of(kv.key) else {
                continue;
            };
            let Some(icon) = icon_from_val(kv.value) else {
                continue;
            };
            let key = if strip_dots {
                key.trim_start_matches('.').to_lowercase()
            } else {
                key
            };
            out.insert(key, icon);
        }
    }
}

unsafe fn single_icon(ds: Janet, key: &str) -> Option<IconKind> {
    unsafe {
        let v = janet_in(ds, conv::keyword(key));
        if janet_checktype(v, JanetType_JANET_NIL) != 0 {
            None
        } else {
            icon_from_val(v)
        }
    }
}

/// (icons/set-pack name spec) → nil
///
/// `spec` is a table: `{:files {".rs" cp …} :file-names {"Makefile" cp …}
/// :default-file cp :folder cp :folder-open cp}` where a value is a
/// codepoint or `[codepoint [r g b]]`.  The pack named "default" is
/// snapshotted so the `icon-pack` option can restore it.
unsafe extern "C-unwind" fn c_set_pack(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("icons/set-pack requires a pack name")
        };
        if argc < 2 || janet_checktype(*argv.add(1), JanetType_JANET_TABLE) == 0 {
            conv::signal_err("icons/set-pack requires a spec table")
        }
        let spec = *argv.add(1);
        let mut pack = IconPack::empty(&name);
        copy_map(
            janet_in(spec, conv::keyword("files")),
            &mut pack.extensions,
            true,
        );
        copy_map(
            janet_in(spec, conv::keyword("file-names")),
            &mut pack.file_names,
            false,
        );
        pack.default_file = single_icon(spec, "default-file");
        pack.folder = single_icon(spec, "folder");
        pack.folder_open = single_icon(spec, "folder-open");
        if name == "default" {
            ed.default_icons = pack.clone();
        }
        ed.icons = pack;
        conv::nil()
    })
}

/// (icons/lookup filename) → string or nil
unsafe extern "C-unwind" fn c_lookup(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("icons/lookup requires a file name")
        };
        match ed.icons.lookup(&name) {
            Some(IconKind::Glyph { glyph, .. }) => conv::string(&glyph.to_string()),
            _ => conv::nil(),
        }
    })
}

/// (icons/current) → string
unsafe extern "C-unwind" fn c_current(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| conv::string(&ed.icons.name))
}

/// (icons/load-pack name-or-path) → name
///
/// Loads a VS Code icon-theme pack (a manifest file or a directory holding
/// one) and records it in the `icon-pack` option so it persists.
unsafe extern "C-unwind" fn c_load_pack(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(target) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("icons/load-pack requires a pack name or path")
        };
        match kernel::icons::find_and_load(std::path::Path::new(&target)) {
            Ok(pack) => {
                let name = pack.name.clone();
                ed.icons = pack;
                ed.options.insert("icon-pack".to_string(), name.clone());
                conv::string(&name)
            }
            Err(e) => conv::signal_err(&format!("icons/load-pack: {e}")),
        }
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"icons/set-pack".as_ptr() as *const _,
            cfun: Some(c_set_pack),
            documentation: c"(icons/set-pack name spec) → nil — install a glyph icon pack from a spec table {:files {:ext cp} :file-names {:name cp} :default-file cp :folder cp :folder-open cp}; values are codepoints or [codepoint [r g b]]".as_ptr() as *const _,
        },
        JanetReg {
            name: c"icons/lookup".as_ptr() as *const _,
            cfun: Some(c_lookup),
            documentation: c"(icons/lookup filename) → string or nil — the active pack's glyph for a file name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"icons/current".as_ptr() as *const _,
            cfun: Some(c_current),
            documentation: c"(icons/current) → string — the name of the active icon pack".as_ptr() as *const _,
        },
        JanetReg {
            name: c"icons/load-pack".as_ptr() as *const _,
            cfun: Some(c_load_pack),
            documentation: c"(icons/load-pack name-or-path) → name — load a VS Code icon-theme pack by path or name in an icon-pack search dir".as_ptr() as *const _,
        },
    ]
}
