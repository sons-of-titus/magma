//! Text layout — the explicit pipeline
//! `byte offset → UTF-8 boundary → grapheme cluster → cell width → screen
//! position` (docs/core-redesign.md §10).
//!
//! Logical columns (char counts, `Buffer::position_of`) stay separate from
//! visual cell columns (this module).  Byte-offset logic is never
//! contaminated by layout logic: every function here takes `&str` slices.

use unicode_segmentation::UnicodeSegmentation;

/// Snap an offset down to the nearest grapheme-cluster start.
pub fn floor_grapheme(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() {
        return offset;
    }
    let mut last_start = 0usize;
    for (i, _) in text.grapheme_indices(true) {
        if i == offset {
            return offset;
        }
        if i > offset {
            break;
        }
        last_start = i;
    }
    last_start
}

/// Cursor-left step: the start of the cluster containing `offset`; when
/// already on a boundary, the start of the preceding cluster.
pub fn prev_grapheme(text: &str, offset: usize) -> usize {
    let containing = floor_grapheme(text, offset.min(text.len()));
    if containing < offset {
        return containing;
    }
    text[..containing]
        .grapheme_indices(true)
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(containing)
}

/// The start of the grapheme cluster after `offset` (cursor-right step).
pub fn next_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_grapheme(text, offset);
    match text[offset..].graphemes(true).next() {
        Some(g) => offset + g.len(),
        None => offset,
    }
}

/// Display width of one grapheme cluster in terminal cells.
/// Combining-only clusters are zero-width; a tab advances to its stop.
pub fn grapheme_cell_width(
    grapheme: &str,
    tab_width: usize,
    current_column: usize,
    glyph_widths: &std::collections::HashMap<char, u8>,
) -> usize {
    if grapheme == "\t" {
        return tab_width.saturating_sub(current_column % tab_width.max(1)).max(1);
    }
    grapheme
        .chars()
        .map(|c| {
            if let Some(&w) = glyph_widths.get(&c) {
                w as usize
            } else {
                unicode_width::UnicodeWidthChar::width(c).unwrap_or(0)
            }
        })
        .sum()
}

/// Visual cell column of a byte offset within its line slice
/// (`text` starts at the line start; `offset` is relative to it).
pub fn cell_column(
    text: &str,
    offset: usize,
    tab_width: usize,
    glyph_widths: &std::collections::HashMap<char, u8>,
) -> usize {
    let offset = floor_grapheme(text, offset);
    let mut col = 0usize;
    let mut byte = 0usize;
    for g in text.graphemes(true) {
        if byte >= offset {
            break;
        }
        col += grapheme_cell_width(g, tab_width, col, glyph_widths);
        byte += g.len();
    }
    col
}

/// Iterate grapheme clusters of a slice.
pub fn grapheme_iter(text: &str) -> impl Iterator<Item = &str> {
    text.graphemes(true)
}

/// Iterate grapheme clusters with their byte offsets.
pub fn grapheme_indices(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.grapheme_indices(true)
}

/// Total cell width of a line slice.
pub fn line_cells(
    text: &str,
    tab_width: usize,
    glyph_widths: &std::collections::HashMap<char, u8>,
) -> usize {
    let mut col = 0usize;
    for g in text.graphemes(true) {
        col += grapheme_cell_width(g, tab_width, col, glyph_widths);
    }
    col
}
