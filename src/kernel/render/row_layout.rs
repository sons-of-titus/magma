//! Row layout cache (R-12) — the grapheme→cell mapping of a visible row,
//! computed once per (buffer, line, revision, tab width) and reused by the
//! text pass and every highlight walk.  Before the cache, each repaint
//! re-segmented the same rows once for painting and once per highlight
//! layer; the cache bounds that work to one pass per actual text change.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::kernel::state::Editor;
use crate::kernel::text_engine::text_layout::grapheme_cell_width;

/// One row's layout: parallel vectors over its grapheme clusters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RowLayout {
    /// Buffer revision the layout was computed at.
    pub revision: u64,
    /// Byte length of the laid-out line slice — a cheap extra guard.
    pub line_len: usize,
    pub tab_width: usize,
    /// Cell width of each grapheme.
    pub widths: Vec<u8>,
    /// Lead char of each grapheme (painted on the first cell).
    pub leads: Vec<char>,
    /// Byte offset of each grapheme relative to the line start.
    pub offsets: Vec<u32>,
    /// Visual cell column where each grapheme starts (cumulative widths).
    pub cols: Vec<u32>,
}

impl RowLayout {
    /// Lay out `line_text` (a slice starting at the line start).
    pub fn build(
        line_text: &str,
        revision: u64,
        tab_width: usize,
        glyph_widths: &std::collections::HashMap<char, u8>,
    ) -> RowLayout {
        let mut layout = RowLayout {
            revision,
            line_len: line_text.len(),
            tab_width,
            ..Default::default()
        };
        let mut col = 0usize;
        for (offset, g) in crate::kernel::text_engine::text_layout::grapheme_indices(line_text) {
            let w = grapheme_cell_width(g, tab_width, col, glyph_widths);
            layout.cols.push(col as u32);
            layout.widths.push(w.min(u8::MAX as usize) as u8);
            layout.leads.push(g.chars().next().unwrap_or(' '));
            layout.offsets.push(offset as u32);
            col += w;
        }
        layout
    }

    /// Total cells the row occupies.
    pub fn cells(&self) -> usize {
        self.widths.iter().map(|&w| w as usize).sum()
    }
}

pub type RowKey = (usize, usize, usize);

/// The Editor-owned cache.  Interior-mutable because the render path holds
/// `&Editor`; the map lock is uncontended (one foreground owner) and held
/// only for a lookup.  The cap keeps fold-heavy views bounded — each entry
/// is a viewport row, so scrolling churn evicts by overwrite.
#[derive(Default)]
pub struct RowLayoutCache {
    map: Mutex<HashMap<RowKey, Arc<RowLayout>>>,
}

impl RowLayoutCache {
    pub fn get_or_build(
        &self,
        key: RowKey,
        line_text: &str,
        revision: u64,
        tab_width: usize,
        glyph_widths: &std::collections::HashMap<char, u8>,
    ) -> Arc<RowLayout> {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = map.get(&key) {
            if cached.revision == revision
                && cached.line_len == line_text.len()
                && cached.tab_width == tab_width {
                    return cached.clone();
                }
        }
        let layout = Arc::new(RowLayout::build(line_text, revision, tab_width, glyph_widths));
        if map.len() >= 8_192 {
            // Dropping arbitrary entries is safe — the next frame rebuilds
            // what it paints.
            let keys: Vec<RowKey> = map.keys().take(64).copied().collect();
            for k in keys {
                map.remove(&k);
            }
        }
        map.insert(key, layout.clone());
        layout
    }

    /// Drop every entry — called when glyph metrics change (font reload).
    pub fn clear(&self) {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// Cached layout for one visible row of buffer `buf_key` whose text starts
/// at `line_start`.  Callers pass the revision from the buffer lock they
/// already hold — re-locking here would deadlock every render.
pub fn row_layout(
    editor: &Editor,
    buf_key: usize,
    line_start: usize,
    line_text: &str,
    revision: u64,
    tab_width: usize,
) -> Arc<RowLayout> {
    editor.row_layouts.get_or_build(
        (buf_key, line_start, tab_width),
        line_text,
        revision,
        tab_width,
        &editor.font_config.glyph_widths,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widths_map() -> std::collections::HashMap<char, u8> {
        std::collections::HashMap::new()
    }

    #[test]
    fn layout_tables_are_parallel_and_align() {
        let layout = RowLayout::build("aé\tb", 7, 4, &widths_map());
        assert_eq!(layout.revision, 7);
        assert_eq!(layout.line_len, "aé\tb".len());
        assert_eq!(layout.leads, vec!['a', 'é', '\t', 'b']);
        // Offsets are byte offsets: é is two bytes.
        assert_eq!(layout.offsets, vec![0, 1, 3, 4]);
        // The tab sits at column 2, so it advances to stop 4 (width 2).
        assert_eq!(layout.widths, vec![1, 1, 2, 1]);
        assert_eq!(layout.cols, vec![0, 1, 2, 4]);
        assert_eq!(layout.cells(), 5);
    }

    #[test]
    fn wide_glyphs_take_two_cells() {
        let layout = RowLayout::build("a望b", 1, 4, &widths_map());
        assert_eq!(layout.widths, vec![1, 2, 1]);
        assert_eq!(layout.cols, vec![0, 1, 3]);
        assert_eq!(layout.cells(), 4);
    }

    #[test]
    fn cache_reuses_layout_until_revision_or_text_changes() {
        let cache = RowLayoutCache::default();
        let key = (1usize, 0usize, 4usize);
        let text = "hello";
        let first = cache.get_or_build(key, text, 1, 4, &widths_map());
        let second = cache.get_or_build(key, text, 1, 4, &widths_map());
        assert!(Arc::ptr_eq(&first, &second), "same revision reuses the entry");

        let bumped = cache.get_or_build(key, text, 2, 4, &widths_map());
        assert!(!Arc::ptr_eq(&first, &bumped), "a new revision relays out");

        // Same revision, different text length (identity change) — rebuild.
        let longer = cache.get_or_build(key, "hello!!", 2, 4, &widths_map());
        assert!(longer.line_len == 7);
    }

    #[test]
    fn cache_clear_drops_everything() {
        let cache = RowLayoutCache::default();
        let a = cache.get_or_build((1, 0, 4), "x", 1, 4, &widths_map());
        cache.clear();
        let b = cache.get_or_build((1, 0, 4), "x", 1, 4, &widths_map());
        assert!(!Arc::ptr_eq(&a, &b), "clear forces a rebuild");
    }
}
