//! Anchors — stable positions carried across edits, with explicit boundary
//! behavior.  Pure functions: no buffer access.
//!
//! Contract (docs/core-redesign.md §A.2) for an edit deleting `[s, e)` and
//! inserting `L` bytes at `s`:
//!
//! ```text
//! anchor.offset < s         → unchanged
//! s ≤ anchor.offset ≤ e     → Before: s          After: s + L
//! anchor.offset > e         → offset + L − (e − s)
//! ```
//!
//! Undo/redo need no special cases: they are ordinary inverse transactions.

use super::edit::{ByteOffset, Edit};

/// Which side of its gap an anchor hugs when an edit lands exactly on it.
/// `Before` is left-sticky (stays left of inserted text, collapses to the
/// left edge of a replacement); `After` is right-sticky.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Affinity {
    Before,
    After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Anchor {
    pub offset: ByteOffset,
    pub affinity: Affinity,
}

impl Anchor {
    pub fn before(offset: ByteOffset) -> Self {
        Anchor { offset, affinity: Affinity::Before }
    }

    pub fn after(offset: ByteOffset) -> Self {
        Anchor { offset, affinity: Affinity::After }
    }
}

pub fn transform_anchor(anchor: Anchor, edit: &Edit) -> Anchor {
    let (s, e, len) = (edit.start(), edit.end(), edit.inserted_len());
    let offset = if anchor.offset < s {
        anchor.offset
    } else if anchor.offset <= e {
        match anchor.affinity {
            Affinity::Before => s,
            Affinity::After => s + len,
        }
    } else {
        anchor.offset + len - (e - s)
    };
    Anchor { offset, affinity: anchor.affinity }
}

/// Transform through a *normalized* transaction edit list: edits sorted by
/// start, disjoint, strictly non-abutting (see `transaction` normalization).
/// All edits are in the same (pre-transaction) coordinates, so at most one
/// closed range `[s, e]` can contain the anchor.
pub fn transform_anchor_through_edits(anchor: Anchor, edits: &[Edit]) -> Anchor {
    let mut delta: isize = 0;
    for edit in edits {
        let (s, e, len) = (edit.start(), edit.end(), edit.inserted_len());
        if anchor.offset < s {
            break;
        }
        if anchor.offset <= e {
            let offset = match anchor.affinity {
                Affinity::Before => s as isize + delta,
                Affinity::After => s as isize + delta + len as isize,
            };
            return Anchor { offset: offset.max(0) as usize, affinity: anchor.affinity };
        }
        delta += len as isize - (e - s) as isize;
    }
    Anchor {
        offset: (anchor.offset as isize + delta).max(0) as usize,
        affinity: anchor.affinity,
    }
}
