//! Row layout: grapheme→cell mapping of a visible row.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use text::text_layout::{grapheme_cell_width, grapheme_indices};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RowLayout {
    pub revision: u64,
    pub line_len: usize,
    pub tab_width: usize,
    pub widths: Vec<u8>,
    pub leads: Vec<char>,
    pub offsets: Vec<u32>,
    pub cols: Vec<u32>,
    pub is_bidi: bool,
}

impl RowLayout {
    pub fn build(
        line_text: &str,
        revision: u64,
        tab_width: usize,
        glyph_widths: &HashMap<char, u8>,
    ) -> RowLayout {
        let mut layout = RowLayout {
            revision,
            line_len: line_text.len(),
            tab_width,
            ..Default::default()
        };
        let mut col = 0usize;
        for (offset, g) in grapheme_indices(line_text) {
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

    fn apply_bidi(&mut self, line_text: &str) {
        let bidi_info = unicode_bidi::BidiInfo::new(line_text, None);
        let has_rtl = bidi_info.levels.iter().any(|l| l.is_rtl());
        if !has_rtl || bidi_info.paragraphs.is_empty() {
            return;
        }
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
        for gi in 0..self.widths.len() {
            if !visual.contains(&gi) {
                visual.push(gi);
            }
        }
        let widths = visual.iter().map(|&gi| self.widths[gi]).collect();
        let leads = visual.iter().map(|&gi| self.leads[gi]).collect();
        let offsets = visual.iter().map(|&gi| self.offsets[gi]).collect();
        self.widths = widths;
        self.leads = leads;
        self.offsets = offsets;
        self.cols.clear();
        let mut col = 0usize;
        for w in &self.widths {
            self.cols.push(col as u32);
            col += *w as usize;
        }
        self.is_bidi = true;
    }

    pub fn visual_col_of(&self, rel_byte: usize) -> usize {
        for (i, o) in self.offsets.iter().enumerate() {
            if *o as usize == rel_byte {
                return self.cols[i] as usize;
            }
        }
        self.cols.last().map(|c| *c as usize + self.widths.last().map(|&w| w as usize).unwrap_or(0)).unwrap_or(0)
    }

    pub fn bidi(&self) -> bool {
        self.is_bidi
    }

    pub fn cells(&self) -> usize {
        self.widths.iter().map(|&w| w as usize).sum()
    }
}

pub type RowKey = (usize, usize, usize);

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
        glyph_widths: &HashMap<char, u8>,
    ) -> Arc<RowLayout> {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = map.get(&key)
            && cached.revision == revision
                && cached.line_len == line_text.len()
                && cached.tab_width == tab_width {
                    return cached.clone();
                }
        let layout = Arc::new(RowLayout::build(line_text, revision, tab_width, glyph_widths));
        if map.len() >= 8_192 {
            let keys: Vec<RowKey> = map.keys().take(64).copied().collect();
            for k in keys {
                map.remove(&k);
            }
        }
        map.insert(key, layout.clone());
        layout
    }

    pub fn clear(&self) {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn widths_map() -> HashMap<char, u8> {
        HashMap::new()
    }

    #[test]
    fn layout_tables_are_parallel_and_align() {
        let layout = RowLayout::build("aé\tb", 7, 4, &widths_map());
        assert_eq!(layout.revision, 7);
        assert_eq!(layout.line_len, "aé\tb".len());
        assert_eq!(layout.leads, vec!['a', 'é', '\t', 'b']);
        assert_eq!(layout.offsets, vec![0, 1, 3, 4]);
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

    fn widths_map() -> HashMap<char, u8> {
        HashMap::new()
    }

    #[test]
    fn ltr_lines_are_untouched_by_bidi() {
        let ltr = RowLayout::build("hello world", 1, 4, &widths_map());
        assert!(!ltr.bidi());
        assert_eq!(ltr.leads, vec!['h', 'e', 'l', 'l', 'o', ' ', 'w', 'o', 'r', 'l', 'd']);
        let offs: Vec<usize> = ltr.offsets.iter().map(|&o| o as usize).collect();
        let mut sorted = offs.clone();
        sorted.sort();
        assert_eq!(offs, sorted);
    }

    #[test]
    fn pure_rtl_line_paints_in_reversed_visual_order() {
        let rtl = RowLayout::build("\u{05D0}\u{05D1}\u{05D2}", 1, 4, &widths_map());
        assert!(rtl.bidi());
        assert_eq!(rtl.leads, vec!['\u{05D2}', '\u{05D1}', '\u{05D0}']);
        assert_eq!(rtl.offsets, vec![4, 2, 0]);
    }

    #[test]
    fn mixed_ltr_rtl_keeps_ltr_runs_in_order() {
        let mixed = RowLayout::build("abc \u{05D0}\u{05D1}\u{05D2}", 1, 4, &widths_map());
        assert!(mixed.bidi());
        assert_eq!(mixed.leads, vec!['a', 'b', 'c', ' ', '\u{05D2}', '\u{05D1}', '\u{05D0}']);
        assert_eq!(mixed.visual_col_of(4), 6);
        assert_eq!(mixed.visual_col_of(8), 4);
    }

    #[test]
    fn cursor_past_the_last_grapheme_maps_to_the_row_end() {
        let rtl = RowLayout::build("\u{05D0}\u{05D1}\u{05D2}", 1, 4, &widths_map());
        assert_eq!(rtl.visual_col_of(6), 3);
    }
}