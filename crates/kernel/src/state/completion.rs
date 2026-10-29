/// One completion candidate.  `label` is what the popup shows; `insert` is
/// what accept types (LSP `textEdit.newText`/`insertText` when the server
/// provides one, the label otherwise); `kind`/`detail`/`docs` are display
/// metadata — `detail` carries the signature line servers report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: String,
    pub detail: String,
    pub insert: String,
    /// Text the typed prefix filters against (LSP `filterText`); empty
    /// means the label.
    pub filter: String,
    /// Server documentation, shown when the item has no signature line.
    pub docs: String,
    /// `insert` is LSP snippet syntax; accept expands it into tab stops.
    pub is_snippet: bool,
}

impl CompletionItem {
    /// A plain word/symbol candidate: everything is the label.
    pub fn word(label: impl Into<String>) -> Self {
        let label = label.into();
        let insert = label.clone();
        CompletionItem {
            label,
            insert,
            ..Default::default()
        }
    }
}

/// Completion popup state.
#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    pub items: Vec<CompletionItem>,
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
