/// Snippet expansion state.
#[derive(Debug, Clone, Default)]
pub struct SnippetState {
    pub active: bool,
    pub tabstops: Vec<crate::snippet::TabStop>,
    pub tabstop_idx: usize,
}
