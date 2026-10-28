//! Transaction-based undo history (docs/core-redesign.md §4).
//!
//! One undo step = one *group* of transactions.  Groups are created
//! explicitly: an insert session (`open_undo_session_at`) groups everything
//! typed until the session closes; every other transaction is its own
//! group.  There is no wall-clock merge window.
//!
//! Branching policy (§4.4): a new edit after an undo discards the redo
//! branch — the deliberate, documented choice (vim/Emacs behavior), not an
//! accident of storage.  A persistent undo tree can replace this structure
//! later without changing call sites.

use std::collections::VecDeque;

use super::transaction::{EditTransaction, TransactionOrigin};

const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub txn: EditTransaction,
    group: u64,
    /// Cursor offset before the transaction was applied.
    pub cursor_before: usize,
    /// Cursor offset after the transaction was applied.
    pub cursor_after: usize,
}

#[derive(Debug)]
pub struct UndoHistory {
    entries: VecDeque<UndoEntry>,
    /// Index of the newest applied entry; -1 = none.
    cursor: isize,
    open_group: Option<u64>,
    next_group: u64,
    /// Undo-cursor position at the last save; `None` = never saved.
    /// Modified ⟺ the undo cursor has moved off the saved position —
    /// undoing back to the saved text is clean (vim semantics), while a
    /// stale async save can never mark newer edits clean (§5).
    saved_cursor: Option<isize>,
}

impl Default for UndoHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoHistory {
    pub fn new() -> Self {
        UndoHistory {
            entries: VecDeque::new(),
            cursor: -1,
            open_group: None,
            next_group: 0,
            // Revision 0 (the loaded content) is the saved state: a buffer
            // freshly read from disk must not report modified.
            saved_cursor: Some(-1),
        }
    }

    pub fn can_undo(&self) -> bool {
        self.cursor >= 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.entries.len() as isize
    }

    /// Begin an explicit group: all transactions recorded until
    /// `close_group` form one undo step.
    pub fn open_group(&mut self) {
        if self.open_group.is_none() {
            self.next_group += 1;
            self.open_group = Some(self.next_group);
        }
    }

    pub fn close_group(&mut self) {
        self.open_group = None;
    }

    pub fn in_open_group(&self) -> bool {
        self.open_group.is_some()
    }

    /// Record a committed transaction.  Undo/redo-origin transactions are
    /// re-derivations of existing history and are not recorded.
    pub fn record(&mut self, txn: EditTransaction, cursor_before: usize, cursor_after: usize) {
        if matches!(
            txn.origin,
            TransactionOrigin::Undo | TransactionOrigin::Redo
        ) {
            return;
        }
        let keep = (self.cursor + 1) as usize;
        if keep < self.entries.len() {
            self.entries.truncate(keep);
        }
        let group = match self.open_group {
            Some(g) => g,
            None => {
                self.next_group += 1;
                self.next_group
            }
        };
        self.entries.push_back(UndoEntry {
            txn,
            group,
            cursor_before,
            cursor_after,
        });
        if self.entries.len() > MAX_ENTRIES {
            self.entries.pop_front();
            // The oldest entry is gone; the saved position shifts with it.
            // If the saved entry itself was evicted, that state is no longer
            // undo-reachable and can never compare clean again.
            self.saved_cursor = match self.saved_cursor {
                Some(s) if s > 0 => Some(s - 1),
                Some(_) => None,
                None => None,
            };
        }
        self.cursor = self.entries.len() as isize - 1;
    }

    /// The transaction range of the next undo step, plus the cursor offset
    /// to restore (the position where the group began).  Does not mutate.
    pub fn undo_step(&self) -> Option<(usize, usize, usize)> {
        let end = self.cursor;
        if end < 0 {
            return None;
        }
        let group = self.entries[end as usize].group;
        let mut start = end;
        while start > 0 && self.entries[(start - 1) as usize].group == group {
            start -= 1;
        }
        let restore = self.entries[start as usize].cursor_before;
        Some((start as usize, end as usize, restore))
    }

    /// The transaction range of the next redo step, plus the cursor offset
    /// to restore (the position where the group ended).  Does not mutate.
    pub fn redo_step(&self) -> Option<(usize, usize, usize)> {
        let start = self.cursor + 1;
        if start as usize >= self.entries.len() {
            return None;
        }
        let group = self.entries[start as usize].group;
        let mut end = start;
        while end + 1 < self.entries.len() as isize
            && self.entries[(end + 1) as usize].group == group
        {
            end += 1;
        }
        let restore = self.entries[end as usize].cursor_after;
        Some((start as usize, end as usize, restore))
    }

    pub fn entry(&self, idx: usize) -> &UndoEntry {
        &self.entries[idx]
    }

    /// Complete an undo: cursor moves in front of the undone range.
    pub fn finish_undo(&mut self, start: usize) {
        self.cursor = start as isize - 1;
    }

    /// Complete a redo: cursor moves onto the redone range.
    pub fn finish_redo(&mut self, end: usize) {
        self.cursor = end as isize;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = -1;
        self.open_group = None;
        self.saved_cursor = Some(-1);
    }

    pub fn cursor_pos(&self) -> isize {
        self.cursor
    }

    pub fn mark_saved_at_cursor(&mut self, cursor: isize) {
        self.saved_cursor = Some(cursor);
    }

    /// Undo-cursor position of the entry whose post-revision is `rev`.
    /// Revision 0 is the initial content (position -1).
    pub fn cursor_for_revision(&self, rev: u64) -> Option<isize> {
        if rev == 0 {
            return Some(-1);
        }
        self.entries
            .iter()
            .position(|e| e.txn.revision_after.0 == rev)
            .map(|i| i as isize)
    }

    pub fn is_modified(&self) -> bool {
        self.saved_cursor != Some(self.cursor)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
