//! BufferView — per-window view state over a shared Buffer.
//!
//! Cursor movement after edits is anchor transformation, never ad-hoc span
//! arithmetic (docs/core-redesign.md §2.3/§3.3): the typing cursor behaves
//! as an `Affinity::After` anchor.  Position re-derivation uses the line
//! index — no prefix copies, no char scans.

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use super::{Buffer, Decoration, Position};
use super::anchor::{Affinity, Anchor};
use super::transaction::EditTransaction;

#[derive(Clone)]
pub struct BufferView {
    /// Shared backing buffer content (multiple views can reference the same buffer).
    pub buffer: Arc<Mutex<Buffer>>,
    pub cursor: Position,
    pub scroll_offset: usize,
    pub folds: Vec<(usize, usize)>,
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
            highlights: Vec::new(),
            highlight_layers: HashMap::new(),
            decoration_layers: HashMap::new(),
        }
    }

    // ── Cursor ──

    pub fn cursor_offset(&self) -> usize { self.cursor.offset }

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

    // ── Content mutations (delegate to Buffer, cursor transforms) ──

    pub fn insert(&mut self, offset: usize, text: &str) {
        let cursor_before = self.cursor.offset;
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            if buf.read_only { return; }
            buf.insert_with_cursor(offset, text, cursor_before)
        };
        if let Some(txn) = txn {
            self.cursor_through(&txn);
        }
    }

    /// Insert at several positions in one transaction (multi-cursor).
    /// Returns the transaction so callers can transform extra cursors.
    pub fn insert_many(&mut self, edits: &[(usize, &str)]) -> Option<EditTransaction> {
        let cursor_before = self.cursor.offset;
        let txn = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            if buf.read_only { return None; }
            buf.insert_many(edits, cursor_before)
        };
        if let Some(txn) = &txn {
            self.cursor_through(txn);
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
        }
    }

    pub fn undo(&mut self) -> bool {
        let restored = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.undo()
        };
        match restored {
            Some(offset) => {
                self.set_cursor(offset);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        let restored = {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.redo()
        };
        match restored {
            Some(offset) => {
                self.set_cursor(offset);
                true
            }
            None => false,
        }
    }

    pub fn open_undo_session(&mut self) {
        self.buffer.lock().unwrap_or_else(|e| e.into_inner()).open_undo_session_at(self.cursor.offset);
    }

    pub fn close_undo_session(&mut self) {
        self.buffer.lock().unwrap_or_else(|e| e.into_inner()).close_undo_session();
    }

    // ── Folds ──

    pub fn add_fold(&mut self, start: usize, end: usize) {
        self.folds.push((start, end));
        self.folds.sort_by_key(|a| a.0);
    }

    pub fn remove_fold(&mut self, start: usize, end: usize) {
        self.folds.retain(|f| f.0 != start || f.1 != end);
    }

    pub fn clear_folds(&mut self) { self.folds.clear(); }

    pub fn is_folded(&self, line_start: usize, line_end: usize) -> bool {
        self.folds.iter().any(|(fs, fe)| *fs <= line_start && *fe >= line_end)
    }

    // ── Highlights ──

    pub fn set_highlights(&mut self, ranges: Vec<(usize, usize, String)>) {
        self.highlights = ranges;
    }

    pub fn clear_highlights(&mut self) { self.highlights.clear(); }

    pub fn set_highlights_layer(&mut self, layer: &str, ranges: Vec<(usize, usize, String)>) {
        self.highlight_layers.insert(layer.to_string(), ranges);
    }

    pub fn clear_highlights_layer(&mut self, layer: &str) {
        self.highlight_layers.remove(layer);
    }

    pub fn clear_all_highlight_layers(&mut self) { self.highlight_layers.clear(); }

    // ── Decorations ──

    pub fn decor_set_inline(&mut self, layer: &str, line: usize, col: usize, text: String, face: String) {
        let v = self.decoration_layers.entry(layer.to_string()).or_default();
        v.retain(|d| !matches!(d, Decoration::InlineText { line: l, col: c, .. } if *l == line && *c == col));
        v.push(Decoration::InlineText { line, col, text, face });
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

    pub fn decor_clear_layer(&mut self, layer: &str) { self.decoration_layers.remove(layer); }

    pub fn decor_clear(&mut self) { self.decoration_layers.clear(); }

    pub fn decor_count_layer(&self, layer: &str) -> usize {
        self.decoration_layers.get(layer).map(|v| v.len()).unwrap_or(0)
    }
}
