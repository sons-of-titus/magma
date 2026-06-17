//! Debug session — runtime state for one active debug adapter connection.

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
    /// Variables visible in the current frame.
    pub variables: Vec<Variable>,
    pub stopped: bool,
    pub stopped_thread: Option<ThreadId>,
    seq: i64,
}

impl DebugSession {
    pub fn new(id: SessionId, adapter: String) -> Self {
        DebugSession {
            id,
            adapter,
            threads: Vec::new(),
            frames: Vec::new(),
            variables: Vec::new(),
            stopped: false,
            stopped_thread: None,
            seq: 1,
        }
    }

    /// Allocate the next DAP sequence number.
    pub fn next_seq(&mut self) -> i64 {
        let s = self.seq;
        self.seq += 1;
        s
    }

    /// Update the frame list from a DAP `stackTrace` response.
    pub fn set_frames(&mut self, frames: Vec<StackFrame>) {
        self.frames = frames;
    }

    /// Update the thread list from a DAP `threads` response.
    pub fn set_threads(&mut self, threads: Vec<Thread>) {
        self.threads = threads;
    }

    /// Return the frame with the given id, if known.
    pub fn frame(&self, id: FrameId) -> Option<&StackFrame> {
        self.frames.iter().find(|f| f.id == id)
    }
}
