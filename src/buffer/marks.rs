//! Marks are persistent cursor positions that survive edits.
//! Sticky marks follow insertions at their position.

use crate::state::id::MarkId;

#[derive(Debug, Clone)]
pub struct Mark {
    pub id: MarkId,
    pub pos: usize,
    pub sticky: bool,
}

#[derive(Debug, Clone)]
pub struct MarkSet {
    marks: Vec<Mark>,
    next_id: u64,
}

impl Default for MarkSet {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkSet {
    pub fn new() -> Self {
        MarkSet {
            marks: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create(&mut self, pos: usize, sticky: bool) -> MarkId {
        let id = MarkId(self.next_id);
        self.next_id += 1;
        self.marks.push(Mark { id, pos, sticky });
        id
    }

    pub fn get(&self, id: MarkId) -> Option<&Mark> {
        self.marks.iter().find(|m| m.id == id)
    }

    pub fn get_mut(&mut self, id: MarkId) -> Option<&mut Mark> {
        self.marks.iter_mut().find(|m| m.id == id)
    }

    pub fn delete(&mut self, id: MarkId) -> bool {
        let len_before = self.marks.len();
        self.marks.retain(|m| m.id != id);
        self.marks.len() < len_before
    }

    pub fn all_positions(&self) -> Vec<(MarkId, usize)> {
        self.marks.iter().map(|m| (m.id, m.pos)).collect()
    }

    pub fn restore_positions(&mut self, states: &[(MarkId, usize)]) {
        for &(id, pos) in states {
            if let Some(mark) = self.get_mut(id) {
                mark.pos = pos;
            }
        }
    }

    /// Adjust all mark positions after an insertion
    pub fn adjust_for_insert(&mut self, offset: usize, len: usize) {
        for mark in &mut self.marks {
            if (mark.sticky && mark.pos >= offset) || (!mark.sticky && mark.pos > offset) {
                mark.pos += len;
            }
        }
    }

    /// Adjust all mark positions after a deletion
    pub fn adjust_for_delete(&mut self, start: usize, end: usize) {
        let deleted = end - start;
        for mark in &mut self.marks {
            if mark.pos > start {
                mark.pos = if mark.pos <= end {
                    start
                } else {
                    mark.pos - deleted
                };
            }
        }
    }
}
