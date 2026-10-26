//! The debug subsystem's background-event vocabulary.
//!
//! `DapHandle`'s reader thread and `debug::spawn_session` send these.  The
//! central `runtime::BackgroundEvent` wraps them as `BackgroundEvent::Dap(_)`
//! so the foreground keeps one ordered channel.

use std::sync::Arc;

use super::SessionId;
use super::dap_client::DapHandle;

/// An event produced by a DAP adapter connection or its startup thread.
#[derive(Debug, Clone)]
pub enum Event {
    /// The awaited DAP initialize handshake finished off-thread.
    Started {
        session_id: SessionId,
        result: Result<Arc<DapHandle>, String>,
    },
    /// The debuggee stopped (breakpoint hit, step complete, etc.).
    Stopped { session_id: SessionId, reason: String, thread_id: u64 },
    /// A line of console/stdout output from the debuggee.
    Output { session_id: SessionId, category: String, output: String },
    /// The debug adapter process terminated.
    Terminated { session_id: SessionId },
    /// A generic adapter response (evaluate result, unknown event).
    Response { session_id: SessionId, event_type: String, data: String },
    /// A DAP command response (stackTrace/scopes/variables/threads) — feeds
    /// the session model the debug pane renders.
    CommandResult { session_id: SessionId, command: String, success: bool, body: String },
}
