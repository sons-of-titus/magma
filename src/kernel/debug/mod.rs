//! Debug subsystem — Phase 6.
//!
//! `DebugManager` orchestrates debug sessions, breakpoints, and DAP adapter
//! coordination.  The actual async DAP I/O lives in `dap_client.rs`; the
//! Janet-facing API lives in `kernel/scripting/debug_api.rs`.

pub mod types;
pub mod session;
pub mod dap_client;

pub use types::{Breakpoint, FrameId, MemoryRegion, SessionId, StackFrame, Thread, Variable};
pub use session::DebugSession;

use std::collections::HashMap;

/// Manages debug sessions, breakpoints, and active adapter state.
#[derive(Default)]
pub struct DebugManager {
    /// All active debug sessions keyed by session ID.
    pub sessions: HashMap<SessionId, DebugSession>,
    /// The session currently receiving step / continue / evaluate commands.
    pub active_session: Option<SessionId>,
    /// Breakpoints per source file path (absolute paths).
    pub breakpoints: HashMap<String, Vec<Breakpoint>>,
    next_session_id: SessionId,
    next_bp_id: u64,
}

impl DebugManager {
    pub fn new() -> Self {
        DebugManager {
            sessions: HashMap::new(),
            active_session: None,
            breakpoints: HashMap::new(),
            next_session_id: 1,
            next_bp_id: 1,
        }
    }

    /// Create a new session for the given adapter binary and return its ID.
    /// The first session created becomes the active session.
    pub fn new_session(&mut self, adapter: String) -> SessionId {
        let id = self.next_session_id;
        self.next_session_id += 1;
        self.sessions.insert(id, DebugSession::new(id, adapter));
        if self.active_session.is_none() {
            self.active_session = Some(id);
        }
        id
    }

    /// Add a breakpoint at `(file, line)` and return its ID.
    pub fn add_breakpoint(&mut self, file: String, line: usize) -> u64 {
        let id = self.next_bp_id;
        self.next_bp_id += 1;
        self.breakpoints
            .entry(file.clone())
            .or_default()
            .push(Breakpoint { id, file, line, verified: false, condition: None });
        id
    }

    /// Remove a breakpoint at `(file, line)`.  Returns `true` if one was removed.
    pub fn remove_breakpoint(&mut self, file: &str, line: usize) -> bool {
        let bps = match self.breakpoints.get_mut(file) {
            Some(v) => v,
            None => return false,
        };
        let before = bps.len();
        bps.retain(|bp| bp.line != line);
        bps.len() < before
    }

    /// All breakpoints for the given file.
    pub fn breakpoints_for(&self, file: &str) -> &[Breakpoint] {
        self.breakpoints.get(file).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// The currently active session (read-only).
    pub fn active_session(&self) -> Option<&DebugSession> {
        self.active_session.and_then(|id| self.sessions.get(&id))
    }

    /// The currently active session (mutable).
    pub fn active_session_mut(&mut self) -> Option<&mut DebugSession> {
        self.active_session.and_then(|id| self.sessions.get_mut(&id))
    }

    /// Build the `setBreakpoints` arguments JSON for a given file.
    pub fn set_breakpoints_args(&self, file: &str) -> String {
        let bps = self.breakpoints_for(file);
        let lines_json: String = bps.iter()
            .map(|bp| format!(r#"{{"line":{}}}"#, bp.line))
            .collect::<Vec<_>>()
            .join(",");
        let file_escaped = file.replace('\\', "\\\\").replace('"', "\\\"");
        format!(r#"{{"source":{{"path":"{file_escaped}"}},"breakpoints":[{lines_json}]}}"#)
    }
}
