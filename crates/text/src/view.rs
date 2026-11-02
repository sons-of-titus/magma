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
    /// Last transaction sequence this fork replayed from the shared
    /// buffer's tail — split-view forks stay coherent through it.
    pub synced_txn_seq: u64,
}

impl BufferView {
    pub fn new(buffer: Arc<Mutex<Buffer>>) -> Self {
        let synced = buffer.lock().unwrap_or_else(|e| e.into_inner()).txn_seq();
        BufferView {
            buffer,
            cursor: Position::zero(),
            scroll_offset: 0,
            folds: Vec::new(),
            fold_ranges: Vec::new(),
            highlights: Vec::new(),
            highlight_layers: HashMap::new(),
            decoration_layers: HashMap::new(),
            synced_txn_seq: synced,
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

    /// A view that just applied `txn` itself is current; record how far.
    fn mark_synced(&mut self) {
        let seq = self
            .buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .txn_seq();
        self.synced_txn_seq = seq;
    }

    /// Replay transactions another fork committed since our last sync into
    /// this fork's cursor, annotations, and decorations — the split-view
    /// coherence path.  A gap larger than the buffer's tail cap cannot be
    /// replayed honestly: the cursor clamps and derived annotations stay
    /// stale until their owner re-publishes.
    pub fn sync_fork(&mut self) {
        let (seq, txns) = {
            let buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            (buf.txn_seq(), buf.txns_since(self.synced_txn_seq))
        };
        if txns.is_empty() {
            self.synced_txn_seq = seq;
            return;
        }
        let missed = seq.saturating_sub(self.synced_txn_seq) as usize;
        let decor = self.decor_byte_offsets();
        if txns.len() >= missed {
            for txn in &txns {
                let anchor = txn.transform_anchor(Anchor {
                    offset: self.cursor.offset,
                    affinity: Affinity::After,
                });
                self.transform_annotations(txn);
                self.set_cursor(anchor.offset);
            }
            self.remap_decorations(&decor, &txns);
                self.mark_synced();
        } else {
            // Evicted tail: bring the cursor somewhere legal and let the
            // derived layers be re-published.
            let len = self.buffer.lock().unwrap_or_else(|e| e.into_inner()).len();
            self.set_cursor(len.min(self.cursor.offset));
        }
        self.synced_txn_seq = seq;
    }

    /// Map a pre-transaction byte range to post-transaction coordinates.
    /// Ranges entirely after an edit shift by the delta — the same anchor
    /// arithmetic the cursor uses.  A range the edit cuts through keeps its
    /// surviving part (grows over an insertion inside it, clamps over a
    /// deletion): dropping it would blank a word or a whole string literal
    /// between the keystroke and the next parse landing.
    ///
    /// Edits are ascending and disjoint in *pre-transaction* coordinates
    /// (§A.3), so the range must be compared against every edit in those
    /// coordinates: the endpoints carry accumulated deltas and only the
    /// final values are shifted.  Shifting in place (comparing post-edit
    /// offsets against the next edit's pre-edit range) double-shifts a
    /// start that precedes a later edit the tail overlaps.
    fn transform_range(txn: &EditTransaction, range: (usize, usize)) -> Option<(usize, usize)> {
        let (mut s, mut e) = range;
        let mut shift_s: i64 = 0;
        let mut shift_e: i64 = 0;
        for ce in &txn.edits {
            let (start, end) = (ce.edit.range.start, ce.edit.range.end);
            if e <= start {
                break;
            }
            let delta = ce.edit.inserted_text.len() as i64 - (end - start) as i64;
            if s >= end {
                shift_s += delta;
                shift_e += delta;
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
                    // Span reduced to its tail: the tail starts at this
                    // edit's end in pre-transaction coordinates, so later
                    // edits keep being compared against [end, e).
                    (false, true) => {
                        s = end;
                        shift_s += delta;
                        shift_e += delta;
                    }
                    // Span bridges the edit: the head stays (every later
                    // edit starts past it), the tail shifts and may still
                    // overlap later edits.
                    (true, true) => {
                        shift_e += delta;
                    }
                }
            }
        }
        let s = (s as i64 + shift_s).max(0) as usize;
        let e = (e as i64 + shift_e).max(0) as usize;
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
        let decor = self.decor_byte_offsets();
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
            self.remap_decorations(&decor, std::slice::from_ref(&txn));
            self.mark_synced();
        }
    }

    /// Insert at several positions in one transaction (multi-cursor).
    /// Returns the transaction so callers can transform extra cursors.
    pub fn insert_many(&mut self, edits: &[(usize, &str)]) -> Option<EditTransaction> {
        let cursor_before = self.cursor.offset;
        let decor = self.decor_byte_offsets();
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
            self.remap_decorations(&decor, std::slice::from_ref(txn));
            self.mark_synced();
        }
        txn
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        let cursor_before = self.cursor.offset;
        let decor = self.decor_byte_offsets();
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            if buf.read_only {
                return;
            }
            buf.delete_with_cursor(start, end, cursor_before)
        };
        if let Some(txn) = txn {
            self.cursor_through(&txn);
            self.transform_annotations(&txn);
            self.remap_decorations(&decor, std::slice::from_ref(&txn));
            self.mark_synced();
        }
    }

    /// Delete several disjoint ranges in one transaction (multi-cursor).
    pub fn delete_many(&mut self, ranges: &[(usize, usize)]) -> Option<EditTransaction> {
        let cursor_before = self.cursor.offset;
        let decor = self.decor_byte_offsets();
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            if buf.read_only {
                return None;
            }
            buf.delete_many(ranges, cursor_before)
        };
        if let Some(txn) = &txn {
            self.cursor_through(txn);
            self.transform_annotations(txn);
            self.remap_decorations(&decor, std::slice::from_ref(txn));
            self.mark_synced();
        }
        txn
    }

    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        let cursor_before = self.cursor.offset;
        let decor = self.decor_byte_offsets();
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            if buf.read_only {
                return;
            }
            buf.replace_with_cursor(start, end, text, cursor_before)
        };
        if let Some(txn) = txn {
            self.cursor_through(&txn);
            self.transform_annotations(&txn);
            self.remap_decorations(&decor, std::slice::from_ref(&txn));
            self.mark_synced();
        }
    }

    pub fn undo(&mut self) -> bool {
        let decor = self.decor_byte_offsets();
        let restored = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.undo_with_txns()
        };
        match restored {
            Some((offset, txns)) => {
                for txn in &txns {
                    self.transform_annotations(txn);
                }
                self.remap_decorations(&decor, &txns);
                self.mark_synced();
                self.set_cursor(offset);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        let decor = self.decor_byte_offsets();
        let restored = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.redo_with_txns()
        };
        match restored {
            Some((offset, txns)) => {
                for txn in &txns {
                    self.transform_annotations(txn);
                }
                self.remap_decorations(&decor, &txns);
                self.mark_synced();
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

}
