//! Kernel document — rope + revision + change history behind one mutation
//! gateway.
//!
//! Phase 2 done (R-23/F-25 closed): the live `Buffer` is a facade over this
//! type — every mutation is a `commit`, validation happens before any rope
//! mutation, and one commit is exactly one revision transition.  Consumers
//! sync through `changes_since` (tree-sitter, the Janet change API) or the
//! buffer's pending-change pump.

use super::line_index::LineIndex;
use super::revision::Revision;
use super::rope::Rope;
use super::transaction::{
    CommittedEdit, EditTransaction, TransactionBuilder, TransactionError,
};

/// Transactions retained for incremental consumers.  Past this window,
/// `changes_since` reports `None` and consumers must re-sync from scratch.
const MAX_HISTORY: usize = 4096;

#[derive(Debug)]
pub struct Document {
    rope: Rope,
    revision: Revision,
    /// Committed history, ascending, bounded to the incremental-consumer
    /// window.  Undo data lives in `UndoHistory`, not here.
    history: Vec<EditTransaction>,
}

impl Document {
    pub fn new() -> Self {
        Document {
            rope: Rope::new(),
            revision: Revision::first(),
            history: Vec::new(),
        }
    }

    pub fn from_string(content: &str) -> Self {
        Document {
            rope: Rope::from_string(content),
            ..Document::new()
        }
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    /// Whole-document text.  Tests and non-hot-path consumers only; the
    /// render path must never need this (Phase 9).
    pub fn text(&self) -> String {
        self.rope.slice(0, self.rope.len()).into_owned()
    }

    /// Snap an offset down to the nearest char boundary, clamped to len.
    /// The forgiving boundary of the public buffer API.
    pub fn snap_offset(&self, offset: usize) -> usize {
        self.rope.floor_boundary(offset)
    }

    pub fn chars_between(&self, start: usize, end: usize) -> usize {
        self.rope.chars_between(start, end)
    }

    pub fn rope_text(&self, start: usize, end: usize) -> String {
        self.rope.slice(start, end).into_owned()
    }

    pub fn char_at(&self, offset: usize) -> Option<char> {
        self.rope.char_at(offset)
    }

    pub fn rope_handle(&self) -> &Rope {
        &self.rope
    }

    pub fn line_index(&self) -> LineIndex<'_> {
        LineIndex::new(&self.rope)
    }

    /// Transactions committed strictly after `seen` — the canonical change
    /// feed for every consumer.  `None` when `seen` predates the retention
    /// window: the consumer must re-sync from the document instead of
    /// applying a silently-partial feed.
    pub fn changes_since(&self, seen: Revision) -> Option<&[EditTransaction]> {
        let oldest = self.history.first().map(|t| t.revision_before)?;
        if seen < oldest {
            return None;
        }
        let pos = self
            .history
            .partition_point(|t| t.revision_before < seen);
        Some(&self.history[pos..])
    }

    pub fn history(&self) -> &[EditTransaction] {
        &self.history
    }

    /// The one mutation path.  `Ok(None)` means the builder held no
    /// surviving edits: nothing was applied and the revision did not move.
    pub fn commit(
        &mut self,
        builder: TransactionBuilder,
    ) -> Result<Option<EditTransaction>, TransactionError> {
        let origin = builder.origin();
        let edits = builder.normalize()?;
        if edits.is_empty() {
            return Ok(None);
        }

        let len = self.rope.len();
        for edit in &edits {
            if edit.range.end > len {
                return Err(TransactionError::OutOfBounds {
                    range: edit.range.clone(),
                    len,
                });
            }
            if !self.rope.is_char_boundary(edit.range.start)
                || !self.rope.is_char_boundary(edit.range.end)
            {
                return Err(TransactionError::NotCharBoundary {
                    offset: if self.rope.is_char_boundary(edit.range.start) {
                        edit.range.end
                    } else {
                        edit.range.start
                    },
                });
            }
        }

        // Read old text for every edit first, then mutate back-to-front so
        // pre-transaction coordinates stay valid for every applied edit.
        let committed: Vec<CommittedEdit> = edits
            .iter()
            .map(|edit| CommittedEdit {
                edit: edit.clone(),
                old_text: self.rope.slice(edit.range.start, edit.range.end).into_owned(),
            })
            .collect();

        for edit in edits.iter().rev() {
            if edit.range.start < edit.range.end {
                self.rope.delete(edit.range.start, edit.range.end);
            }
            if !edit.inserted_text.is_empty() {
                self.rope.insert(edit.range.start, &edit.inserted_text);
            }
        }

        let revision_before = self.revision;
        self.revision = revision_before.next();
        let txn = EditTransaction {
            revision_before,
            revision_after: self.revision,
            edits: committed,
            origin,
        };
        self.history.push(txn.clone());
        if self.history.len() > MAX_HISTORY {
            self.history.drain(0..self.history.len() - MAX_HISTORY);
        }
        Ok(Some(txn))
    }
}

impl Default for Document {
    fn default() -> Self {
        Document::new()
    }
}
