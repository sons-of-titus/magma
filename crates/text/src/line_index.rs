//! Line index — the single owner of newline structure.
//!
//! Implemented over ropey's internally-maintained line tree (O(log n),
//! updated incrementally by construction) rather than a second kernel-side
//! newline structure; duplicating it would recreate exactly the redundant
//! derivation this redesign deletes (docs/core-redesign.md §A.5).
//!
//! Semantics: lines are `\n`-delimited; `\r` belongs to line content; a
//! trailing `\n` begins a final empty line.  All queries are O(log n) in
//! document size — no interactive-path operation scans a prefix.

use super::edit::{ByteOffset, ByteRange};
use super::rope::Rope;

pub struct LineIndex<'a> {
    rope: &'a Rope,
}

impl<'a> LineIndex<'a> {
    pub fn new(rope: &'a Rope) -> Self {
        LineIndex { rope }
    }

    /// Number of lines; an empty document has one line.
    pub fn line_count(&self) -> usize {
        self.rope.line_count()
    }

    /// Index of the last line (`line_count() - 1`).
    pub fn last_line(&self) -> usize {
        self.line_count() - 1
    }

    /// Line containing the character at `byte_offset` (the byte at a `\n`
    /// belongs to the line it terminates).  Clamped to the document.
    pub fn line_of(&self, byte_offset: ByteOffset) -> usize {
        self.rope.line_of(byte_offset)
    }

    /// Offset of the first byte of `line`, or `None` past the last line.
    pub fn line_start(&self, line: usize) -> Option<ByteOffset> {
        self.rope.line_start_offset(line)
    }

    /// Offset one past the last content byte of `line` (the position of its
    /// `\n`, or document end for the final line), or `None` past the last line.
    pub fn line_end(&self, line: usize) -> Option<ByteOffset> {
        if line >= self.line_count() {
            return None;
        }
        match self.rope.line_start_offset(line + 1) {
            Some(next_start) => Some(next_start - 1),
            None => Some(self.rope.len()),
        }
    }

    /// Content range of `line`, excluding its `\n`.
    pub fn line_span(&self, line: usize) -> Option<ByteRange> {
        Some(self.line_start(line)?..self.line_end(line)?)
    }
}
