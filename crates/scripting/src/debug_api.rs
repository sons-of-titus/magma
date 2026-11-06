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
//!   (debug/toggle-breakpoint)          → nil
//!   (debug/evaluate expr)              → nil (result: debug-evaluate-result event)
//!   (debug/session)                    → session-id or nil
//!   (debug/breakpoints file)           → array of line numbers

use serde_json::json;

use super::conv;
use super::with_editor;
use evil_janet::*;
use kernel::event::keys;
use kernel::event::payload::*;

type SessionId = kernel::debug::types::SessionId;

/// Send a DAP request through the session's foreground-owned connection.
pub(crate) fn send_dap_command(
    ed: &mut kernel::state::Editor,
    session_id: SessionId,
    command: &str,
    arguments: &str,
) {
    let Some(handle) = ed.debug.connections.get(&session_id).cloned() else {
        return;
    };
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or(json!({}));
    handle.request(command, args);
}

// ── C functions ───────────────────────────────────────────────────────────────

/// The buffer whose path matches `file`, if one is open.
fn buffer_for_path(
    ed: &mut kernel::state::Editor,
    file: &str,
) -> Option<std::sync::Arc<std::sync::Mutex<kernel::text_engine::Buffer>>> {
    ed.buffers
        .iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref() == Some(file))
        .map(|(_, a)| a.clone())
}

/// (debug/start adapter &opt launch-config-json) → session-id
/// The initialize handshake runs off the foreground; readiness arrives via
/// the `Dap(Started)` background event.
unsafe extern "C-unwind" fn c_debug_start(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let adapter =
            unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_else(|| "codelldb".to_string());
        let launch_config: Option<serde_json::Value> =
            unsafe { conv::get_str(argc, argv, 1) }.and_then(|c| serde_json::from_str(&c).ok());
        match kernel::debug::spawn_session(ed, &adapter, launch_config) {
            Ok(id) => conv::integer(id as i32),
            Err(e) => { logging::error!("debug_api.rs:64: {e}"); conv::nil() }
        }
    })
}

/// (debug/disconnect &opt session-id) → nil
/// Ordered teardown of the active (or named) session.
unsafe extern "C-unwind" fn c_debug_disconnect(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = unsafe { conv::get_int(argc, argv, 0) }
            .map(|v| v as u64)
            .or(ed.debug.active_session);
        if let Some(sid) = sid {
            ed.debug.disconnect(sid);
        }
        conv::nil()
    })
}

/// (debug/continue &opt thread-id) → nil
unsafe extern "C-unwind" fn c_debug_continue(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session {
            Some(s) => s,
            None => return conv::nil(),
        };
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| ed.debug.active_session().and_then(|s| s.stopped_thread))
            .unwrap_or(1);
        send_dap_command(
            ed,
            sid,
            "continue",
            &format!(r#"{{"threadId":{thread_id}}}"#),
        );
        if let Some(s) = ed.debug.active_session_mut() {
            s.stopped = false;
        }
        conv::nil()
    })
}

/// (debug/step-in &opt thread-id) → nil
unsafe extern "C-unwind" fn c_debug_step_in(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session {
            Some(s) => s,
            None => return conv::nil(),
        };
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| ed.debug.active_session().and_then(|s| s.stopped_thread))
            .unwrap_or(1);
        send_dap_command(ed, sid, "stepIn", &format!(r#"{{"threadId":{thread_id}}}"#));
        conv::nil()
    })
}

/// (debug/step-over &opt thread-id) → nil
unsafe extern "C-unwind" fn c_debug_step_over(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session {
            Some(s) => s,
            None => return conv::nil(),
        };
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| ed.debug.active_session().and_then(|s| s.stopped_thread))
            .unwrap_or(1);
        send_dap_command(ed, sid, "next", &format!(r#"{{"threadId":{thread_id}}}"#));
        conv::nil()
    })
}

/// (debug/step-out &opt thread-id) → nil
unsafe extern "C-unwind" fn c_debug_step_out(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session {
            Some(s) => s,
            None => return conv::nil(),
        };
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| ed.debug.active_session().and_then(|s| s.stopped_thread))
            .unwrap_or(1);
        send_dap_command(
            ed,
            sid,
            "stepOut",
            &format!(r#"{{"threadId":{thread_id}}}"#),
        );
        conv::nil()
    })
}

/// (debug/add-breakpoint file line) → nil
unsafe extern "C-unwind" fn c_debug_add_breakpoint(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let file = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        let line = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0) as usize;
        if file.is_empty() {
            return conv::nil();
        }
        let bp_id = ed.debug.add_breakpoint(file.clone(), line);
        // While a buffer for this file is open, its line tracks edits.
        if let Some(arc) = buffer_for_path(ed, &file) {
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let offset = buf.line_start_offset(line.saturating_sub(1)).unwrap_or(0);
            buf.attach_breakpoint(bp_id, offset);
        }
        // Notify the active debug session.
        if let Some(sid) = ed.debug.active_session {
            kernel::debug::send_set_breakpoints(ed, sid, &file);
        }
        ed.events.emit_typed(
            keys::events::DEBUG_BREAKPOINT_CHANGED,
            DebugBreakpointChangedPayload {
                file,
                line: line.to_string(),
                action: "add".to_string(),
            },
        );
        conv::nil()
    })
}

/// (debug/set-breakpoint-condition file line condition) → nil
/// An empty condition clears it.  Resyncs the active adapter session.
unsafe extern "C-unwind" fn c_debug_set_breakpoint_condition(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let file = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        let line = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0) as usize;
        let condition = unsafe { conv::get_str(argc, argv, 2) }.unwrap_or_default();
        if file.is_empty() {
            return conv::nil();
        }
        let condition = if condition.is_empty() {
            None
        } else {
            Some(condition.to_string())
        };
        kernel::debug::set_breakpoint_condition(ed, &file, line, condition);
        conv::nil()
    })
}

/// (debug/remove-breakpoint file line) → nil
unsafe extern "C-unwind" fn c_debug_remove_breakpoint(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let file = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        let line = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0) as usize;
        if file.is_empty() {
            return conv::nil();
        }
        let removed = ed.debug.remove_breakpoint(&file, line);
        if let Some(arc) = buffer_for_path(ed, &file) {
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            for id in removed {
                buf.detach_breakpoint(id);
            }
        }
        // The :breakpoints GutterProvider reads live from ed.debug.breakpoints.
        if let Some(sid) = ed.debug.active_session {
            kernel::debug::send_set_breakpoints(ed, sid, &file);
        }
        ed.events.emit_typed(
            keys::events::DEBUG_BREAKPOINT_CHANGED,
            DebugBreakpointChangedPayload {
                file,
                line: line.to_string(),
                action: "remove".to_string(),
            },
        );
        conv::nil()
    })
}

/// (debug/toggle-breakpoint) → nil
///
/// Toggle a 1-based breakpoint at the cursor line of the focused buffer.
/// Routes through the kernel's native toggle so keybindings and gutter
/// clicks share the past-EOF anchoring rule.
unsafe extern "C-unwind" fn c_debug_toggle_breakpoint(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let buf_id = kernel::input::focused_buffer_id(ed);
        let cursor = ed
            .view_of_buffer(buf_id)
            .map(|v| v.cursor_offset())
            .unwrap_or(0);
        let target = ed.buffers.get(buf_id).and_then(|arc| {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let path = buf.path.clone()?;
            let line = buf.slice(0, cursor).chars().filter(|&c| c == '\n').count() + 1;
            Some((path, line))
        });
        if let Some((path, line)) = target {
            kernel::debug::toggle_breakpoint_at(ed, &path, line);
        }
        conv::nil()
    })
}

/// (debug/set-enabled file line enabled) → nil
/// Disabled breakpoints stay visible in the UI but are withheld from the
/// adapter until re-enabled.
unsafe extern "C-unwind" fn c_debug_set_enabled(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let file = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        let line = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0) as usize;
        let enabled = unsafe { conv::get_bool(argc, argv, 2) }.unwrap_or(true);
        if file.is_empty() {
            return conv::nil();
        }
        kernel::debug::set_breakpoint_enabled(ed, &file, line, enabled);
        conv::nil()
    })
}

/// (debug/pause &opt thread-id) → nil
/// Without a thread id the last-stopped (or first known) thread is paused —
/// a hardcoded id would silently miss on adapters that number threads
/// differently.
unsafe extern "C-unwind" fn c_debug_pause(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session {
            Some(s) => s,
            None => return conv::nil(),
        };
        let session = ed.debug.active_session();
        let thread_id = unsafe { conv::get_int(argc, argv, 0) }
            .map(|t| t as u64)
            .or_else(|| session.and_then(|s| s.stopped_thread))
            .or_else(|| session.and_then(|s| s.threads.first().map(|t| t.id)));
        let Some(thread_id) = thread_id else {
            return conv::nil();
        };
        send_dap_command(ed, sid, "pause", &format!(r#"{{"threadId":{thread_id}}}"#));
        conv::nil()
    })
}

/// (debug/restart) → nil
/// Asks the adapter to restart the debuggee; unsupported adapters report the
/// failure into the debug console.
unsafe extern "C-unwind" fn c_debug_restart(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let sid = match ed.debug.active_session {
            Some(s) => s,
            None => return conv::nil(),
        };
        if let Some(s) = ed.debug.active_session_mut() {
            s.stopped = false;
        }
        send_dap_command(ed, sid, "restart", "{}");
        conv::nil()
    })
}

/// (debug/watch-add expr) → nil
unsafe extern "C-unwind" fn c_debug_watch_add(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(expr) = (unsafe { conv::get_str(argc, argv, 0) }) else {
            return conv::nil();
        };
        let expr = expr.trim().to_string();
        if expr.is_empty() {
            return conv::nil();
        }
        if !ed.debug.watches.contains(&expr) {
            ed.debug.watches.push(expr);
        }
        conv::nil()
    })
}

/// (debug/watch-remove expr) → nil
unsafe extern "C-unwind" fn c_debug_watch_remove(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(expr) = (unsafe { conv::get_str(argc, argv, 0) }) else {
            return conv::nil();
        };
        ed.debug.watches.retain(|w| w != &expr);
        for session in ed.debug.sessions.values_mut() {
            session.watch_results = vec![None; ed.debug.watches.len()];
        }
        conv::nil()
    })
}

/// (debug/watches) → [expr …]
unsafe extern "C-unwind" fn c_debug_watches(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_array(ed.debug.watches.len() as i32);
        for w in &ed.debug.watches {
            janet_array_push(arr, conv::string(w));
        }
        janet_wrap_array(arr)
    })
}

/// (debug/evaluate expr &opt frame-id) → nil
unsafe extern "C-unwind" fn c_debug_evaluate(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let expr = unsafe { conv::get_str(argc, argv, 0) }.unwrap_or_default();
        let frame_id = unsafe { conv::get_int(argc, argv, 1) }.unwrap_or(0);
        let sid = match ed.debug.active_session {
            Some(s) => s,
            None => return conv::nil(),
        };
        let escaped = expr
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        let args = format!(r#"{{"expression":"{escaped}","context":"repl","frameId":{frame_id}}}"#);
        send_dap_command(ed, sid, "evaluate", &args);
        conv::nil()
    })
}

/// (debug/session) → session-id or nil
unsafe extern "C-unwind" fn c_debug_session(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| match ed.debug.active_session {
        Some(id) => conv::integer(id as i32),
        None => conv::nil(),
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
            name: c"debug/disconnect".as_ptr() as *const _,
            cfun: Some(c_debug_disconnect),
            documentation: c"(debug/disconnect &opt session-id) → nil — ordered disconnect + terminate of a debug session".as_ptr() as *const _,
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
            name: c"debug/set-breakpoint-condition".as_ptr() as *const _,
            cfun: Some(c_debug_set_breakpoint_condition as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set or clear (empty string) the hit condition on breakpoints at a file/line".as_ptr() as *const _,
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
        JanetReg {
            name: c"debug/toggle-breakpoint".as_ptr() as *const _,
            cfun: Some(c_debug_toggle_breakpoint),
            documentation: c"(debug/toggle-breakpoint) → nil — toggle a breakpoint at the cursor line of the focused buffer (1-based)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/set-enabled".as_ptr() as *const _,
            cfun: Some(c_debug_set_enabled),
            documentation: c"(debug/set-enabled file line enabled) → nil — enable or disable breakpoints at a location; disabled ones are withheld from the adapter".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/pause".as_ptr() as *const _,
            cfun: Some(c_debug_pause),
            documentation: c"(debug/pause &opt thread-id) → nil — pause execution in the active session".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/restart".as_ptr() as *const _,
            cfun: Some(c_debug_restart),
            documentation: c"(debug/restart) → nil — ask the adapter to restart the debuggee".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/watch-add".as_ptr() as *const _,
            cfun: Some(c_debug_watch_add),
            documentation: c"(debug/watch-add expr) → nil — add a watch expression, evaluated whenever the session stops".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/watch-remove".as_ptr() as *const _,
            cfun: Some(c_debug_watch_remove),
            documentation: c"(debug/watch-remove expr) → nil — remove a watch expression".as_ptr() as *const _,
        },
        JanetReg {
            name: c"debug/watches".as_ptr() as *const _,
            cfun: Some(c_debug_watches),
            documentation: c"(debug/watches) → [expr …] — current watch expressions".as_ptr() as *const _,
        },
    ]
}
