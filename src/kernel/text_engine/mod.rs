//! Buffer layer — text storage (rope), undo/redo, marks, highlights, and diagnostics.

pub mod rope;
pub mod undo;
pub mod marks;
pub mod decoration;
mod fold;

pub use decoration::Decoration;

use rope::Rope;
use undo::{UndoTree, UndoOp};
use marks::MarkSet;
use crate::kernel::state::id::{BufferId, MarkId};

/// Buffer major mode — the content/context type of a buffer (Emacs-style).
///
/// The Rust core defines only structural parent modes.  Language-specific
/// modes (rust-mode, python-mode, …) are defined by Janet extensions and
/// stored as `Custom(String)`.  One major mode is active per buffer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MajorMode {
    /// No special mode; plain editing with no added keybindings.
    #[default]
    Fundamental,
    /// Parent mode for all human-readable prose buffers.
    Text,
    /// Parent mode for all programming-language buffers.
    Prog,
    /// Extension-defined mode (e.g. "rust-mode", "markdown-mode").
    Custom(String),
}

impl MajorMode {
    pub fn name(&self) -> &str {
        match self {
            MajorMode::Fundamental => "fundamental",
            MajorMode::Text        => "text",
            MajorMode::Prog        => "prog",
            MajorMode::Custom(s)   => s.as_str(),
        }
    }

    /// Parse a major mode from a string supplied by the user or an extension.
    /// Anything the core doesn't recognise becomes `Custom`.
    pub fn from_name(s: &str) -> Self {
        match s {
            "fundamental" | "fundamental-mode" => MajorMode::Fundamental,
            "text"        | "text-mode"        => MajorMode::Text,
            "prog"        | "prog-mode"        => MajorMode::Prog,
            other                              => MajorMode::Custom(other.to_string()),
        }
    }
}

#[derive(Debug)]
pub struct Buffer {
    pub id: BufferId,
    pub name: String,
    pub path: Option<String>,
    pub rope: Rope,
    pub undo: UndoTree,
    pub marks: MarkSet,
    pub cursor: usize,
    pub major_mode: MajorMode,
    pub encoding: String,
    pub len: usize,
    /// Highlight regions as `(start_byte, end_byte, face_name)` tuples.
    pub highlights: Vec<(usize, usize, String)>,
    /// Per-layer highlight regions. Layer merge order:
    /// `base → syntax → semantic → search → selection` (higher wins per cell).
    pub highlight_layers: std::collections::HashMap<String, Vec<(usize, usize, String)>>,
    /// LSP diagnostics for this buffer (formatted strings).
    pub diagnostics: Vec<String>,
    /// Per-buffer options that shadow global `editor.options`.
    pub local_options: std::collections::HashMap<String, String>,
    /// When true, `buffer/insert` and `buffer/delete` are no-ops via the Janet API.
    pub read_only: bool,
    /// Ephemeral buffers skip "save before closing?" prompts and session restore.
    pub ephemeral: bool,
    /// Folded byte ranges [start, end).  Lines inside are hidden during rendering.
    pub folds: Vec<(usize, usize)>,
    /// Named decoration layers owned by extensions.
    pub decoration_layers: std::collections::HashMap<String, Vec<Decoration>>,
    /// Pre-rendered text drawn above the buffer content area.
    pub header_line: Option<String>,
}

impl Buffer {
    pub fn new(id: BufferId, name: &str) -> Self {
        Buffer {
            id,
            name: name.to_string(),
            path: None,
            rope: Rope::new(),
            undo: UndoTree::new(10000),
            marks: MarkSet::new(),
            cursor: 0,
            major_mode: MajorMode::Fundamental,
            encoding: "utf-8".to_string(),
            len: 0,
            highlights: Vec::new(),
            highlight_layers: std::collections::HashMap::new(),
            diagnostics: Vec::new(),
            local_options: std::collections::HashMap::new(),
            read_only: false,
            ephemeral: false,
            folds: Vec::new(),
            decoration_layers: std::collections::HashMap::new(),
            header_line: None,
        }
    }

    pub fn from_string(id: BufferId, name: &str, content: &str) -> Self {
        let mut buf = Buffer::new(id, name);
        buf.rope = Rope::from_string(content);
        buf.len = buf.rope.len();
        buf
    }

    pub fn line_count(&self) -> usize {
        self.rope.line_count()
    }

    pub fn line_start_offset(&self, n: usize) -> Option<usize> {
        self.rope.line_start_offset(n)
    }

    pub fn len(&self) -> usize {
        self.rope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    pub fn insert(&mut self, offset: usize, text: &str) {
        let mark_states = self.marks.all_positions();
        self.undo.record(UndoOp::Insert {
            offset,
            text: text.to_string(),
        }, mark_states);
        self.rope.insert(offset, text);
        self.marks.adjust_for_insert(offset, text.len());
        self.len = self.rope.len();
        if offset <= self.cursor {
            self.cursor += text.len();
        }
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        let deleted_text = self.rope.slice(start, end).to_string();
        let mark_states = self.marks.all_positions();
        self.undo.record(UndoOp::Delete {
            offset: start,
            text: deleted_text,
        }, mark_states);
        self.rope.delete(start, end);
        self.marks.adjust_for_delete(start, end);
        self.len = self.rope.len();
        if self.cursor > start {
            self.cursor = if self.cursor <= end {
                start
            } else {
                self.cursor - (end - start)
            };
        }
    }

    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        let deleted_text = self.rope.slice(start, end).to_string();
        let mark_states = self.marks.all_positions();
        self.undo.record(UndoOp::Delete {
            offset: start,
            text: deleted_text,
        }, mark_states.clone());
        self.rope.delete(start, end);
        self.marks.adjust_for_delete(start, end);

        self.undo.record(UndoOp::Insert {
            offset: start,
            text: text.to_string(),
        }, mark_states);
        self.rope.insert(start, text);
        self.marks.adjust_for_insert(start, text.len());
        self.len = self.rope.len();

        if self.cursor >= start {
            let deleted = end - start;
            self.cursor = if self.cursor <= end {
                start + text.len()
            } else {
                self.cursor - deleted + text.len()
            };
        }
    }

    pub fn slice(&self, start: usize, end: usize) -> String {
        self.rope.slice(start, end).to_string()
    }

    pub fn line(&self, n: usize) -> Option<String> {
        self.rope.line(n).map(|c| c.to_string())
    }

    pub fn lines(&self) -> Vec<String> {
        self.rope.all_lines()
    }

    pub fn char_at(&self, offset: usize) -> Option<char> {
        self.rope.char_at(offset)
    }

    pub fn undo(&mut self) -> bool {
        if let Some(ops) = self.undo.undo() {
            for op in ops.iter().rev() {
                match op {
                    UndoOp::Insert { offset, text } => {
                        self.rope.delete(*offset, offset + text.len());
                        self.marks.adjust_for_delete(*offset, offset + text.len());
                    }
                    UndoOp::Delete { offset, text } => {
                        self.rope.insert(*offset, text);
                        self.marks.adjust_for_insert(*offset, text.len());
                    }
                }
            }
            if let Some(states) = self.undo.undo_mark_states() {
                self.marks.restore_positions(states);
            }
            self.len = self.rope.len();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(ops) = self.undo.redo() {
            for op in ops {
                match op {
                    UndoOp::Insert { offset, text } => {
                        self.rope.insert(*offset, text);
                        self.marks.adjust_for_insert(*offset, text.len());
                    }
                    UndoOp::Delete { offset, text } => {
                        self.rope.delete(*offset, offset + text.len());
                        self.marks.adjust_for_delete(*offset, offset + text.len());
                    }
                }
            }
            if let Some(states) = self.undo.redo_mark_states() {
                self.marks.restore_positions(states);
            }
            self.len = self.rope.len();
            true
        } else {
            false
        }
    }

    pub fn modified(&self) -> bool {
        self.undo.is_modified_since_save()
    }

    pub fn mark_saved(&mut self) {
        self.undo.mark_saved();
    }

    // ── Cursor ──

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, pos: usize) {
        // Allow cursor at len (end-of-buffer) so the last char is reachable for appending.
        self.cursor = pos.min(self.len());
    }

    // ── Undo sessions ──

    pub fn open_undo_session(&mut self) {
        let offset = self.cursor;
        let mark_states = self.marks.all_positions();
        self.undo.open_session(undo::UndoOp::Insert {
            offset,
            text: String::new(),
        }, mark_states);
    }

    pub fn close_undo_session(&mut self) {
        self.undo.close_session();
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

    // ── Diagnostics ──

    pub fn set_diagnostics(&mut self, diagnostics: Vec<String>) {
        self.diagnostics = diagnostics;
    }

    pub fn get_diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    // ── Marks ──

    pub fn create_mark(&mut self, pos: usize, sticky: bool) -> MarkId {
        self.marks.create(pos, sticky)
    }

    pub fn mark_pos(&self, id: MarkId) -> Option<usize> {
        self.marks.get(id).map(|m| m.pos)
    }

    pub fn delete_mark(&mut self, id: MarkId) -> bool {
        self.marks.delete(id)
    }
}

#[cfg(test)]
mod tests;
