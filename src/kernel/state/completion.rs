/// Completion popup state.
#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    pub items: Vec<String>,
    pub idx: usize,
    pub visible: bool,
    pub prefix: String,
}
