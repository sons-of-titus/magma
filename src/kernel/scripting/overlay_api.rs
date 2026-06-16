//! Janet API for overlay (floating window) management (Sprint 11).

use evil_janet::*;
use super::{conv, with_editor};
use crate::kernel::state::Overlay;

/// (editor/overlay-create x y w h buf-id) → overlay-id
///
/// Create a floating overlay at position (x, y) with dimensions (w, h).
/// `buf-id` is an optional buffer slab key whose content fills the overlay;
/// pass nil for an empty overlay.  Returns the overlay's integer ID.
unsafe extern "C-unwind" fn c_overlay_create(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let x = conv::get_int(argc, argv, 0).unwrap_or(0) as u16;
        let y = conv::get_int(argc, argv, 1).unwrap_or(0) as u16;
        let w = conv::get_int(argc, argv, 2).unwrap_or(20) as u16;
        let h = conv::get_int(argc, argv, 3).unwrap_or(10) as u16;
        let buf_id = if argc > 4 && janet_checktype(*argv.add(4), JanetType_JANET_NIL) == 0 {
            conv::get_int(argc, argv, 4).map(|k| k as usize)
        } else {
            None
        };
        let id = ed.next_overlay_id;
        ed.next_overlay_id += 1;
        ed.overlays.push(Overlay { id, x, y, width: w, height: h, buffer_id: buf_id, z_order: 0 });
        conv::integer(id as i32)
    })
}

/// (editor/overlay-destroy id)
///
/// Remove the overlay with the given ID.  No-op if the ID does not exist.
unsafe extern "C-unwind" fn c_overlay_destroy(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        ed.overlays.retain(|o| o.id != id);
        conv::nil()
    })
}

/// (editor/overlay-move id x y)
///
/// Move the overlay with the given ID to a new position.  No-op if the ID
/// does not exist.
unsafe extern "C-unwind" fn c_overlay_move(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let id = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let x  = conv::get_int(argc, argv, 1).unwrap_or(0) as u16;
        let y  = conv::get_int(argc, argv, 2).unwrap_or(0) as u16;
        if let Some(ov) = ed.overlays.iter_mut().find(|o| o.id == id) {
            ov.x = x;
            ov.y = y;
        }
        conv::nil()
    })
}

/// (editor/overlay-list) → [{:id n :x n :y n :width n :height n :z-order n} …]
///
/// Return all active overlays as an array of tables.
unsafe extern "C-unwind" fn c_overlay_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_array(ed.overlays.len() as i32);
        for ov in &ed.overlays {
            let tbl = janet_table(6);
            janet_table_put(tbl, conv::keyword("id"),      conv::integer(ov.id as i32));
            janet_table_put(tbl, conv::keyword("x"),       conv::integer(ov.x as i32));
            janet_table_put(tbl, conv::keyword("y"),       conv::integer(ov.y as i32));
            janet_table_put(tbl, conv::keyword("width"),   conv::integer(ov.width as i32));
            janet_table_put(tbl, conv::keyword("height"),  conv::integer(ov.height as i32));
            janet_table_put(tbl, conv::keyword("z-order"), conv::integer(ov.z_order));
            janet_array_push(arr, janet_wrap_table(tbl));
        }
        janet_wrap_array(arr)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"overlay/create".as_ptr() as *const _,
            cfun: Some(c_overlay_create),
            documentation: c"Create a floating overlay window".as_ptr() as *const _,
        },
        JanetReg {
            name: c"overlay/destroy".as_ptr() as *const _,
            cfun: Some(c_overlay_destroy),
            documentation: c"Remove an overlay by ID".as_ptr() as *const _,
        },
        JanetReg {
            name: c"overlay/move".as_ptr() as *const _,
            cfun: Some(c_overlay_move),
            documentation: c"Move an overlay to a new (x, y) position".as_ptr() as *const _,
        },
        JanetReg {
            name: c"overlay/list".as_ptr() as *const _,
            cfun: Some(c_overlay_list),
            documentation: c"Return all active overlays".as_ptr() as *const _,
        },
    ]
}
