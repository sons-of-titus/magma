//! Primitive edit — one normalized representation for insert, delete and
//! replace: a byte range that is replaced by new text.  An empty range with
//! text is an insert; a range with empty text is a delete.

use std::ops::Range;

pub type ByteOffset = usize;
pub type ByteRange = Range<usize>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Bytes removed from the pre-edit text (may be empty).
    pub range: ByteRange,
    /// Text inserted in place of `range` (may be empty).
    pub inserted_text: String,
}

impl Edit {
    pub fn insert(offset: ByteOffset, text: &str) -> Self {
        Edit {
            range: offset..offset,
            inserted_text: text.to_string(),
        }
    }

    pub fn delete(range: ByteRange) -> Self {
        Edit {
            range,
            inserted_text: String::new(),
        }
    }

    pub fn replace(range: ByteRange, text: &str) -> Self {
        Edit {
            range,
            inserted_text: text.to_string(),
        }
    }

    pub fn start(&self) -> ByteOffset {
        self.range.start
    }

    pub fn end(&self) -> ByteOffset {
        self.range.end
    }

    pub fn inserted_len(&self) -> usize {
        self.inserted_text.len()
    }

    /// Where the inserted text lives immediately after applying this edit
    /// *in isolation*.  Inside a multi-edit transaction the actual position
    /// is shifted by the preceding edits' deltas — see
    /// `EditTransaction::inverse_edits`.
    pub fn new_range(&self) -> ByteRange {
        self.start()..self.start() + self.inserted_len()
    }

    pub fn is_noop(&self) -> bool {
        self.range.start == self.range.end && self.inserted_text.is_empty()
    }

    pub fn is_insert_like(&self) -> bool {
        self.range.start == self.range.end && !self.inserted_text.is_empty()
    }

    pub fn is_delete_like(&self) -> bool {
        self.range.start < self.range.end && self.inserted_text.is_empty()
    }
}
