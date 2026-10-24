//! The editor-facing Buffer facade (docs/core-redesign.md §2).
//!
//! Split from `mod.rs` (file-size gate): queries, the mutation gateway,
//! transaction undo, persistence state, change publication, and marks.

use super::anchor::Anchor;
use super::document::Document;
use super::marks::MarkSet;
use super::transaction::{EditTransaction, TransactionBuilder, TransactionOrigin};
use super::undo::UndoHistory;
use super::{MajorMode, Position};
use crate::kernel::state::id::{BufferId, MarkId};

#[derive(Debug)]
pub struct Buffer {
    pub id: BufferId,
    pub name: String,
    pub path: Option<String>,
    pub(super) doc: Document,
    marks: MarkSet,
    undo: UndoHistory,
    /// Transactions committed since the last change publication.  The event
    /// pump drains these into canonical `buffer-changed` events (§6).
    pending_changes: Vec<EditTransaction>,
    pub major_mode: MajorMode,
    pub encoding: String,
    /// LSP/tool diagnostics for this buffer, typed.  Render through
    /// `Diagnostic::display` — nothing parses display strings back apart.
    pub diagnostics: Vec<crate::kernel::semantic::provider::Diagnostic>,
    /// Per-buffer options that shadow global `editor.options`.
    pub local_options: std::collections::HashMap<String, String>,
    /// When true, `buffer/insert` and `buffer/delete` are no-ops via the Janet API.
    pub read_only: bool,
    /// Ephemeral buffers skip "save before closing?" prompts and session restore.
    pub ephemeral: bool,
    /// Pre-rendered text drawn above the buffer content area.
    pub header_line: Option<String>,
    /// DAP breakpoint anchors (breakpoint id → position).  Transformed
    /// through every committed transaction so breakpoints track their lines
    /// as the text edits, like marks do.
    pub bp_anchors: Vec<(u64, Anchor)>,
}

impl Buffer {
    pub fn new(id: BufferId, name: &str) -> Self {
        Buffer {
            id,
            name: name.to_string(),
            path: None,
            doc: Document::new(),
            marks: MarkSet::new(),
            undo: UndoHistory::new(),
            pending_changes: Vec::new(),
            major_mode: MajorMode::Fundamental,
            encoding: "utf-8".to_string(),
            diagnostics: Vec::new(),
            local_options: std::collections::HashMap::new(),
            read_only: false,
            ephemeral: false,
            header_line: None,
            bp_anchors: Vec::new(),
        }
    }

    pub fn from_string(id: BufferId, name: &str, content: &str) -> Self {
        let mut buf = Buffer::new(id, name);
        buf.doc = Document::from_string(content);
        buf
    }

    // ── Queries ──

    pub fn revision(&self) -> u64 {
        self.doc.revision().0
    }

    pub fn line_count(&self) -> usize {
        self.doc.line_index().line_count()
    }

    pub fn line_start_offset(&self, n: usize) -> Option<usize> {
        self.doc.line_index().line_start(n)
    }

    /// Content byte range of a line, excluding its terminator.
    pub fn line_span(&self, n: usize) -> Option<std::ops::Range<usize>> {
        self.doc.line_index().line_span(n)
    }

    pub fn len(&self) -> usize {
        self.doc.len()
    }

    pub fn is_empty(&self) -> bool {
        self.doc.is_empty()
    }

    pub fn slice(&self, start: usize, end: usize) -> String {
        self.doc_text_slice(start, end)
    }

    fn doc_text_slice(&self, start: usize, end: usize) -> String {
        let len = self.doc.len();
        let start = self.doc.snap_offset(start.min(len));
        let end = self.doc.snap_offset(end.min(len));
        if start >= end {
            String::new()
        } else {
            self.doc.rope_text(start, end)
        }
    }

    pub fn line(&self, n: usize) -> Option<String> {
        let li = self.doc.line_index();
        let span = li.line_span(n)?;
        Some(self.doc_text_slice(span.start, span.end))
    }

    pub fn lines(&self) -> Vec<String> {
        (0..self.line_count()).filter_map(|i| self.line(i)).collect()
    }

    pub fn char_at(&self, offset: usize) -> Option<char> {
        let len = self.doc.len();
        if offset >= len {
            return None;
        }
        let offset = self.doc.snap_offset(offset);
        self.doc.char_at(offset)
    }

    /// Borrow the underlying rope — for streaming consumers (tree-sitter).
    pub fn rope_handle(&self) -> &super::rope::Rope {
        self.doc.rope_handle()
    }

    /// Zero-based (line, byte-column) of an offset — tree-sitter points.
    pub fn line_col_bytes(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.doc.len());
        let li = self.doc.line_index();
        let row = li.line_of(offset);
        let col = offset - li.line_start(row).unwrap_or(0);
        (row, col)
    }

    /// LSP position of an offset: (line, UTF-16 code-unit column), as the
    /// language-server protocol requires (docs/core-redesign.md §8).
    pub fn lsp_position(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.doc.len());
        let li = self.doc.line_index();
        let line = li.line_of(offset);
        let start = li.line_start(line).unwrap_or(0);
        let column: usize = self.doc_text_slice(start, offset)
            .chars()
            .map(|c| c.len_utf16())
            .sum();
        (line, column)
    }

    /// Line/column for a byte offset without any prefix scan: the line
    /// index locates the line, the column is a char count over one line.
    pub fn position_of(&self, offset: usize) -> Position {
        let offset = offset.min(self.doc.len());
        let li = self.doc.line_index();
        let line = li.line_of(offset);
        let start = li.line_start(line).unwrap_or(0);
        let column = self.doc.chars_between(start, offset);
        Position { line, column, offset }
    }

    // ── The mutation gateway (§2.1) ──

    fn commit(
        &mut self,
        builder: TransactionBuilder,
        cursor_before: usize,
        cursor_after: usize,
    ) -> Option<EditTransaction> {
        let txn = self.doc.commit(builder).ok()??;
        self.marks.apply_transaction(&txn);
        for (_, anchor) in &mut self.bp_anchors {
            *anchor = txn.transform_anchor(*anchor);
        }
        self.undo.record(txn.clone(), cursor_before, cursor_after);
        self.pending_changes.push(txn.clone());
        Some(txn)
    }

    pub fn insert(&mut self, offset: usize, text: &str) {
        let _ = self.insert_with_cursor(offset, text, offset);
    }

    pub fn insert_with_cursor(
        &mut self,
        offset: usize,
        text: &str,
        cursor_before: usize,
    ) -> Option<EditTransaction> {
        if text.is_empty() {
            return None;
        }
        let offset = self.doc.snap_offset(offset);
        let cursor_after = offset + text.len();
        self.commit(
            TransactionBuilder::new().insert(offset, text),
            cursor_before,
            cursor_after,
        )
    }

    /// One transaction inserting at several positions (multi-cursor).
    /// Offsets are pre-transaction coordinates and must be distinct.
    /// The restored cursor sits after the text inserted at its own
    /// position, shifted by inserts made before it.
    pub fn insert_many(
        &mut self,
        edits: &[(usize, &str)],
        cursor_before: usize,
    ) -> Option<EditTransaction> {
        let len = self.doc.len();
        let mut cursor_after = cursor_before;
        let builder = edits.iter().fold(TransactionBuilder::new(), |b, &(off, text)| {
            let off = off.min(len);
            if off <= cursor_before {
                cursor_after += text.len();
            }
            b.insert(off, text)
        });
        self.commit(builder, cursor_before, cursor_after)
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        let _ = self.delete_with_cursor(start, end, 0);
    }

    pub fn delete_with_cursor(
        &mut self,
        start: usize,
        end: usize,
        cursor_before: usize,
    ) -> Option<EditTransaction> {
        let len = self.doc.len();
        let start = self.doc.snap_offset(start.min(len));
        let end = self.doc.snap_offset(end.min(len));
        if start >= end {
            return None;
        }
        self.commit(
            TransactionBuilder::new().delete(start..end),
            cursor_before,
            start,
        )
    }

    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        let _ = self.replace_with_cursor(start, end, text, 0);
    }

    pub fn replace_with_cursor(
        &mut self,
        start: usize,
        end: usize,
        text: &str,
        cursor_before: usize,
    ) -> Option<EditTransaction> {
        let len = self.doc.len();
        let start = self.doc.snap_offset(start.min(len));
        let end = self.doc.snap_offset(end.min(len));
        if start >= end && text.is_empty() {
            return None;
        }
        let cursor_after = start + text.len();
        self.commit(
            TransactionBuilder::new().replace(start..end, text),
            cursor_before,
            cursor_after,
        )
    }

    // ── Undo / redo (§4) ──

    /// Undo one group; returns the cursor offset to restore.
    pub fn undo(&mut self) -> Option<usize> {
        let (start, end, restore) = self.undo.undo_step()?;
        for i in (start..=end).rev() {
            let inverse = self.undo.entry(i).txn.invert(TransactionOrigin::Undo);
            let _ = self.commit(inverse, restore, restore);
        }
        self.undo.finish_undo(start);
        Some(restore)
    }

    /// Redo one group; returns the cursor offset to restore.
    pub fn redo(&mut self) -> Option<usize> {
        let (start, end, restore) = self.undo.redo_step()?;
        for i in start..=end {
            let edits: Vec<crate::kernel::text_engine::edit::Edit> = self
                .undo
                .entry(i)
                .txn
                .edits
                .iter()
                .map(|ce| ce.edit.clone())
                .collect();
            let builder =
                edits.into_iter().fold(
                    TransactionBuilder::new().with_origin(TransactionOrigin::Redo),
                    TransactionBuilder::push,
                );
            let _ = self.commit(builder, restore, restore);
        }
        self.undo.finish_redo(end);
        Some(restore)
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    // ── Undo sessions (explicit grouping, §4.3) ──

    pub fn open_undo_session_at(&mut self, _cursor_offset: usize) {
        self.undo.open_group();
    }

    pub fn close_undo_session(&mut self) {
        self.undo.close_group();
    }

    // ── Persistence state (§5) ──

    pub fn modified(&self) -> bool {
        self.undo.is_modified()
    }

    pub fn mark_saved(&mut self) {
        let cursor = self.undo.cursor_pos();
        self.undo.mark_saved_at_cursor(cursor);
    }

    /// Mark the revision an asynchronous write actually persisted.  A
    /// completion can never mark newer edits clean: only the exact undo
    /// position of that revision becomes the saved point.
    pub fn mark_saved_at(&mut self, rev: u64) {
        if let Some(pos) = self.undo.cursor_for_revision(rev) {
            self.undo.mark_saved_at_cursor(pos);
        }
    }

    // ── Change publication (§6) ──

    pub fn take_pending_changes(&mut self) -> Vec<EditTransaction> {
        std::mem::take(&mut self.pending_changes)
    }

    pub fn has_pending_changes(&self) -> bool {
        !self.pending_changes.is_empty()
    }

    /// Transactions committed strictly after `rev`, as
    /// `(revision_before, revision_after, [(start, old_end, new_end)])`.
    /// `None` when `rev` has fallen out of the retention window and the
    /// caller must re-sync from the document.
    pub fn changes_since(
        &self,
        rev: u64,
    ) -> Option<Vec<(u64, u64, Vec<(usize, usize, usize)>)>> {
        use crate::kernel::text_engine::revision::Revision;
        self.doc
            .changes_since(Revision(rev))
            .map(|txns| {
                txns.iter()
                    .map(|t| {
                        (
                            t.revision_before.0,
                            t.revision_after.0,
                            t.edits
                                .iter()
                                .map(|ce| {
                                    (
                                        ce.edit.start(),
                                        ce.edit.end(),
                                        ce.edit.start() + ce.edit.inserted_len(),
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect()
            })
    }

    // ── Diagnostics ──

    pub fn set_diagnostics(
        &mut self,
        diagnostics: Vec<crate::kernel::semantic::provider::Diagnostic>,
    ) {
        self.diagnostics = diagnostics;
    }

    pub fn get_diagnostics(&self) -> &[crate::kernel::semantic::provider::Diagnostic] {
        &self.diagnostics
    }

    /// Display strings through the single formatter (Janet queries, logs).
    pub fn diagnostic_strings(&self) -> Vec<String> {
        self.diagnostics.iter().map(|d| d.display()).collect()
    }

    // ── Marks (anchors, §2.3) ──

    pub fn create_mark(&mut self, pos: usize, sticky: bool) -> MarkId {
        self.marks.create(pos, sticky)
    }

    pub fn mark_pos(&self, id: MarkId) -> Option<usize> {
        self.marks.get(id).map(|m| m.pos())
    }

    pub fn delete_mark(&mut self, id: MarkId) -> bool {
        self.marks.delete(id)
    }

    pub fn mark_count(&self) -> usize {
        self.marks.len()
    }
}

