//! Edit transactions — N edits, one revision transition.
//!
//! Edits in a transaction are *parallel*: all ranges are expressed in
//! `revision_before` coordinates (like an LSP workspace edit).  Normalization
//! sorts by start, merges abutting ranges, and rejects overlaps, so the
//! closed ranges end up strictly separated and anchor transformation over
//! the list is well-defined at every boundary (docs/core-redesign.md §A.3).

use std::ops::Range;
use thiserror::Error;

use super::edit::Edit;
use super::revision::Revision;

/// What produced a transaction.  Undo/redo commits carry their own origins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransactionOrigin {
    /// Direct typing / interactive editing.
    #[default]
    User,
    /// An editor command (colon-mode verb, keymap action).
    Command,
    /// A whole-buffer or region reformat (replace-all, auto-indent).
    Format,
    /// An LSP workspace/formatting edit applied locally.
    Lsp,
    Undo,
    Redo,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransactionError {
    #[error("edits overlap: {first:?} and {second:?}")]
    OverlappingEdits { first: Range<usize>, second: Range<usize> },
    #[error("invalid range (end before start): {0:?}")]
    InvalidRange(Range<usize>),
    #[error("range {range:?} out of bounds for document of {len} bytes")]
    OutOfBounds { range: Range<usize>, len: usize },
    #[error("offset {offset} is not a UTF-8 char boundary")]
    NotCharBoundary { offset: usize },
}

/// Accumulates the edits of one logical change (one keystroke, one
/// multi-cursor insertion, one replace-all, one workspace edit).
#[derive(Debug, Clone, Default)]
pub struct TransactionBuilder {
    edits: Vec<Edit>,
    origin: TransactionOrigin,
}

impl TransactionBuilder {
    pub fn new() -> Self {
        TransactionBuilder::default()
    }

    pub fn with_origin(mut self, origin: TransactionOrigin) -> Self {
        self.origin = origin;
        self
    }

    pub fn origin(&self) -> TransactionOrigin {
        self.origin
    }

    pub fn insert(mut self, offset: usize, text: &str) -> Self {
        self.edits.push(Edit::insert(offset, text));
        self
    }

    pub fn delete(mut self, range: Range<usize>) -> Self {
        self.edits.push(Edit::delete(range));
        self
    }

    pub fn replace(mut self, range: Range<usize>, text: &str) -> Self {
        self.edits.push(Edit::replace(range, text));
        self
    }

    pub fn push(mut self, edit: Edit) -> Self {
        self.edits.push(edit);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.edits.iter().all(|e| e.is_noop())
    }

    /// Sort, drop no-ops, merge abutting ranges, reject overlaps.
    /// Pure: needs no text access.
    pub fn normalize(self) -> Result<Vec<Edit>, TransactionError> {
        let mut edits: Vec<Edit> = self
            .edits
            .into_iter()
            .filter(|e| !e.is_noop())
            .collect();
        for e in &edits {
            if e.range.end < e.range.start {
                return Err(TransactionError::InvalidRange(e.range.clone()));
            }
        }
        edits.sort_by_key(|e| (e.range.start, e.range.end));

        let mut out: Vec<Edit> = Vec::with_capacity(edits.len());
        for edit in edits {
            match out.last_mut() {
                Some(prev) if edit.range.start <= prev.range.end => {
                    if edit.range.start < prev.range.end {
                        return Err(TransactionError::OverlappingEdits {
                            first: prev.range.clone(),
                            second: edit.range,
                        });
                    }
                    // Abutting: one replacement spanning both.
                    prev.inserted_text.push_str(&edit.inserted_text);
                    prev.range.end = edit.range.end;
                }
                _ => out.push(edit),
            }
        }
        Ok(out)
    }
}

/// An edit plus the text it displaced — enough to reverse it exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedEdit {
    pub edit: Edit,
    pub old_text: String,
}

/// The immutable record of one committed change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditTransaction {
    pub revision_before: Revision,
    pub revision_after: Revision,
    /// Ascending, disjoint, strictly non-abutting; pre-transaction coordinates.
    pub edits: Vec<CommittedEdit>,
    pub origin: TransactionOrigin,
}

impl EditTransaction {
    /// The edits that reverse this transaction, in post-transaction
    /// coordinates (ascending, disjoint).  Edit i's inserted text physically
    /// sits at `start + cumulative delta of the preceding edits` in the
    /// final text, so each inverse range carries that shift.
    pub fn inverse_edits(&self) -> Vec<Edit> {
        let mut delta: isize = 0;
        self.edits
            .iter()
            .map(|ce| {
                let start = (ce.edit.start() as isize + delta).max(0) as usize;
                let end = start + ce.edit.inserted_len();
                delta += ce.edit.inserted_len() as isize
                    - (ce.edit.end() - ce.edit.start()) as isize;
                Edit::replace(start..end, &ce.old_text)
            })
            .collect()
    }

    /// A builder holding the inverse edits with the given origin.
    pub fn invert(&self, origin: TransactionOrigin) -> TransactionBuilder {
        self.inverse_edits()
            .into_iter()
            .fold(TransactionBuilder::new().with_origin(origin), TransactionBuilder::push)
    }

    /// Transform an anchor from `revision_before` coordinates to
    /// `revision_after` coordinates (docs/core-redesign.md §A.2).
    pub fn transform_anchor(&self, anchor: super::anchor::Anchor) -> super::anchor::Anchor {
        let edits: Vec<Edit> = self.edits.iter().map(|ce| ce.edit.clone()).collect();
        super::anchor::transform_anchor_through_edits(anchor, &edits)
    }
}
