/// Visual block insert/append state.
#[derive(Debug, Clone, Default)]
pub struct BlockVisualState {
    pub active: bool,
    pub lines: Vec<usize>,
    pub col: usize,
    pub was_append: bool,
    pub pre_pos: usize,
}
