/// Sampled lines for one buffer, valid for the recorded revision and strip
/// size.  The per-frame work is then only painting, not document reads
/// (rework.md §7).
#[derive(Debug, Clone, Default)]
pub struct MinimapCache {
    pub revision: u64,
    pub total: usize,
    pub max_rows: usize,
    /// `(document line index, line text)` for sampled lines, ascending.
    pub lines: Vec<(usize, String)>,
}
