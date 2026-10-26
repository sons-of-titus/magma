//! Debug session — runtime state for one active debug adapter connection.

use std::collections::HashMap;
use std::collections::VecDeque;

use super::types::{FrameId, SessionId, StackFrame, Thread, ThreadId, Variable};

/// All mutable state for one debug adapter session.
#[derive(Debug, Clone)]
pub struct DebugSession {
    pub id: SessionId,
    /// Name or path of the debug adapter binary.
    pub adapter: String,
    pub threads: Vec<Thread>,
    /// Frames for the currently stopped thread.
    pub frames: Vec<StackFrame>,
    /// Variables fetched so far, keyed by their DAP `variablesReference`;
    /// the first scope's variables live under `scope_ref`.
    pub variable_scopes: HashMap<i64, Vec<Variable>>,
    /// The first scope's reference — the root of the variables tree.
    pub scope_ref: Option<i64>,
    /// The reference whose `variables` response is expected next.
    pub pending_variables: Option<i64>,
    /// Watch expression results, aligned with `DebugManager::watches`.
    pub watch_results: Vec<Option<String>>,
    /// Indices into the watch list whose `evaluate` responses are expected,
    /// in dispatch order.
    pub pending_watch_evals: VecDeque<usize>,
    /// The top stopped frame's (source path, 1-based line) — the current
    /// execution location the editor highlights.
    pub stopped_line: Option<(String, usize)>,
    /// DAP console output (adapter stdout, evaluate results), capped.
    pub console: Vec<String>,
    pub stopped: bool,
    pub stopped_thread: Option<ThreadId>,
    /// (verified, line) pairs as confirmed by the adapter — the UI's
    /// gutter shows unverified breakpoints differently.
    pub verified_breakpoints: Vec<(bool, usize)>,
    seq: i64,
}

const CONSOLE_CAP: usize = 500;

impl DebugSession {
    pub fn new(id: SessionId, adapter: String) -> Self {
        DebugSession {
            id,
            adapter,
            threads: Vec::new(),
            frames: Vec::new(),
            variable_scopes: HashMap::new(),
            scope_ref: None,
            pending_variables: None,
            watch_results: Vec::new(),
            pending_watch_evals: VecDeque::new(),
            stopped_line: None,
            console: Vec::new(),
            stopped: false,
            stopped_thread: None,
            verified_breakpoints: Vec::new(),
            seq: 1,
        }
    }

    /// Allocate the next DAP sequence number.
    pub fn next_seq(&mut self) -> i64 {
        let s = self.seq;
        self.seq += 1;
        s
    }

    /// Append a console line, dropping the oldest past the cap.
    pub fn push_console(&mut self, line: &str) {
        for piece in line.split('\n').filter(|s| !s.is_empty()) {
            self.console.push(piece.to_string());
        }
        if self.console.len() > CONSOLE_CAP {
            let overflow = self.console.len() - CONSOLE_CAP;
            self.console.drain(..overflow);
        }
    }

    /// Reset the per-stop state: watches re-arm, stale expansion caches go.
    pub fn note_stopped(&mut self, watch_count: usize) {
        self.stopped = true;
        self.frames.clear();
        self.variable_scopes.clear();
        self.scope_ref = None;
        self.stopped_line = None;
        self.watch_results = vec![None; watch_count];
        self.pending_watch_evals = VecDeque::new();
    }

    /// Update the frame list from a DAP `stackTrace` response.
    pub fn set_frames(&mut self, frames: Vec<StackFrame>) {
        self.frames = frames;
    }

    /// Update the thread list from a DAP `threads` response.
    pub fn set_threads(&mut self, threads: Vec<Thread>) {
        self.threads = threads;
    }

    /// Update the visible variables from a DAP `variables` response.
    pub fn set_variables(&mut self, r: i64, variables: Vec<Variable>) {
        self.variable_scopes.insert(r, variables);
    }

    /// Record adapter-verified breakpoints.  The manager maps them back to
    /// its per-file lists by line number.
    pub fn set_verified_breakpoints(&mut self, verified: Vec<(bool, usize)>) {
        self.verified_breakpoints = verified;
    }

    /// Return the frame with the given id, if known.
    pub fn frame(&self, id: FrameId) -> Option<&StackFrame> {
        self.frames.iter().find(|f| f.id == id)
    }
}
