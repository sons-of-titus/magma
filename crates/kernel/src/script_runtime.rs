//! ScriptRuntime trait — abstraction over the optional scripting backend.
//!
//! The kernel never names a concrete scripting implementation; binaries inject
//! one (e.g. `scripting::JanetRuntime`) via `BootOptions`/`bootstrap`.

use std::collections::HashMap;

use crate::command::CommandResult;
use crate::command::args::ArgValue;
use crate::state::Editor;

pub trait ScriptRuntime: Send + Sync {
    fn init(&mut self, editor: &mut Editor);
    fn eval(&mut self, editor: &mut Editor, expr: &str) -> String;
    fn eval_result(&mut self, editor: &mut Editor, expr: &str) -> Result<String, String>;
    fn load_file(&mut self, editor: &mut Editor, path: &str) -> Result<(), String>;
    fn call_command(
        &mut self,
        editor: &mut Editor,
        name: &str,
        args: &HashMap<String, ArgValue>,
    ) -> CommandResult;
    fn execute_stored_task(&mut self, _editor: &mut Editor, _task_id: u64) {}
    fn execute_task_complete_callback(&mut self, _editor: &mut Editor, _task_id: u64) {}

    /// Run the event-bus dispatch inside the scripting scope (Janet VM).
    /// Default: drain directly (no scripting).
    fn dispatch_events(&mut self, ed: &mut Editor) {
        let _ = ed;
        // no-op default; kernel falls back to direct drain
    }

    /// Send a DAP command. Default implementation sends directly through the
    /// debug connection (no scripting scope required).
    fn send_dap_command(
        &mut self,
        ed: &mut Editor,
        session_id: u64,
        command: &str,
        arguments: &str,
    ) {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));
        if let Some(handle) = ed.debug.connections.get(&session_id).cloned() {
            handle.request(command, args);
        }
    }
}

/// Kernel-side helper: dispatch events, using the scripting runtime if present.
pub fn dispatch_events(ed: &mut Editor) {
    if let Some(rt) = ed.runtime.take() {
        let mut rt = rt;
        rt.dispatch_events(ed);
        ed.runtime = Some(rt);
    } else {
        ed.events.drain_and_dispatch();
    }
}

/// Kernel-side helper: send a DAP command via the scripting runtime if present.
pub fn send_dap_command(ed: &mut Editor, session_id: u64, command: &str, arguments: &str) {
    if let Some(rt) = ed.runtime.take() {
        let mut rt = rt;
        rt.send_dap_command(ed, session_id, command, arguments);
        ed.runtime = Some(rt);
    } else {
        // No runtime: send directly through the debug connection.
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));
        if let Some(handle) = ed.debug.connections.get(&session_id).cloned() {
            handle.request(command, args);
        }
    }
}
