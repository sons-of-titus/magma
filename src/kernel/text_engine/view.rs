//! BufferView — per-window view state over a shared Buffer.

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use super::{Buffer, Decoration, Position};

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
        let buf = self.buffer.lock().unwrap();
        let offset = offset.min(buf.len());
        let text = buf.rope.slice(0, offset).to_string();
        drop(buf);
        self.cursor = Position::from_offset(&text, offset);
    }

    // ── Content mutations (delegate to Buffer AND update cursor) ──

    pub fn insert(&mut self, offset: usize, text: &str) {
        let cursor_before = self.cursor.offset;
        {
            let mut buf = self.buffer.lock().unwrap();
            if buf.read_only { return; }
            buf.insert(offset, text);
        }
        let new_offset = if offset <= cursor_before {
            cursor_before + text.len()
        } else {
            cursor_before
        };
        self.recompute_cursor_at(new_offset);
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        let cursor_before = self.cursor.offset;
        {
            let mut buf = self.buffer.lock().unwrap();
            buf.delete(start, end);
        }
        let new_offset = if cursor_before > start {
            if cursor_before <= end { start } else { cursor_before - (end - start) }
        } else {
            cursor_before
        };
        self.recompute_cursor_at(new_offset);
    }

    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        let cursor_before = self.cursor.offset;
        {
            let mut buf = self.buffer.lock().unwrap();
            buf.replace(start, end, text);
        }
        let deleted = end - start;
        let new_offset = if cursor_before >= start {
            if cursor_before <= end {
                start + text.len()
            } else {
                cursor_before - deleted + text.len()
            }
        } else {
            cursor_before
        };
        self.recompute_cursor_at(new_offset);
    }

    pub fn undo(&mut self) -> bool {
        let ok = self.buffer.lock().unwrap().undo();
        if ok {
            let buf = self.buffer.lock().unwrap();
            let offset = self.cursor.offset.min(buf.len());
            let text = buf.rope.slice(0, offset).to_string();
            drop(buf);
            self.cursor = Position::from_offset(&text, offset);
        }
        ok
    }

    pub fn redo(&mut self) -> bool {
        let ok = self.buffer.lock().unwrap().redo();
        if ok {
            let buf = self.buffer.lock().unwrap();
            let offset = self.cursor.offset.min(buf.len());
            let text = buf.rope.slice(0, offset).to_string();
            drop(buf);
            self.cursor = Position::from_offset(&text, offset);
        }
        ok
    }

    pub fn open_undo_session(&mut self) {
        let cursor_offset = self.cursor.offset;
        self.buffer.lock().unwrap().open_undo_session_at(cursor_offset);
    }

    pub fn close_undo_session(&mut self) {
        self.buffer.lock().unwrap().close_undo_session();
    }

    fn recompute_cursor_at(&mut self, offset: usize) {
        let buf = self.buffer.lock().unwrap();
        let offset = offset.min(buf.len());
        let text = buf.rope.slice(0, offset).to_string();
        drop(buf);
        self.cursor = Position::from_offset(&text, offset);
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
