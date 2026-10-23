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
    /// Live adapter connections, foreground-owned (rework.md §1).
    pub connections: HashMap<SessionId, std::sync::Arc<dap_client::DapHandle>>,
    /// Watch expressions evaluated whenever the session stops.
    pub watches: Vec<String>,
    next_session_id: SessionId,
    next_bp_id: u64,
}

impl DebugManager {
    pub fn new() -> Self {
        DebugManager {
            sessions: HashMap::new(),
            active_session: None,
            breakpoints: HashMap::new(),
            connections: HashMap::new(),
            watches: Vec::new(),
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

    /// Add a breakpoint at `(file, line)` and return its ID.  The breakpoint
    /// starts enabled; line tracking for open buffers attaches separately.
    pub fn add_breakpoint(&mut self, file: String, line: usize) -> u64 {
        let id = self.next_bp_id;
        self.next_bp_id += 1;
        self.breakpoints
            .entry(file.clone())
            .or_default()
            .push(Breakpoint { id, file, line, verified: false, enabled: true, condition: None });
        id
    }

    /// Remove a breakpoint at `(file, line)`.  Returns the removed ids.
    pub fn remove_breakpoint(&mut self, file: &str, line: usize) -> Vec<u64> {
        let Some(bps) = self.breakpoints.get_mut(file) else { return Vec::new() };
        let removed: Vec<u64> = bps.iter()
            .filter(|bp| bp.line == line)
            .map(|bp| bp.id)
            .collect();
        bps.retain(|bp| bp.line != line);
        if bps.is_empty() {
            self.breakpoints.remove(file);
        }
        removed
    }

    /// Enable or disable every breakpoint at `(file, line)`.
    pub fn set_enabled(&mut self, file: &str, line: usize, enabled: bool) {
        if let Some(bps) = self.breakpoints.get_mut(file) {
            for bp in bps.iter_mut().filter(|bp| bp.line == line) {
                bp.enabled = enabled;
            }
        }
    }

    /// All breakpoints for the given file.
    pub fn breakpoints_for(&self, file: &str) -> &[Breakpoint] {
        self.breakpoints.get(file).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Every breakpoint in the project, across files, ordered by (file, line).
    pub fn all_breakpoints(&self) -> Vec<&Breakpoint> {
        let mut all: Vec<&Breakpoint> = self.breakpoints.values().flatten().collect();
        all.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        all
    }

    /// The currently active session (read-only).
    pub fn active_session(&self) -> Option<&DebugSession> {
        self.active_session.and_then(|id| self.sessions.get(&id))
    }

    /// The currently active session (mutable).
    pub fn active_session_mut(&mut self) -> Option<&mut DebugSession> {
        self.active_session.and_then(|id| self.sessions.get_mut(&id))
    }

    /// Ordered teardown of one session's adapter.
    pub fn disconnect(&mut self, id: SessionId) {
        if let Some(handle) = self.connections.remove(&id) {
            handle.disconnect();
        }
        self.sessions.remove(&id);
        if self.active_session == Some(id) {
            self.active_session = None;
        }
    }

    /// Teardown of everything — the editor session is ending.
    pub fn disconnect_all(&mut self) {
        for handle in self.connections.values() {
            handle.disconnect();
        }
        self.connections.clear();
        self.sessions.clear();
        self.active_session = None;
    }

    /// Build the `setBreakpoints` arguments JSON for a given file.  Disabled
    /// breakpoints are omitted — DAP `SourceBreakpoint` has no enabled flag,
    /// so a disabled breakpoint simply doesn't reach the adapter.
    pub fn set_breakpoints_args(&self, file: &str) -> String {
        let bps = self.breakpoints_for(file).iter()
            .filter(|bp| bp.enabled)
            .collect::<Vec<_>>();
        let lines_json: String = bps.iter()
            .map(|bp| format!(r#"{{"line":{}}}"#, bp.line))
            .collect::<Vec<_>>()
            .join(",");
        let file_escaped = file.replace('\\', "\\\\").replace('"', "\\\"");
        format!(r#"{{"source":{{"path":"{file_escaped}"}},"breakpoints":[{lines_json}]}}"#)
    }
}

/// Re-anchor breakpoint lines from open buffers: each buffer transforms its
/// breakpoint anchors through edits, and this copies the tracked positions
/// back into the manager before the lines go to an adapter or the UI.
pub fn sync_breakpoint_lines(ed: &mut crate::kernel::state::Editor) {
    let files: Vec<String> = ed.debug.breakpoints.keys().cloned().collect();
    for file in files {
        let Some(arc) = ed.buffers.iter()
            .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref() == Some(file.as_str()))
            .map(|(_, a)| a.clone()) else { continue };
        let updates: Vec<(u64, usize)> = {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            buf.bp_anchors.iter()
                .filter_map(|(id, _)| buf.breakpoint_line(*id).map(|line| (*id, line)))
                .collect()
        };
        for (id, line) in updates {
            for bp in ed.debug.breakpoints.get_mut(&file).into_iter().flatten() {
                if bp.id == id {
                    bp.line = line;
                }
            }
        }
    }
}

/// Enable or disable breakpoints at a location and resync the adapter.
pub fn set_breakpoint_enabled(ed: &mut crate::kernel::state::Editor, file: &str, line: usize, enabled: bool) {
    ed.debug.set_enabled(file, line, enabled);
    sync_breakpoint_lines(ed);
    #[cfg(feature = "janet")]
    if let Some(sid) = ed.debug.active_session {
        let args = ed.debug.set_breakpoints_args(file);
        crate::kernel::scripting::send_dap_command(ed, sid, "setBreakpoints", &args);
    }
}

/// Toggle a breakpoint at a 1-based line — the native path behind gutter
/// clicks.  Lives in Rust, not Janet: built-in behavior must not depend on
/// event handlers registered by init-time scripts.
pub fn toggle_breakpoint_at(ed: &mut crate::kernel::state::Editor, file: &str, line: usize) {
    let existing = ed.debug.breakpoints_for(file).iter().any(|bp| bp.line == line);
    if existing {
        remove_breakpoints_at(ed, file, line);
        return;
    }
    let bp_id = ed.debug.add_breakpoint(file.to_string(), line);
    if let Some(arc) = ed.buffers.iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref() == Some(file))
        .map(|(_, a)| a.clone()) {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let offset = buf.line_start_offset(line.saturating_sub(1)).unwrap_or(0);
        buf.attach_breakpoint(bp_id, offset);
    }
    sync_breakpoint_lines(ed);
    #[cfg(feature = "janet")]
    if let Some(sid) = ed.debug.active_session {
        let args = ed.debug.set_breakpoints_args(file);
        crate::kernel::scripting::send_dap_command(ed, sid, "setBreakpoints", &args);
    }
    ed.events.emit_typed(
        crate::kernel::event::keys::events::DEBUG_BREAKPOINT_CHANGED,
        crate::kernel::event::payload::DebugBreakpointChangedPayload {
            file: file.to_string(),
            line: line.to_string(),
            action: "add".to_string(),
        },
    );
}

/// Remove every breakpoint at a location: manager, buffer anchors, adapter.
pub fn remove_breakpoints_at(ed: &mut crate::kernel::state::Editor, file: &str, line: usize) {
    let removed = ed.debug.remove_breakpoint(file, line);
    if let Some(arc) = ed.buffers.iter()
        .find(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).path.as_deref() == Some(file))
        .map(|(_, a)| a.clone()) {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        for id in removed {
            buf.detach_breakpoint(id);
        }
    }
    #[cfg(feature = "janet")]
    if let Some(sid) = ed.debug.active_session {
        let args = ed.debug.set_breakpoints_args(file);
        crate::kernel::scripting::send_dap_command(ed, sid, "setBreakpoints", &args);
    }
    ed.events.emit_typed(
        crate::kernel::event::keys::events::DEBUG_BREAKPOINT_CHANGED,
        crate::kernel::event::payload::DebugBreakpointChangedPayload {
            file: file.to_string(),
            line: line.to_string(),
            action: "remove".to_string(),
        },
    );
}

/// Spawn an adapter off the foreground and register the session; readiness
/// arrives via `DapStarted`.  Shared by `debug/start` and `debug-config`.
pub fn spawn_session(
    ed: &mut crate::kernel::state::Editor,
    adapter: &str,
    launch_config: Option<serde_json::Value>,
) -> Result<SessionId, String> {
    use crate::kernel::runtime::BackgroundEvent;

    let bg = ed.background.clone()
        .ok_or_else(|| "no background runtime".to_string())?;
    let session_id = ed.debug.new_session(adapter.to_string());
    let adapter = adapter.to_string();
    std::thread::Builder::new()
        .name(format!("dap-init-{session_id}"))
        .spawn(move || {
            let result = dap_client::DapHandle::start(
                &adapter, session_id, launch_config, bg.clone()).map(std::sync::Arc::new);
            bg.send_event(BackgroundEvent::DapStarted { session_id, result });
        })
        .map_err(|e| format!("dap-init thread: {e}"))?;
    Ok(session_id)
}

/// A saved file's breakpoints go back to the adapter — its lines may have
/// shifted while the debuggee was running, and the adapter still targets
/// the old ones.
pub fn resync_after_save(ed: &mut crate::kernel::state::Editor, path: &str) {
    if ed.debug.active_session.is_none() || !ed.debug.breakpoints.contains_key(path) {
        return;
    }
    sync_breakpoint_lines(ed);
    #[cfg(feature = "janet")]
    if let Some(sid) = ed.debug.active_session {
        let args = ed.debug.set_breakpoints_args(path);
        crate::kernel::scripting::send_dap_command(ed, sid, "setBreakpoints", &args);
    }
}
