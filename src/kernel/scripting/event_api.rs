//! Janet API functions for the event system — registered as `extern "C"` via evil-janet.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use evil_janet::*;
use super::conv;
use super::with_editor;

/// Send+Sync wrapper for `Janet` (which contains raw pointers in its union).
#[derive(Clone, Copy)]
struct JanetSend(Janet);
unsafe impl Send for JanetSend {}
unsafe impl Sync for JanetSend {}

/// Global storage for Janet event handler functions, guarded by a Mutex so
/// the Rust event-bus closures (which are Send+Sync) can access them.
/// Each handler is GC-rooted so the collector does not reclaim it.
static JANET_HANDLERS: LazyLock<Mutex<Vec<Option<JanetSend>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Register a Janet function as an event handler, returning its index.
fn register_handler(fn_val: Janet) -> usize {
    unsafe {
        janet_gcroot(fn_val);
    }
    let mut handlers = JANET_HANDLERS.lock().unwrap();
    let idx = handlers.len();
    handlers.push(Some(JanetSend(fn_val)));
    idx
}

/// Call a Janet function with one argument outside any active fiber.
///
/// `janet_call` requires an existing fiber context; this function creates a
/// fresh fiber with `janet_fiber` so it works from plain Rust call sites.
unsafe fn call_janet_fn_with_arg(fn_val: Janet, arg: Janet) { unsafe {
    let func = janet_unwrap_function(fn_val);
    let args = [arg];
    let fiber = janet_fiber(func, 64, 1, args.as_ptr());
    if fiber.is_null() {
        return;
    }
    let mut out: Janet = std::mem::zeroed();
    let sig = janet_continue(fiber, janet_wrap_nil(), &mut out);
    if sig != JanetSignal_JANET_SIGNAL_OK {
        debug!("EV: janet handler signal={}", sig);
    }
}}

/// Build a Janet table from a `HashMap<String, String>`.
unsafe fn data_to_table(data: &HashMap<String, String>) -> *mut JanetTable { unsafe {
    let tbl = janet_table(data.len() as i32);
    for (key, value) in data {
        let k = conv::keyword(key);
        let v = conv::string(value);
        janet_table_put(tbl, k, v);
    }
    tbl
}}

/// (event/on name handler-fn) → subscription-id
unsafe extern "C-unwind" fn c_event_on(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(n) => n,
            None => conv::signal_err("event/on requires event-name and handler-fn"),
        };
        if argc < 2 {
            conv::signal_err("event/on requires a handler function");
        }
        let fn_val = *argv.add(1);
        let fn_idx = register_handler(fn_val);
        let ev_name = name.clone();
        let sub_id = ed.events.on(&name, move |data: &HashMap<String, String>| {
            if let Ok(handlers) = JANET_HANDLERS.lock()
                && let Some(Some(JanetSend(fn_ref))) = handlers.get(fn_idx) {
                    let tbl = data_to_table(data);
                    let arg = janet_wrap_table(tbl);
                    debug!("EV: calling handler for event '{}'", ev_name);
                    call_janet_fn_with_arg(*fn_ref, arg);
                    debug!("EV: handler returned for event '{}'", ev_name);
                }
            None
        });
        conv::integer(sub_id as i32)
    })
}

/// (event/once name handler-fn) → subscription-id
unsafe extern "C-unwind" fn c_event_once(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(n) => n,
            None => conv::signal_err("event/once requires event-name and handler-fn"),
        };
        if argc < 2 {
            conv::signal_err("event/once requires a handler function");
        }
        let fn_val = *argv.add(1);
        let fn_idx = register_handler(fn_val);
        let sub_id = ed.events.once(&name, move |data: &HashMap<String, String>| {
            if let Ok(handlers) = JANET_HANDLERS.lock()
                && let Some(Some(JanetSend(fn_ref))) = handlers.get(fn_idx) {
                    let tbl = data_to_table(data);
                    let arg = janet_wrap_table(tbl);
                    call_janet_fn_with_arg(*fn_ref, arg);
                }
            None
        });
        conv::integer(sub_id as i32)
    })
}

/// (event/off subscription-id)
unsafe extern "C-unwind" fn c_event_off(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(id) = conv::get_int(argc, argv, 0) else {
            return conv::nil();
        };
        ed.events.off(id as u64);
        let idx = id as usize;
        if let Ok(mut handlers) = JANET_HANDLERS.lock()
            && idx < handlers.len()
                && let Some(JanetSend(fn_val)) = handlers[idx].take() {
                    janet_gcunroot(fn_val);
                }
        conv::nil()
    })
}

/// (event/list-events) → [{:name s :count n} ...]
unsafe extern "C-unwind" fn c_event_list_events(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let events = ed.events.list_events();
        let arr = janet_wrap_array(janet_array(events.len() as i32));
        for (name, count) in &events {
            let tbl = janet_wrap_table(janet_table(2));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("name"), conv::string(name));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("count"), conv::integer(*count as i32));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

/// (event/list-subscribers event-name) → [sub-id ...]
unsafe extern "C-unwind" fn c_event_list_subscribers(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = conv::get_str(argc, argv, 0).unwrap_or_default();
        let ids = ed.events.list_subscribers(&name);
        let arr_ptr = janet_array(ids.len() as i32);
        for id in ids {
            janet_array_push(arr_ptr, conv::integer(id as i32));
        }
        janet_wrap_array(arr_ptr)
    })
}

unsafe fn janet_to_string(v: Janet) -> Option<String> { unsafe {
    if janet_checktype(v, JanetType_JANET_STRING) == 0
        && janet_checktype(v, JanetType_JANET_KEYWORD) == 0
    {
        return None;
    }
    let ptr = janet_unwrap_string(v);
    if ptr.is_null() { return None; }
    let cstr = std::ffi::CStr::from_ptr(ptr as *const i8);
    Some(cstr.to_string_lossy().into_owned())
}}

/// (event/emit name &opt data)
unsafe extern "C-unwind" fn c_event_emit(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(name) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        let mut data = HashMap::new();
        if argc >= 2 {
            let data_val = *argv.add(1);
            if janet_checktype(data_val, JanetType_JANET_TABLE) != 0 {
                let tbl = &*janet_unwrap_table(data_val);
                // Iterate over the full backing-array capacity, not just `count`.
                // Janet uses an open-addressing hash table so live entries can sit
                // at any position in [0, capacity); iterating only 0..count would
                // miss entries whose hash index is >= count.
                for i in 0..tbl.capacity {
                    let kv = &*tbl.data.add(i as usize);
                    // Skip empty slots (nil key) and dead/tombstone slots.
                    if janet_checktype(kv.key, JanetType_JANET_NIL) != 0 { continue; }
                    if let (Some(k), Some(v)) = (janet_to_string(kv.key), janet_to_string(kv.value)) {
                        data.insert(k, v);
                    }
                }
            }
        }
        ed.events.emit(&name, data);
        conv::nil()
    })
}

/// Drop all stored Janet handler values.
///
/// Called at the start of each `janet_bridge::init()` so that Janet GC values
/// from a previous session (potentially on a different OS thread with a
/// different `janet_vm` heap) are never dereferenced.  The `janet_gcroot`
/// calls that paired with those values were on the old thread's VM, so we
/// cannot and must not call `janet_gcunroot` here — we just discard the Vec.
pub fn reset_handlers() {
    if let Ok(mut handlers) = JANET_HANDLERS.lock() {
        handlers.clear();
    }
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"event/on".as_ptr() as *const _,
            cfun: Some(c_event_on as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Subscribe to an event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"event/once".as_ptr() as *const _,
            cfun: Some(c_event_once as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Subscribe to an event once".as_ptr() as *const _,
        },
        JanetReg {
            name: c"event/off".as_ptr() as *const _,
            cfun: Some(c_event_off as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Unsubscribe from an event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"event/list-events".as_ptr() as *const _,
            cfun: Some(c_event_list_events as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List events and subscriber counts".as_ptr() as *const _,
        },
        JanetReg {
            name: c"event/list-subscribers".as_ptr() as *const _,
            cfun: Some(c_event_list_subscribers as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List subscriber IDs for an event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"event/emit".as_ptr() as *const _,
            cfun: Some(c_event_emit as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Emit an event".as_ptr() as *const _,
        },
    ]
}
