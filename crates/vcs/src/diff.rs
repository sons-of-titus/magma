//! The diff bundle the editor's side-by-side change view renders.
//!
//! The backend operations that produce it live in [`crate::remote`]; the
//! editor decides how to lay the two panes out.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_data_defaults_to_empty_panes() {
        let d = DiffData::default();
        assert!(d.head.is_empty());
        assert!(d.work.is_empty());
        assert!(d.patch.is_empty());
    }

    #[test]
    fn diff_data_clones_its_three_parts() {
        let d = DiffData {
            head: "old".into(),
            work: "new".into(),
            patch: "@@ -1 +1 @@".into(),
        };
        let copy = d.clone();
        assert_eq!(copy.head, "old");
        assert_eq!(copy.work, "new");
        assert_eq!(copy.patch, "@@ -1 +1 @@");
    }
}
