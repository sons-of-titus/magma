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
    /// The row required bidi reordering (F-15).
    pub is_bidi: bool,
}

impl RowLayout {
    /// Lay out `line_text` (a slice starting at the line start).  Lines
    /// containing RTL runs (F-15) are reordered into visual order here —
    /// the storage stays logical; every consumer of the layout paints the
    /// visual sequence.
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
        layout.apply_bidi(line_text);
        layout
    }

    /// Reorder the layout tables into visual order when the line contains
    /// RTL text (the bidi L2 rule: reverse each RTL run).  Logical storage
    /// is untouched — `offsets` map visual positions back to their logical
    /// bytes, so highlight matching and cursor mapping keep working.
    fn apply_bidi(&mut self, line_text: &str) {
        let bidi_info = unicode_bidi::BidiInfo::new(line_text, None);
        let has_rtl = bidi_info.levels.iter().any(|l| l.is_rtl());
        if !has_rtl || bidi_info.paragraphs.is_empty() {
            return;
        }
        // Visual order of the char-index runs across the whole paragraph.
        let para = &bidi_info.paragraphs[0];
        let (levels, runs) = bidi_info.visual_runs(para, para.range.clone());
        let mut visual: Vec<usize> = Vec::with_capacity(self.widths.len());
        for run in runs {
            let (start, end) = (run.start, run.end);
            let run_graphemes: Vec<usize> = (0..self.offsets.len())
                .filter(|&gi| {
                    let o = self.offsets[gi] as usize;
                    o >= start && o < end
                })
                .collect();
            if levels[start].is_rtl() {
                visual.extend(run_graphemes.into_iter().rev());
            } else {
                visual.extend(run_graphemes);
            }
        }
        // Graphemes outside every run (defensive): append logically.
        for gi in 0..self.widths.len() {
            if !visual.contains(&gi) {
                visual.push(gi);
            }
        }
        let widths = visual.iter().map(|&gi| self.widths[gi]).collect();
        let leads  = visual.iter().map(|&gi| self.leads[gi]).collect();
        let offsets = visual.iter().map(|&gi| self.offsets[gi]).collect();
        self.widths = widths;
        self.leads = leads;
        self.offsets = offsets;
        // Recompute visual columns from the reordered widths.
        self.cols.clear();
        let mut col = 0usize;
        for w in &self.widths {
            self.cols.push(col as u32);
            col += *w as usize;
        }
        self.is_bidi = true;
    }

    /// Visual cell column of the grapheme starting at `rel_byte` within the
    /// line — the cursor mapping for bidi rows (logical byte → screen
    /// column).  Falls back to a linear scan for LTR rows' callers.
    pub fn visual_col_of(&self, rel_byte: usize) -> usize {
        for (i, o) in self.offsets.iter().enumerate() {
            if *o as usize == rel_byte {
                return self.cols[i] as usize;
            }
        }
        // Past the last grapheme (cursor on the line's end): total cells.
        self.cols.last().map(|c| *c as usize + self.widths.last().map(|&w| w as usize).unwrap_or(0)).unwrap_or(0)
    }

    /// True when the row required bidi reordering.
    pub fn bidi(&self) -> bool { self.is_bidi }

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

#[cfg(test)]
mod bidi_tests {
    use super::*;

    fn widths_map() -> std::collections::HashMap<char, u8> {
        std::collections::HashMap::new()
    }

    #[test]
    fn ltr_lines_are_untouched_by_bidi() {
        let ltr = RowLayout::build("hello world", 1, 4, &widths_map());
        assert!(!ltr.bidi());
        assert_eq!(ltr.leads, vec!['h', 'e', 'l', 'l', 'o', ' ', 'w', 'o', 'r', 'l', 'd']);
        // Monotonic logical offsets survive.
        let offs: Vec<usize> = ltr.offsets.iter().map(|&o| o as usize).collect();
        let mut sorted = offs.clone();
        sorted.sort();
        assert_eq!(offs, sorted);
    }

    #[test]
    fn pure_rtl_line_paints_in_reversed_visual_order() {
        // אבג — logical order alef, bet, gimel; visual order is reversed.
        let rtl = RowLayout::build("\u{05D0}\u{05D1}\u{05D2}", 1, 4, &widths_map());
        assert!(rtl.bidi());
        assert_eq!(rtl.leads, vec!['\u{05D2}', '\u{05D1}', '\u{05D0}'],
            "the last logical glyph paints leftmost");
        // offsets map each visual cell back to its logical byte.
        assert_eq!(rtl.offsets, vec![4, 2, 0]);
    }

    #[test]
    fn mixed_ltr_rtl_keeps_ltr_runs_in_order() {
        // "abc אבג" — LTR prefix stays put; the RTL run reverses within.
        let mixed = RowLayout::build("abc \u{05D0}\u{05D1}\u{05D2}", 1, 4, &widths_map());
        assert!(mixed.bidi());
        assert_eq!(mixed.leads, vec!['a', 'b', 'c', ' ', '\u{05D2}', '\u{05D1}', '\u{05D0}']);
        // The RTL run paints reversed: gimel leftmost, alef rightmost.
        // Cursor on alef (logical byte 4) → visual column 6.
        assert_eq!(mixed.visual_col_of(4), 6);
        // Cursor on gimel (logical byte 8) → visual column 4.
        assert_eq!(mixed.visual_col_of(8), 4);
    }

    #[test]
    fn cursor_past_the_last_grapheme_maps_to_the_row_end() {
        let rtl = RowLayout::build("\u{05D0}\u{05D1}\u{05D2}", 1, 4, &widths_map());
        assert_eq!(rtl.visual_col_of(6), 3, "end-of-line cursor lands after the row");
    }
}
