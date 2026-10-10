//! DAP command-result handling, split from the `runtime.rs` event router.
//!
//! A stopped debuggee with an empty stack is a facade: this chain populates
//! the session model (stackTrace → scopes → variables) so the debug pane
//! renders real data.

use crate::kernel::state::Editor;

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
            let first_frame = frames.first().map(|f| f.id);
            if let Some(frame_id) = first_frame {
                #[cfg(feature = "janet")]
                crate::kernel::scripting::send_dap_command(
                    ed, session_id, "scopes", &format!(r#"{{"frameId":{frame_id}}}"#),
                );
            }
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.set_frames(frames);
            }
        }
        ("scopes", true) => {
            let scope_ref = v["scopes"].as_array()
                .and_then(|s| s.first())
                .and_then(|s| s["variablesReference"].as_u64());
            if let Some(r) = scope_ref {
                #[cfg(feature = "janet")]
                crate::kernel::scripting::send_dap_command(
                    ed, session_id, "variables", &format!(r#"{{"variablesReference":{r}}}"#),
                );
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
            if let Some(session) = ed.debug.sessions.get_mut(&session_id) {
                session.set_variables(vars);
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
            if let Some(sessions_iter) = None::<()> { let _ = sessions_iter; }
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
