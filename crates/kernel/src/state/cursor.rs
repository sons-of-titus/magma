/// An extra cursor for multi-cursor editing.
#[derive(Debug, Clone, Copy)]
pub struct ExtraCursor {
    pub pos: usize,
    pub anchor: Option<usize>,
}

/// Multi-cursor editing state.
#[derive(Debug, Clone, Default)]
pub struct MultiCursorState {
    pub extra_cursors: Vec<ExtraCursor>,
    pub active: bool,
}
