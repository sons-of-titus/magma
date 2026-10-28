//! Marks are persistent buffer positions that survive edits.
//!
//! Marks are anchors (docs/core-redesign.md §0.4/§A.2): they transform
//! through each committed transaction.  Sticky marks follow insertions at
//! their position (`Affinity::After`); non-sticky marks stay left
//! (`Affinity::Before`).  There is no eager adjustment and no snapshot
//! restoration — undo/redo transform marks like any other transaction.

use super::anchor::{Affinity, Anchor};
use super::transaction::EditTransaction;
use id::MarkId;

#[derive(Debug, Clone)]
pub struct Mark {
    pub id: MarkId,
    anchor: Anchor,
}

impl Mark {
    pub fn pos(&self) -> usize {
        self.anchor.offset
    }

    pub fn sticky(&self) -> bool {
        matches!(self.affinity(), Affinity::After)
    }

    fn affinity(&self) -> Affinity {
        self.anchor.affinity
    }
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
        self.marks.push(Mark {
            id,
            anchor: Anchor {
                offset: pos,
                affinity: if sticky {
                    Affinity::After
                } else {
                    Affinity::Before
                },
            },
        });
        id
    }

    pub fn get(&self, id: MarkId) -> Option<&Mark> {
        self.marks.iter().find(|m| m.id == id)
    }

    pub fn delete(&mut self, id: MarkId) -> bool {
        let len_before = self.marks.len();
        self.marks.retain(|m| m.id != id);
        self.marks.len() < len_before
    }

    pub fn len(&self) -> usize {
        self.marks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// The one mark-movement path: transform every mark through a committed
    /// transaction.  No other span arithmetic exists for marks.
    pub fn apply_transaction(&mut self, txn: &EditTransaction) {
        for mark in &mut self.marks {
            mark.anchor = txn.transform_anchor(mark.anchor);
        }
    }
}
