//! DAP breakpoint line tracking — split from `buffer.rs` (size gate).
//!
//! Each open buffer records the byte offset a breakpoint was placed at; the
//! mutation gateway transforms those anchors through every committed
//! transaction, so breakpoint lines follow their code as it edits.

use super::anchor::Anchor;
use super::buffer::Buffer;

impl Buffer {
    /// Track a DAP breakpoint at a byte offset; the anchor follows edits.
    pub fn attach_breakpoint(&mut self, bp_id: u64, offset: usize) {
        self.detach_breakpoint(bp_id);
        self.bp_anchors.push((bp_id, Anchor::before(offset.min(self.doc.len()))));
    }

    pub fn detach_breakpoint(&mut self, bp_id: u64) {
        self.bp_anchors.retain(|(id, _)| *id != bp_id);
    }

    /// Current 1-based line of a tracked breakpoint, if still attached.
    pub fn breakpoint_line(&self, bp_id: u64) -> Option<usize> {
        let (_, anchor) = self.bp_anchors.iter().find(|(id, _)| *id == bp_id)?;
        Some(self.position_of(anchor.offset).line + 1)
    }
}
