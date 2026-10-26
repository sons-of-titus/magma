//! Editor-side model for the side-by-side diff view.
//!
//! The repository operations that produce these values live in the `vcs`
//! crate; the types here describe editor layout, so they reference window IDs
//! and layout constraints and stay in the kernel.

/// The three pieces the side-by-side editor diff renders.
#[derive(Debug, Clone, Default)]
pub struct DiffData {
    /// File content at HEAD — the left pane.
    pub head: String,
    /// Working-tree content — the right pane.
    pub work: String,
    /// Zero-context diff; its hunks drive per-hunk stage/discard.
    pub patch: String,
}

/// One pane the diff view concealed while open — enough to put the user's
/// editor layout back exactly as it was.
#[derive(Debug, Clone)]
pub struct HiddenPane {
    pub id: crate::kernel::state::id::WindowId,
    pub buffer_id: Option<usize>,
    pub layout: crate::kernel::render::view_tree::LayoutConstraint,
    pub weight: f32,
}

/// The side-by-side editor diff: two linked panes that stand in for the whole
/// editor layout while open.  The two halves read as a single tab
/// (`{file}:HEAD->TREE`); `hidden` carries the panes they displaced.
#[derive(Debug, Clone)]
pub struct DiffView {
    pub left: crate::kernel::state::id::WindowId,
    pub right: crate::kernel::state::id::WindowId,
    pub left_buf: usize,
    pub right_buf: usize,
    /// Repository-relative path the two panes show.
    pub path: String,
    /// Zero-context patch the view was built from — the hunk source.
    pub patch: String,
    /// Every pane the diff concealed, with its buffer, layout and weight.
    pub hidden: Vec<HiddenPane>,
}
