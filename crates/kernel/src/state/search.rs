use crate::task::{QuickfixEntry, QUICKFIX_MAX_RESULTS};

/// Search state shared by the find dialog, the repeat-search commands, and
/// the async grep pipeline.  First slice of the R-24 Editor consolidation:
/// related fields move behind one named sub-struct.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Pattern of the last search; `None` until a search runs.  Set by the
    /// GUI find dialog and the word-under-cursor commands.
    pub pattern: Option<String>,
    /// Direction the last search ran in (`n`/`N` semantics).
    pub forward: bool,
    /// Search generation: each new find-in-files supersedes the last, and
    /// results from stale generations are dropped on arrival.
    pub generation: u64,
    /// Find-in-files / grep hits (`--vimgrep`), plus keyword-context output.
    /// Their own list, so a build and a grep no longer overwrite each other
    /// and the Find panel shows only these.
    pub results: Vec<QuickfixEntry>,
    /// Cursor into `results` for find-result stepping.
    pub index: usize,
}

impl SearchState {
    /// Record a pattern + direction from an interactive search.
    pub fn set(&mut self, pattern: impl Into<String>, forward: bool) {
        self.pattern = Some(pattern.into());
        self.forward = forward;
    }

    /// Replace the find results (truncated to the cap) and reset the cursor.
    pub fn set_results(&mut self, results: Vec<QuickfixEntry>) {
        self.results = results;
        self.results.truncate(QUICKFIX_MAX_RESULTS);
        self.index = 0;
    }
}
