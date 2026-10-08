//! Canonical document revision.
//!
//! Every committed transaction advances the revision exactly once; undo and
//! redo are commits like any other.  Revisions are never reused.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Revision(pub u64);

impl Revision {
    pub const fn first() -> Self {
        Revision(0)
    }

    pub const fn next(self) -> Self {
        Revision(self.0 + 1)
    }
}
