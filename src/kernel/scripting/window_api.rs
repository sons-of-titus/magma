//! Janet API functions for window operations — registered as `extern "C"` via evil-janet.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::state::Editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

/// (window/current) → table|nil   (shows id, buffer slab key, position, size)
unsafe extern "C-unwind" fn c_window_current(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let wid = ed.view_tree.focused_window();
        let win = wid.and_then(|id| ed.view_tree.window(id));
        match win {
            Some(w) => {
                let tbl = janet_wrap_table(janet_table(0));
                janet_table_put(janet_unwrap_table(tbl), conv::keyword("id"), conv::integer(w.id.0 as i32));
                let buf_slab = w.buffer_id.map(|k| k as i32).unwrap_or(-1);
                janet_table_put(janet_unwrap_table(tbl), conv::keyword("buffer"), conv::integer(buf_slab));
                janet_table_put(janet_unwrap_table(tbl), conv::keyword("x"), conv::integer(w.x as i32));
                janet_table_put(janet_unwrap_table(tbl), conv::keyword("y"), conv::integer(w.y as i32));
                janet_table_put(janet_unwrap_table(tbl), conv::keyword("width"), conv::integer(w.width as i32));
                janet_table_put(janet_unwrap_table(tbl), conv::keyword("height"), conv::integer(w.height as i32));
                tbl
            }
            None => conv::nil(),
        }
    })
}

/// (window/list) → [table ...]
unsafe extern "C-unwind" fn c_window_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let wins = ed.view_tree.windows();
        let arr = janet_wrap_array(janet_array(wins.len() as i32));
        for w in wins {
            let tbl = janet_wrap_table(janet_table(0));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("id"), conv::integer(w.id.0 as i32));
            let buf_slab = w.buffer_id.map(|k| k as i32).unwrap_or(-1);
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("buffer"), conv::integer(buf_slab));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("x"), conv::integer(w.x as i32));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("y"), conv::integer(w.y as i32));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("width"), conv::integer(w.width as i32));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("height"), conv::integer(w.height as i32));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

/// (window/split &opt direction)  — direction: :horizontal (default) or :vertical
unsafe extern "C-unwind" fn c_window_split(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let dir_str = conv::get_str(argc, argv, 0);
        let focused = ed.view_tree.focused_window();
        if let Some(wid) = focused {
            match dir_str.as_deref() {
                Some("vertical") | Some(":vertical") => { ed.view_tree.split_vertical(wid); }
                _ => { ed.view_tree.split_horizontal(wid); }
            }
        }
        conv::nil()
    })
}

/// (window/focus id)
unsafe extern "C-unwind" fn c_window_focus(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        if let Some(id) = unsafe { conv::get_int(argc, argv, 0) } {
            let wid = crate::kernel::state::id::WindowId::from_u64(id as u64);
            ed.view_tree.focus(wid);
            ed.events.emit_typed(keys::events::WINDOW_FOCUSED, WindowFocusedPayload {
                id: id.to_string(),
                buffer: ed.view_tree.buffer(wid).map(|b| b.to_string()).unwrap_or_default(),
            });
        }
        conv::nil()
    })
}

/// (window/resize id w-frac &opt h-frac)
/// Set the horizontal weight [0.0–1.0] for window `id`; layout engine redistributes.
unsafe extern "C-unwind" fn c_window_resize(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0);
        let w_frac = if argc > 1 {
            let v = *argv.add(1);
            if janet_checktype(v, JanetType_JANET_NUMBER) != 0 {
                janet_unwrap_number(v) as f32
            } else { 0.5 }
        } else { 0.5 };
        if let Some(id) = id {
            let wid = crate::kernel::state::id::WindowId::from_u64(id as u64);
            ed.view_tree.resize_weighted(wid, w_frac);
        }
        conv::nil()
    })
}

/// (window/set-scroll-top &opt window-id top)
/// Pin the scroll offset for a window to an explicit line number.
unsafe extern "C-unwind" fn c_window_set_scroll_top(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0);
        let top = conv::get_int(argc, argv, 1).unwrap_or(0).max(0) as usize;
        let wid = id
            .map(|id| crate::kernel::state::id::WindowId::from_u64(id as u64))
            .or_else(|| ed.view_tree.focused_window());
        if let Some(wid) = wid {
            if let Some(win) = ed.view_tree.window_mut(wid) {
                win.scroll_offset = top;
                win.scroll_pinned = true;
            }
        }
        conv::nil()
    })
}

/// (window/unpin-scroll &opt window-id)
/// Release the pinned scroll offset so cursor-following resumes.
unsafe extern "C-unwind" fn c_window_unpin_scroll(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let id = conv::get_int(argc, argv, 0);
        let wid = id
            .map(|id| crate::kernel::state::id::WindowId::from_u64(id as u64))
            .or_else(|| ed.view_tree.focused_window());
        if let Some(wid) = wid {
            if let Some(win) = ed.view_tree.window_mut(wid) {
                win.scroll_pinned = false;
            }
        }
        conv::nil()
    })
}

/// (window/buffer id) → slab-key or nil
unsafe extern "C-unwind" fn c_window_buffer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let wid = conv::get_int(argc, argv, 0)
            .map(|id| crate::kernel::state::id::WindowId::from_u64(id as u64));
        match wid.and_then(|id| ed.view_tree.buffer(id)) {
            Some(k) => conv::integer(k as i32),
            None => conv::nil(),
        }
    })
}

/// (window/set-buffer window-id buffer-slab-key)
unsafe extern "C-unwind" fn c_window_set_buffer(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let wid = conv::get_int(argc, argv, 0)
            .map(|id| crate::kernel::state::id::WindowId::from_u64(id as u64));
        let buf_key = conv::get_int(argc, argv, 1).unwrap_or(-1) as usize;
        if let Some(wid) = wid {
            ed.view_tree.set_buffer(wid, buf_key);
        }
        conv::nil()
    })
}

/// (window/dimensions) → {:width w :height h}
/// Uses the focused window's dimensions as a proxy for terminal size.
unsafe extern "C-unwind" fn c_window_dimensions(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let (cols, rows) = ed.view_tree.focused_window()
            .and_then(|wid| ed.view_tree.window(wid))
            .map(|w| (w.width, w.height))
            .unwrap_or((80, 24));
        let tbl = janet_wrap_table(janet_table(2));
        janet_table_put(janet_unwrap_table(tbl), conv::keyword("width"), conv::integer(cols as i32));
        janet_table_put(janet_unwrap_table(tbl), conv::keyword("height"), conv::integer(rows as i32));
        tbl
    })
}

fn resolve_window(ed: &mut Editor, arg_id: Option<i32>) -> Option<(Option<usize>, u16, u16)> {
    let wid = arg_id
        .map(|id| crate::kernel::state::id::WindowId::from_u64(id as u64));
    let maybe = wid.and_then(|wid| ed.view_tree.window(wid).map(|w| (w.buffer_id, w.width, w.height)));
    if maybe.is_some() { return maybe; }
    ed.view_tree.focused_window()
        .and_then(|wid| ed.view_tree.window(wid))
        .map(|w| (w.buffer_id, w.width, w.height))
}

fn window_scroll_state(ed: &mut Editor, buf_id: usize, win_height: u16) -> crate::kernel::render::frame::ScrollState {
    let visible_lines = (win_height).saturating_sub(2) as usize;
    crate::kernel::render::frame::compute_scroll_state(ed, buf_id, visible_lines)
}

/// (window/width &opt window-id) → int
unsafe extern "C-unwind" fn c_window_width(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arg_id = conv::get_int(argc, argv, 0);
        match resolve_window(&mut *ed, arg_id) {
            Some((_buf, w, _h)) => conv::integer(w as i32),
            None => conv::nil(),
        }
    })
}

/// (window/height &opt window-id) → int
unsafe extern "C-unwind" fn c_window_height(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arg_id = conv::get_int(argc, argv, 0);
        match resolve_window(&mut *ed, arg_id) {
            Some((_buf, _w, h)) => conv::integer(h as i32),
            None => conv::nil(),
        }
    })
}

/// (window/cursor-row &opt window-id) → int
unsafe extern "C-unwind" fn c_window_cursor_row(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arg_id = conv::get_int(argc, argv, 0);
        match resolve_window(&mut *ed, arg_id) {
            Some((Some(bid), _w, h)) => {
                let st = window_scroll_state(&mut *ed, bid, h);
                conv::integer(st.cursor_vis_row as i32)
            }
            _ => conv::nil(),
        }
    })
}

/// (window/cursor-col &opt window-id) → int
unsafe extern "C-unwind" fn c_window_cursor_col(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arg_id = conv::get_int(argc, argv, 0);
        match resolve_window(&mut *ed, arg_id) {
            Some((Some(bid), _w, h)) => {
                let st = window_scroll_state(&mut *ed, bid, h);
                conv::integer(st.cursor_col as i32)
            }
            _ => conv::nil(),
        }
    })
}

/// (window/scroll-top &opt window-id) → int
/// Returns the pinned scroll offset when set, otherwise the cursor-following value.
unsafe extern "C-unwind" fn c_window_scroll_top(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arg_id = conv::get_int(argc, argv, 0);
        let wid = arg_id
            .map(|id| crate::kernel::state::id::WindowId::from_u64(id as u64))
            .or_else(|| ed.view_tree.focused_window());
        if let Some(wid) = wid {
            if let Some(win) = ed.view_tree.window(wid) {
                if win.scroll_pinned {
                    return conv::integer(win.scroll_offset as i32);
                }
            }
        }
        match resolve_window(&mut *ed, arg_id) {
            Some((Some(bid), _w, h)) => {
                let st = window_scroll_state(&mut *ed, bid, h);
                conv::integer(st.scroll_top as i32)
            }
            _ => conv::nil(),
        }
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"window/current".as_ptr() as *const _,
            cfun: Some(c_window_current as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get current window info".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/list".as_ptr() as *const _,
            cfun: Some(c_window_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List all windows".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/split".as_ptr() as *const _,
            cfun: Some(c_window_split as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Split the current window".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/focus".as_ptr() as *const _,
            cfun: Some(c_window_focus as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Focus a window by id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/buffer".as_ptr() as *const _,
            cfun: Some(c_window_buffer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get window's buffer slab key".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/set-buffer".as_ptr() as *const _,
            cfun: Some(c_window_set_buffer as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set window's buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/dimensions".as_ptr() as *const _,
            cfun: Some(c_window_dimensions as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get terminal dimensions".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/width".as_ptr() as *const _,
            cfun: Some(c_window_width as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get window width".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/height".as_ptr() as *const _,
            cfun: Some(c_window_height as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get window height".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/cursor-row".as_ptr() as *const _,
            cfun: Some(c_window_cursor_row as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get cursor visible row in window".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/cursor-col".as_ptr() as *const _,
            cfun: Some(c_window_cursor_col as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get cursor visible column in window".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/scroll-top".as_ptr() as *const _,
            cfun: Some(c_window_scroll_top as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get top visible line number in window".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/resize".as_ptr() as *const _,
            cfun: Some(c_window_resize as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Resize window by fractional weight".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/set-scroll-top".as_ptr() as *const _,
            cfun: Some(c_window_set_scroll_top as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Pin the scroll top for a window".as_ptr() as *const _,
        },
        JanetReg {
            name: c"window/unpin-scroll".as_ptr() as *const _,
            cfun: Some(c_window_unpin_scroll as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Release pinned scroll, resume cursor-following".as_ptr() as *const _,
        },
    ]
}
