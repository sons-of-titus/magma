//! Undo/Redo tree for buffer operations.
//! Adjacent edits within a time window are merged into groups.

use crate::state::id::MarkId;

const MERGE_WINDOW_MS: u64 = 200;

#[derive(Debug, Clone)]
pub enum UndoOp {
    Insert { offset: usize, text: String },
    Delete { offset: usize, text: String },
}

#[derive(Debug, Clone)]
struct UndoGroup {
    ops: Vec<UndoOp>,
    timestamp: std::time::Instant,
    #[allow(dead_code)]
    merge_id: u64,
    mark_states: Vec<(MarkId, usize)>,
    /// When true, all subsequent `record` calls merge into this group
    /// regardless of the time window. Used for insert sessions.
    open: bool,
}

#[derive(Debug)]
pub struct UndoTree {
    groups: Vec<UndoGroup>,
    cursor: isize,
    saved_at: Option<usize>,
    merge_counter: u64,
    max_groups: usize,
}

impl UndoTree {
    pub fn new(max_groups: usize) -> Self {
        UndoTree {
            groups: Vec::with_capacity(max_groups),
            cursor: -1,
            saved_at: None,
            merge_counter: 0,
            max_groups,
        }
    }

    /// Open a new insert session — all subsequent operations merge
    /// into this group until `close_session()` is called.
    pub fn open_session(&mut self, op: UndoOp, mark_states: Vec<(MarkId, usize)>) {
        // Truncate any redo history beyond cursor
        let keep_len = (self.cursor + 1) as usize;
        self.groups.truncate(keep_len);

        // If the last group is still open, close it first
        if let Some(last) = self.groups.last_mut() {
            last.open = false;
        }

        self.merge_counter += 1;
        let now = std::time::Instant::now();
        self.groups.push(UndoGroup {
            ops: vec![op],
            timestamp: now,
            merge_id: self.merge_counter,
            mark_states,
            open: true,
        });

        if self.groups.len() > self.max_groups {
            self.groups.remove(0);
            if self.cursor >= 0 {
                self.cursor -= 1;
            }
            if let Some(saved) = self.saved_at {
                if saved > 0 {
                    self.saved_at = Some(saved - 1);
                } else {
                    self.saved_at = None;
                }
            }
        }

        self.cursor = self.groups.len() as isize - 1;
    }

    /// Close the current open group so future edits start a new step.
    pub fn close_session(&mut self) {
        if let Some(last) = self.groups.last_mut() {
            last.open = false;
        }
    }

    pub fn record(&mut self, op: UndoOp, mark_states: Vec<(MarkId, usize)>) {
        // Remove any redo history beyond cursor
        let keep_len = (self.cursor + 1) as usize;
        self.groups.truncate(keep_len);

        // If the last group is open, always merge into it
        if let Some(last) = self.groups.last_mut()
            && last.open {
                last.ops.push(op);
                last.timestamp = std::time::Instant::now();
                last.mark_states = mark_states;
                return;
            }

        // Try to merge with the last group based on time window
        let now = std::time::Instant::now();
        let should_merge = self.groups.last().is_some_and(|last| {
            last.ops.len() < 100
                && (now.duration_since(last.timestamp).as_millis() as u64) < MERGE_WINDOW_MS
        });

        if should_merge
            && let Some(last) = self.groups.last_mut() {
                last.ops.push(op);
                last.timestamp = now;
                last.mark_states = mark_states;
                return;
            }

        self.merge_counter += 1;
        self.groups.push(UndoGroup {
            ops: vec![op],
            timestamp: now,
            merge_id: self.merge_counter,
            mark_states,
            open: false,
        });

        // Enforce max groups
        if self.groups.len() > self.max_groups {
            self.groups.remove(0);
            if self.cursor >= 0 {
                self.cursor -= 1;
            }
            if let Some(saved) = self.saved_at {
                if saved > 0 {
                    self.saved_at = Some(saved - 1);
                } else {
                    self.saved_at = None;
                }
            }
        }

        self.cursor = self.groups.len() as isize - 1;
    }

    pub fn can_undo(&self) -> bool {
        self.cursor >= 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.groups.len() as isize
    }

    pub fn undo(&mut self) -> Option<&[UndoOp]> {
        if !self.can_undo() {
            return None;
        }
        let group = &self.groups[self.cursor as usize];
        self.cursor -= 1;
        Some(&group.ops)
    }

    pub fn redo(&mut self) -> Option<&[UndoOp]> {
        if !self.can_redo() {
            return None;
        }
        self.cursor += 1;
        let group = &self.groups[self.cursor as usize];
        Some(&group.ops)
    }

    pub fn undo_mark_states(&self) -> Option<&[(MarkId, usize)]> {
        if !self.can_undo() {
            return None;
        }
        Some(&self.groups[self.cursor as usize].mark_states)
    }

    pub fn redo_mark_states(&self) -> Option<&[(MarkId, usize)]> {
        if !self.can_redo() {
            return None;
        }
        Some(&self.groups[(self.cursor + 1) as usize].mark_states)
    }

    pub fn mark_saved(&mut self) {
        if self.cursor >= 0 {
            self.saved_at = Some(self.cursor as usize);
        } else {
            self.saved_at = None;
        }
    }

    pub fn is_modified_since_save(&self) -> bool {
        self.saved_at.is_none_or(|saved| {
            self.cursor < 0 || saved != self.cursor as usize
        })
    }

    pub fn clear(&mut self) {
        self.groups.clear();
        self.cursor = -1;
        self.saved_at = None;
    }
}
