//! Janet API — search: pattern, set-pattern, forward?, last-find.

use evil_janet::*;
use super::conv;
use super::with_editor;

unsafe extern "C-unwind" fn c_search_pattern(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| match &ed.search_pattern {
        Some(p) => conv::string(p),
        None => conv::nil(),
    })
}

unsafe extern "C-unwind" fn c_search_set_pattern(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(pattern) = conv::get_str(argc, argv, 0) else {
            conv::signal_err("search/set-pattern requires a pattern")
        };
        ed.search_pattern = Some(pattern);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_search_forward(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| conv::boolean(ed.search_forward))
}

unsafe extern "C-unwind" fn c_search_last_find(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        match ed.last_find {
            Some((ch, fwd, till)) => {
                let tbl = janet_table(3);
                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                janet_table_put(tbl, conv::keyword("char"), conv::string(s));
                janet_table_put(tbl, conv::keyword("forward"), conv::boolean(fwd));
                janet_table_put(tbl, conv::keyword("till"), conv::boolean(till));
                janet_wrap_table(tbl)
            }
            None => conv::nil(),
        }
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"search/pattern".as_ptr() as *const _,
            cfun: Some(c_search_pattern as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the current search pattern".as_ptr() as *const _,
        },
        JanetReg {
            name: c"search/set-pattern".as_ptr() as *const _,
            cfun: Some(c_search_set_pattern as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the current search pattern".as_ptr() as *const _,
        },
        JanetReg {
            name: c"search/forward?".as_ptr() as *const _,
            cfun: Some(c_search_forward as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return whether the last search was forward".as_ptr() as *const _,
        },
        JanetReg {
            name: c"search/last-find".as_ptr() as *const _,
            cfun: Some(c_search_last_find as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return last f/t search params or nil".as_ptr() as *const _,
        },
    ]
}
