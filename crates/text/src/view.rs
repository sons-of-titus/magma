//! BufferView — per-window view state over a shared Buffer.
//!
//! Cursor movement after edits is anchor transformation, never ad-hoc span
//! arithmetic (docs/core-redesign.md §2.3/§3.3): the typing cursor behaves
//! as an `Affinity::After` anchor.  Position re-derivation uses the line
//! index — no prefix copies, no char scans.

use super::anchor::{Affinity, Anchor};
use super::transaction::EditTransaction;
use super::{Buffer, Decoration, Position};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct BufferView {
    /// Shared backing buffer content (multiple views can reference the same buffer).
    pub buffer: Arc<Mutex<Buffer>>,
    pub cursor: Position,
    pub scroll_offset: usize,
    /// Collapsed ranges: rendered as a single fold row.
    pub folds: Vec<(usize, usize)>,
    /// Foldable ranges reported by the language (tree-sitter fold query):
    /// what the gutter offers to fold, distinct from what is folded.
    pub fold_ranges: Vec<(usize, usize)>,
    pub highlights: Vec<(usize, usize, String)>,
    pub highlight_layers: HashMap<String, Vec<(usize, usize, String)>>,
    pub decoration_layers: HashMap<String, Vec<Decoration>>,
}

impl BufferView {
    pub fn new(buffer: Arc<Mutex<Buffer>>) -> Self {
        BufferView {
            buffer,
            cursor: Position::zero(),
            scroll_offset: 0,
            folds: Vec::new(),
            fold_ranges: Vec::new(),
            highlights: Vec::new(),
            highlight_layers: HashMap::new(),
            decoration_layers: HashMap::new(),
        }
    }

    // ── Cursor ──

    pub fn cursor_offset(&self) -> usize {
        self.cursor.offset
    }

    pub fn set_cursor(&mut self, offset: usize) {
        let pos = {
            let buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.position_of(offset)
        };
        self.cursor = pos;
    }

    /// Move the cursor through a committed transaction as a right-sticky
    /// anchor: it follows text inserted at it and collapses over deletions.
    fn cursor_through(&mut self, txn: &EditTransaction) {
        let anchor = txn.transform_anchor(Anchor {
            offset: self.cursor.offset,
            affinity: Affinity::After,
        });
        self.set_cursor(anchor.offset);
    }

    /// Map a pre-transaction byte range to post-transaction coordinates.
    /// Ranges entirely after an edit shift by the delta — the same anchor
    /// arithmetic the cursor uses.  A range the edit cuts through keeps its
    /// surviving part (grows over an insertion inside it, clamps over a
    /// deletion): dropping it would blank a word or a whole string literal
    /// between the keystroke and the next parse landing.
    fn transform_range(txn: &EditTransaction, range: (usize, usize)) -> Option<(usize, usize)> {
        let (mut s, mut e) = range;
        // Edits are ascending and disjoint (§A.3), so the first edit at or
        // after the range settles it.
        for ce in &txn.edits {
            let (start, end) = (ce.edit.range.start, ce.edit.range.end);
            if e <= start {
                break;
            }
            let delta = ce.edit.inserted_text.len() as i64 - (end - start) as i64;
            if s >= end {
                s = (s as i64 + delta).max(0) as usize;
                e = (e as i64 + delta).max(0) as usize;
            } else {
                // Overlap.  The parts of the span before and after the edit
                // survive; only the span the edit fully covers disappears.
                let left = s < start;
                let right = e > end;
                match (left, right) {
                    (false, false) => return None,
                    // Span truncated to the prefix before the edit — later
                    // edits are past it.
                    (true, false) => {
                        e = start;
                        break;
                    }
                    // Span reduced to its tail after the edit.
                    (false, true) => {
                        s = (end as i64 + delta).max(0) as usize;
                        e = (e as i64 + delta).max(0) as usize;
                    }
                    // Span bridges the edit: it stretches or shrinks by the
                    // delta and keeps its head.
                    (true, true) => {
                        e = (e as i64 + delta).max(0) as usize;
                    }
                }
            }
        }
        Some((s.min(e), e.max(s)))
    }

    /// Fold spans follow edits exactly like highlights: an edit cutting
    /// through a fold opens it (dropping the span beats folding half a
    /// function), and spans past the edit shift by the delta.  The fold
    /// start can land mid-line after an edit on its line; the next
    /// tree-sitter recompute re-snaps it, so no extra handling here.
    fn transform_annotations(&mut self, txn: &EditTransaction) {
        let map = |v: &Vec<(usize, usize, String)>| -> Vec<(usize, usize, String)> {
            v.iter()
                .filter_map(|(a, b, f)| {
                    Self::transform_range(txn, (*a, *b)).map(|(a, b)| (a, b, f.clone()))
                })
                .collect()
        };
        let map_range = |v: &Vec<(usize, usize)>| -> Vec<(usize, usize)> {
            v.iter()
                .filter_map(|&(a, b)| Self::transform_range(txn, (a, b)))
                .collect()
        };
        self.highlights = map(&self.highlights);
        for layer in self.highlight_layers.values_mut() {
            *layer = map(layer);
        }
        self.folds = map_range(&self.folds);
        self.fold_ranges = map_range(&self.fold_ranges);
    }

    // ── Content mutations (delegate to Buffer, cursor transforms) ──

    pub fn insert(&mut self, offset: usize, text: &str) {
        let cursor_before = self.cursor.offset;
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            if buf.read_only {
                return;
            }
            buf.insert_with_cursor(offset, text, cursor_before)
        };
        if let Some(txn) = txn {
            self.cursor_through(&txn);
            self.transform_annotations(&txn);
        }
    }

    /// Insert at several positions in one transaction (multi-cursor).
    /// Returns the transaction so callers can transform extra cursors.
    pub fn insert_many(&mut self, edits: &[(usize, &str)]) -> Option<EditTransaction> {
        let cursor_before = self.cursor.offset;
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            if buf.read_only {
                return None;
            }
            buf.insert_many(edits, cursor_before)
        };
        if let Some(txn) = &txn {
            self.cursor_through(txn);
            self.transform_annotations(txn);
        }
        txn
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        let cursor_before = self.cursor.offset;
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.delete_with_cursor(start, end, cursor_before)
        };
        if let Some(txn) = txn {
            self.cursor_through(&txn);
            self.transform_annotations(&txn);
        }
    }

    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        let cursor_before = self.cursor.offset;
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.replace_with_cursor(start, end, text, cursor_before)
        };
        if let Some(txn) = txn {
            self.cursor_through(&txn);
            self.transform_annotations(&txn);
        }
    }

    pub fn undo(&mut self) -> bool {
        let restored = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.undo_with_txns()
        };
        match restored {
            Some((offset, txns)) => {
                for txn in &txns {
                    self.transform_annotations(txn);
                }
                self.set_cursor(offset);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        let restored = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.redo_with_txns()
        };
        match restored {
            Some((offset, txns)) => {
                for txn in &txns {
                    self.transform_annotations(txn);
                }
                self.set_cursor(offset);
                true
            }
            None => false,
        }
    }

    pub fn open_undo_session(&mut self) {
        self.buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .open_undo_session_at(self.cursor.offset);
    }

    pub fn close_undo_session(&mut self) {
        self.buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .close_undo_session();
    }

    // ── Folds ──

    pub fn add_fold(&mut self, start: usize, end: usize) {
        self.folds.push((start, end));
        self.folds.sort_by_key(|a| a.0);
    }

    pub fn remove_fold(&mut self, start: usize, end: usize) {
        self.folds.retain(|f| f.0 != start || f.1 != end);
    }

    pub fn clear_folds(&mut self) {
        self.folds.clear();
    }

    pub fn is_folded(&self, line_start: usize, line_end: usize) -> bool {
        self.folds
            .iter()
            .any(|(fs, fe)| *fs <= line_start && *fe >= line_end)
    }

    pub fn set_fold_ranges(&mut self, ranges: Vec<(usize, usize)>) {
        self.fold_ranges = ranges;
    }

    // ── Highlights ──

    pub fn set_highlights(&mut self, ranges: Vec<(usize, usize, String)>) {
        self.highlights = ranges;
    }

    pub fn clear_highlights(&mut self) {
        self.highlights.clear();
    }

    pub fn set_highlights_layer(&mut self, layer: &str, ranges: Vec<(usize, usize, String)>) {
        self.highlight_layers.insert(layer.to_string(), ranges);
    }

    pub fn clear_highlights_layer(&mut self, layer: &str) {
        self.highlight_layers.remove(layer);
    }

    pub fn clear_all_highlight_layers(&mut self) {
        self.highlight_layers.clear();
    }

    // ── Decorations ──

    pub fn decor_set_inline(
        &mut self,
        layer: &str,
        line: usize,
        col: usize,
        text: String,
        face: String,
    ) {
        let v = self.decoration_layers.entry(layer.to_string()).or_default();
        v.retain(|d| !matches!(d, Decoration::InlineText { line: l, col: c, .. } if *l == line && *c == col));
        v.push(Decoration::InlineText {
            line,
            col,
            text,
            face,
        });
    }

    pub fn decor_set_eol(&mut self, layer: &str, line: usize, text: String, face: String) {
        let v = self.decoration_layers.entry(layer.to_string()).or_default();
        v.retain(|d| !matches!(d, Decoration::EndOfLine { line: l, .. } if *l == line));
        v.push(Decoration::EndOfLine { line, text, face });
    }

    pub fn decor_set_prefix(&mut self, layer: &str, line: usize, text: String, face: String) {
        let v = self.decoration_layers.entry(layer.to_string()).or_default();
        v.retain(|d| !matches!(d, Decoration::LinePrefix { line: l, .. } if *l == line));
        v.push(Decoration::LinePrefix { line, text, face });
    }

    pub fn decor_clear_layer(&mut self, layer: &str) {
        self.decoration_layers.remove(layer);
    }

    pub fn decor_clear(&mut self) {
        self.decoration_layers.clear();
    }

    pub fn decor_count_layer(&self, layer: &str) -> usize {
        self.decoration_layers
            .get(layer)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}
