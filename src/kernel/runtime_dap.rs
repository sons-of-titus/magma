//! DAP command-result handling, split from the `runtime.rs` event router.
//!
//! A stopped debuggee with an empty stack is a facade: this chain populates
//! the session model (stackTrace → scopes → variables), evaluates watch
//! expressions, jumps the editor to the current execution line, and routes
//! console output into the session's console.

use crate::kernel::state::Editor;
use crate::kernel::state::MessageKind;

/// Open `path` in a buffer and place the cursor at the 1-based `line`.
pub(crate) fn open_source_line(ed: &mut Editor, path: &str, line: usize) {
    let mut args = std::collections::HashMap::new();
    args.insert("path".to_string(),
        crate::kernel::command::args::ArgValue::Path(path.to_string()));
    if crate::kernel::command::execute_command(ed, "open-file", &args).is_err() {
        return;
    }
    let Some((key, arc)) = ed.buffers.iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref() == Some(path))
        .map(|(k, a)| (k, a.clone())) else { return };
    let offset = arc.lock().unwrap_or_else(|e| e.into_inner())
        .line_start_offset(line.saturating_sub(1))
        .unwrap_or(0);
    if let Some(view) = ed.views.get_mut(&key) {
        view.set_cursor(offset);
    }
}

/// Ask the adapter for the children of a variables reference.  The response
/// is correlated back through the session's `pending_variables`.
pub fn request_variables(ed: &mut Editor, session_id: u64, r: i64) {
    if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
        session.pending_variables = Some(r);
    }
    #[cfg(feature = "janet")]
    crate::kernel::scripting::send_dap_command(
        ed, session_id, "variables", &format!(r#"{{"variablesReference":{r}}}"#));
}

/// Send `evaluate` for every watch expression against the current top frame.
fn evaluate_watches(ed: &mut Editor, session_id: u64, frame_id: u64) {
    let watches = ed.debug.watches.clone();
    let mut args_list: Vec<String> = Vec::new();
    {
        let Some(session) = ed.debug.sessions.get_mut(&session_id) else { return };
        session.watch_results = vec![None; watches.len()];
        session.pending_watch_evals.clear();
        for (i, expr) in watches.iter().enumerate() {
            session.pending_watch_evals.push_back(i);
            let escaped = expr.replace('\\', "\\\\").replace('"', "\\\"");
            args_list.push(format!(
                r#"{{"expression":"{escaped}","context":"watch","frameId":{frame_id}}}"#));
        }
    }
    for args in &args_list {
        #[cfg(feature = "janet")]
        crate::kernel::scripting::send_dap_command(ed, session_id, "evaluate", args);
    }
}

pub(crate) fn apply_dap_command_result(
    ed: &mut Editor,
    session_id: u64,
    command: &str,
    success: bool,
    body: &str,
) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else { return };
    match (command, success) {
        ("stackTrace", true) => {
            let frames: Vec<crate::kernel::debug::types::StackFrame> = v["stackFrames"].as_array()
                .map(|arr| arr.iter().map(|f| crate::kernel::debug::types::StackFrame {
                    id: f["id"].as_u64().unwrap_or(0),
                    name: f["name"].as_str().unwrap_or("?").to_string(),
                    source: f["source"]["path"].as_str().map(|s| s.to_string()),
                    line: f["line"].as_u64().unwrap_or(0) as usize,
                    column: f["column"].as_u64().unwrap_or(0) as usize,
                }).collect())
                .unwrap_or_default();
            let first = frames.first().cloned();
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.set_frames(frames);
                session.stopped_line = first.as_ref()
                    .and_then(|f| f.source.clone())
                    .map(|path| (path, f_line(&first)));
            }
            if let Some(frame_id) = first.as_ref().map(|f| f.id) {
                // The current execution location opens in the editor.
                if let Some((path, line)) = ed.debug.sessions.get(&session_id)
                    .and_then(|s| s.stopped_line.clone()) {
                    open_source_line(ed, &path, line);
                }
                #[cfg(feature = "janet")]
                crate::kernel::scripting::send_dap_command(
                    ed, session_id, "scopes", &format!(r#"{{"frameId":{frame_id}}}"#));
                evaluate_watches(ed, session_id, frame_id);
            }
        }
        ("scopes", true) => {
            let scope_ref = v["scopes"].as_array()
                .and_then(|s| s.first())
                .and_then(|s| s["variablesReference"].as_u64());
            if let Some(r) = scope_ref {
                if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                    session.scope_ref = Some(r as i64);
                }
                request_variables(ed, session_id, r as i64);
            }
        }
        ("variables", true) => {
            let vars: Vec<crate::kernel::debug::types::Variable> = v["variables"].as_array()
                .map(|arr| arr.iter().map(|x| crate::kernel::debug::types::Variable {
                    name: x["name"].as_str().unwrap_or("?").to_string(),
                    value: x["value"].as_str().unwrap_or("").to_string(),
                    type_name: x["type"].as_str().map(|s| s.to_string()),
                    variables_reference: x["variablesReference"].as_i64().unwrap_or(0),
                }).collect())
                .unwrap_or_default();
            let r = ed.debug.sessions.get_mut(&session_id)
                .and_then(|s| s.pending_variables.take())
                .unwrap_or(0);
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.set_variables(r, vars);
            }
        }
        ("setBreakpoints", true) => {
            // The adapter echoes the breakpoints with verified flags and
            // (possibly adjusted) lines: the session stores what is real.
            let verified: Vec<(bool, usize)> = v["breakpoints"].as_array()
                .map(|arr| arr.iter()
                    .map(|bp| (bp["verified"].as_bool().unwrap_or(false),
                               bp["line"].as_u64().unwrap_or(0) as usize))
                    .collect())
                .unwrap_or_default();
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.set_verified_breakpoints(verified);
            }
        }
        ("breakpoint", true) => {
            // A single-breakpoint change event: same verification path.
            if let Some(line) = v["breakpoint"]["line"].as_u64() {
                let verified = v["breakpoint"]["verified"].as_bool().unwrap_or(false);
                if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                    session.set_verified_breakpoints(vec![(verified, line as usize)]);
                }
            }
        }
        ("threads", true) => {
            let threads: Vec<crate::kernel::debug::types::Thread> = v["threads"].as_array()
                .map(|arr| arr.iter().map(|t| crate::kernel::debug::types::Thread {
                    id: t["id"].as_u64().unwrap_or(0),
                    name: t["name"].as_str().unwrap_or("?").to_string(),
                }).collect())
                .unwrap_or_default();
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.set_threads(threads);
            }
        }
        _ => {}
    }
}

fn f_line(frame: &Option<crate::kernel::debug::types::StackFrame>) -> usize {
    frame.as_ref().map(|f| f.line).unwrap_or(1)
}

/// DAP `evaluate` response handling: correlated watch results land in the
/// session model, everything else appends to the console.
pub(crate) fn apply_dap_response(ed: &mut Editor, session_id: u64, data: &str) {
    let Some(session) = ed.debug.sessions.get_mut(&session_id) else { return };
    if let Some(i) = session.pending_watch_evals.pop_front() {
        if i < session.watch_results.len() {
            session.watch_results[i] = Some(data.to_string());
        }
        session.push_console(&format!("= {data}"));
        return;
    }
    session.push_console(&format!("= {data}"));
}

/// A failed request surfaces in the console, not just the void.
pub(crate) fn note_request_error(ed: &mut Editor, session_id: u64, command: &str, body: &str) {
    if command == "initialize" {
        return;
    }
    let message = serde_json::from_str::<serde_json::Value>(body).ok()
        .and_then(|v| v["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| "adapter error".to_string());
    let line = format!("! {command}: {message}");
    if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
        session.push_console(&line);
    } else {
        ed.show_message(MessageKind::Error, format!("{command}: {message}"));
    }
}
