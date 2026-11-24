//! Janet API for the notification center — dismissable failure/warning
//! cards the frontend stacks as toasts, for failures a user must not
//! miss.  The transient `editor/echo` stays the tool for answers to
//! questions the user just asked.
//!
//! Registered C functions:
//!   (notify/post kind title &opt body) → id
//!   (notify/dismiss id)                → :ok or nil
//!   (notifications)                    → array of tables

use super::conv;
use super::with_editor;
use evil_janet::*;

/// `(notify/post kind title &opt body)` → id
///
/// `kind` is "info", "warn", or "error" — the card's tint.
unsafe extern "C-unwind" fn c_notify_post(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let kind = match conv::get_str(argc, argv, 0) {
            Some(k) => k,
            None => conv::signal_err("notify/post: kind string required"),
        };
        let title = match conv::get_str(argc, argv, 1) {
            Some(t) => t,
            None => conv::signal_err("notify/post: title string required"),
        };
        let body = conv::get_str(argc, argv, 2).unwrap_or_default();
        let id = ed.notifications.post(&kind, title, body);
        conv::integer(id as i32)
    })
}

/// `(notify/dismiss id)` → `:ok` or nil
unsafe extern "C-unwind" fn c_notify_dismiss(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let id = match unsafe { conv::get_int(argc, argv, 0) } {
            Some(i) if i >= 0 => i as u64,
            _ => return conv::nil(),
        };
        if ed.notifications.dismiss(id) {
            conv::keyword("ok")
        } else {
            conv::nil()
        }
    })
}

/// `(notifications)` → array of `{:id :kind :title :body}` tables
unsafe extern "C-unwind" fn c_notifications(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let items = ed.notifications.list();
        let arr = janet_wrap_array(janet_array(items.len() as i32));
        for n in items {
            let tbl = janet_wrap_table(janet_table(4));
            let t = janet_unwrap_table(tbl);
            janet_table_put(t, conv::keyword("id"), conv::integer(n.id as i32));
            janet_table_put(t, conv::keyword("kind"), conv::string(&n.kind));
            janet_table_put(t, conv::keyword("title"), conv::string(&n.title));
            janet_table_put(t, conv::keyword("body"), conv::string(&n.body));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"notify/post".as_ptr() as *const _,
            cfun: Some(c_notify_post as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Post a notification card: kind (info|warn|error), title, optional body; returns its id"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"notify/dismiss".as_ptr() as *const _,
            cfun: Some(c_notify_dismiss as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Dismiss a notification by id; :ok when it was live".as_ptr()
                as *const _,
        },
        JanetReg {
            name: c"notifications".as_ptr() as *const _,
            cfun: Some(c_notifications as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List live notifications as {:id :kind :title :body} tables".as_ptr()
                as *const _,
        },
    ]
}
