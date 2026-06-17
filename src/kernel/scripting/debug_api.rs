//! Janet API for the Debug System (Phase 6).
//!
//! Registered C functions:
//!   (debug/start adapter)              → session-id
//!   (debug/continue &opt thread-id)    → nil
//!   (debug/step-in &opt thread-id)     → nil
//!   (debug/step-over &opt thread-id)   → nil
//!   (debug/step-out &opt thread-id)    → nil
//!   (debug/add-breakpoint file line)   → nil
//!   (debug/remove-breakpoint file line)→ nil
//!   (debug/evaluate expr)              → nil (result: debug-evaluate-result event)
//!   (debug/session)                    → session-id or nil
//!   (debug/breakpoints file)           → array of line numbers

use std::sync::{Arc, LazyLock, Mutex};
use std::collections::HashMap;

use tokio::io::AsyncWriteExt;
use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::debug::dap_client::{DapClient, dap_request};
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

type SessionId = crate::kernel::debug::types::SessionId;

struct DapHandle {
    stdin: Arc<tokio::sync::Mutex<tokio::process::ChildStdin>>,
    bg: crate::kernel::runtime::BackgroundHandle,
    next_seq: Arc<Mutex<i64>>,
}

static DAP_CLIENTS: LazyLock<Mutex<HashMap<SessionId, DapHandle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn reset_dap_clients() {
    if let Ok(mut guard) = DAP_CLIENTS.lock() {
        guard.clear();
    }
}

fn alloc_seq(handle: &DapHandle) -> i64 {
    let mut g = handle.next_seq.lock().unwrap_or_else(|e| e.into_inner());
    let s = *g;
    *g += 1;
    s
}

fn send_dap_command(session_id: SessionId, command: &str, arguments: &str) {
    let (stdin_arc, bg, body) = {
        let mut guard = DAP_CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
        let h = match guard.get_mut(&session_id) {
            Some(h) => h,
            None => return,
        };
        let seq = alloc_seq(h);
        let body = dap_request(seq, command, arguments);
        (h.stdin.clone(), h.bg.clone(), body)
    };
    bg.spawn(async move {
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut stdin = stdin_arc.lock().await;
        let _ = stdin.write_all(header.as_bytes()).await;
        let _ = stdin.write_all(body.as_bytes()).await;
        let _ = stdin.flush().await;
    });
}

// ── C functions ───────────────────────────────────────────────────────────────

/// (debug/start adapter) → session-id
unsafe extern "C-unwind" fn c_debug_start(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let adapter = unsafe { conv::get_str(argc, argv, 0) }
            .unwrap_or_else(|| "codelldb".to_string());
        let bg = match ed.background.clone() {
            Some(bg) => bg,
            None => conv::signal_err("debug/start: no background runtime"),
        };
        let session_id = ed.debug.new_session(adapter.clone());
        {
            let adapter_clone = adapter.clone();
            let bg_clone = bg.clone();
            bg.spawn(async move {
                match DapClient::start(&adapter_clone, session_id, bg_clone).await {
                    Ok(client) => {
                        let mut guard = DAP_CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
                        guard.insert(session_id, DapHandle {
                            stdin: client.stdin,
                            bg: client.bg,
                            next_seq: client.next_seq,
                        });
                    }
                    Err(e) => {
                        debug!("debug/start: adapter failed: {e}");
                    }
                }
            });
        }
        ed.events.emit_typed(keys::events::DEBUG_SESSION_STARTED, DebugSessionStartedPayload {
            session_id: session_id.to_string(),
            adapter,
        });
        conv::integer(session_id as i32)
    })
}

/// (debug/continue &opt thread-id) → nil
unsafe extern "C-unwind" fn c_debug_continue(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session { Some(s) => s, None => return conv::nil() };
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| ed.debug.active_session().and_then(|s| s.stopped_thread))
            .unwrap_or(1);
        send_dap_command(sid, "continue", &format!(r#"{{"threadId":{thread_id}}}"#));
        if let Some(s) = ed.debug.active_session_mut() { s.stopped = false; }
        conv::nil()
    })
}

/// (debug/step-in &opt thread-id) → nil
unsafe extern "C-unwind" fn c_debug_step_in(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session { Some(s) => s, None => return conv::nil() };
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| ed.debug.active_session().and_then(|s| s.stopped_thread))
            .unwrap_or(1);
        send_dap_command(sid, "stepIn", &format!(r#"{{"threadId":{thread_id}}}"#));
        conv::nil()
    })
}

/// (debug/step-over &opt thread-id) → nil
unsafe extern "C-unwind" fn c_debug_step_over(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session { Some(s) => s, None => return conv::nil() };
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| ed.debug.active_session().and_then(|s| s.stopped_thread))
            .unwrap_or(1);
        send_dap_command(sid, "next", &format!(r#"{{"threadId":{thread_id}}}"#));
        conv::nil()
    })
}

/// (debug/step-out &opt thread-id) → nil
unsafe extern "C-unwind" fn c_debug_step_out(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session { Some(s) => s, None => return conv::nil() };
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| ed.debug.active_session().and_then(|s| s.stopped_thread))
            .unwrap_or(1);
        send_dap_command(sid, "stepOut", &format!(r#"{{"threadId":{thread_id}}}"#));
        conv::nil()
    })
}

/// (debug/add-breakpoint file line) → nil
unsafe extern "C-unwind" fn c_debug_add_breakpoint(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let file = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        let line = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0) as usize;
        if file.is_empty() { return conv::nil(); }
        ed.debug.add_breakpoint(file.clone(), line);
        // The :breakpoints GutterProvider reads live from ed.debug.breakpoints —
        // no gutter state manipulation needed here.
        // Notify the active debug session.
        if let Some(sid) = ed.debug.active_session {
            let args = ed.debug.set_breakpoints_args(&file);
            send_dap_command(sid, "setBreakpoints", &args);
        }
        ed.events.emit_typed(keys::events::DEBUG_BREAKPOINT_CHANGED, DebugBreakpointChangedPayload {
            file,
            line: line.to_string(),
            action: "add".to_string(),
        });
        conv::nil()
    })
}

/// (debug/remove-breakpoint file line) → nil
unsafe extern "C-unwind" fn c_debug_remove_breakpoint(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let file = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        let line = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0) as usize;
        if file.is_empty() { return conv::nil(); }
        ed.debug.remove_breakpoint(&file, line);
        // The :breakpoints GutterProvider reads live from ed.debug.breakpoints —
        // no gutter state manipulation needed here.
        if let Some(sid) = ed.debug.active_session {
            let args = ed.debug.set_breakpoints_args(&file);
            send_dap_command(sid, "setBreakpoints", &args);
        }
        ed.events.emit_typed(keys::events::DEBUG_BREAKPOINT_CHANGED, DebugBreakpointChangedPayload {
            file,
            line: line.to_string(),
            action: "remove".to_string(),
        });
        conv::nil()
    })
}

/// (debug/evaluate expr &opt frame-id) → nil
unsafe extern "C-unwind" fn c_debug_evaluate(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let expr = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        let frame_id = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0);
        let sid = match ed.debug.active_session { Some(s) => s, None => return conv::nil() };
        let escaped = expr.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
        let args = format!(r#"{{"expression":"{escaped}","context":"repl","frameId":{frame_id}}}"#);
        send_dap_command(sid, "evaluate", &args);
        conv::nil()
    })
}

/// (debug/session) → session-id or nil
unsafe extern "C-unwind" fn c_debug_session(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| match ed.debug.active_session {
        Some(id) => conv::integer(id as i32),
        None     => conv::nil(),
    })
}

/// (debug/breakpoints file) → array of line numbers
unsafe extern "C-unwind" fn c_debug_breakpoints(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let file = conv::get_str(argc, argv, 0).unwrap_or_default();
        let bps = ed.debug.breakpoints_for(&file);
        let arr = janet_array(bps.len() as i32);
        for bp in bps {
            janet_array_push(arr, conv::integer(bp.line as i32));
        }
        janet_wrap_array(arr)
    })
}

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"debug/start".as_ptr() as *const _,
            cfun: Some(c_debug_start),
            documentation: c"(debug/start adapter) → session-id — spawn a debug adapter and initialize a DAP session".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/continue".as_ptr() as *const _,
            cfun: Some(c_debug_continue),
            documentation: c"(debug/continue &opt thread-id) → nil — resume execution in the active session".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/step-in".as_ptr() as *const _,
            cfun: Some(c_debug_step_in),
            documentation: c"(debug/step-in &opt thread-id) → nil — step into the next function call".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/step-over".as_ptr() as *const _,
            cfun: Some(c_debug_step_over),
            documentation: c"(debug/step-over &opt thread-id) → nil — step over the current line".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/step-out".as_ptr() as *const _,
            cfun: Some(c_debug_step_out),
            documentation: c"(debug/step-out &opt thread-id) → nil — step out of the current function".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/add-breakpoint".as_ptr() as *const _,
            cfun: Some(c_debug_add_breakpoint),
            documentation: c"(debug/add-breakpoint file line) → nil — add a breakpoint; updates gutter and notifies active session".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/remove-breakpoint".as_ptr() as *const _,
            cfun: Some(c_debug_remove_breakpoint),
            documentation: c"(debug/remove-breakpoint file line) → nil — remove a breakpoint; updates gutter and notifies active session".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/evaluate".as_ptr() as *const _,
            cfun: Some(c_debug_evaluate),
            documentation: c"(debug/evaluate expr &opt frame-id) → nil — evaluate in current debug context; result arrives via debug-evaluate-result event".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/session".as_ptr() as *const _,
            cfun: Some(c_debug_session),
            documentation: c"(debug/session) → session-id or nil — active debug session ID".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/breakpoints".as_ptr() as *const _,
            cfun: Some(c_debug_breakpoints),
            documentation: c"(debug/breakpoints file) → [line ...] — line numbers of all breakpoints in file".as_ptr() as *const _,
        },
    ]
}
