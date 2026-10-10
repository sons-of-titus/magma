/// Completion popup state.
#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    pub items: Vec<String>,
    pub idx: usize,
    pub visible: bool,
    pub prefix: String,
    /// Per-buffer word sets for buffer-word completion, each tagged with the
    /// document revision it was scanned at.  Big buffers rebuild off-thread
    /// (the `buffer-words` background event lands the result).
    pub word_cache: std::collections::HashMap<usize, WordCache>,
    /// The (buffer, revision) pair a background word scan is in flight for —
    /// coalesces per-keystroke requests into one scan per revision.
    pub words_requested: Option<(usize, u64)>,
}

#[derive(Debug, Clone)]
pub struct WordCache {
    pub revision: u64,
    pub words: std::sync::Arc<std::collections::BTreeSet<String>>,
}
