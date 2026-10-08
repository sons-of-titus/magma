//! Editor view-resolution, change publication, and render signatures.
//!
//! Split from `mod.rs` (file-size gate): everything that routes editor
//! access through views or publishes document changes to consumers.

use crate::kernel::state::Editor;

impl Editor {
    /// Get the focused window's view.
    pub fn focused_view(&self) -> Option<&crate::kernel::text_engine::BufferView> {
        let win = self.view_tree.focused_window()?;
        let key = self.view_tree.buffer(win)?;
        match self.window_views.get(&(win, key)) {
            Some(v) => Some(v),
            None => self.views.get(&key),
        }
    }

    /// Get the focused window's view (mutable).
    pub fn focused_view_mut(&mut self) -> Option<&mut crate::kernel::text_engine::BufferView> {
        let win = self.view_tree.focused_window()?;
        let key = self.view_tree.buffer(win)?;
        match self.window_views.get_mut(&(win, key)) {
            Some(v) => Some(v),
            None => self.views.get_mut(&key),
        }
    }

    /// Fork the current view state of `buf` into a per-window view for
    /// `win` (split-window independence, docs/core-redesign.md §3).
    pub fn fork_view_for_window(&mut self, win: crate::kernel::state::id::WindowId, buf: usize) {
        if let Some(v) = self.views.get(&buf).cloned() {
            self.window_views.insert((win, buf), v);
        }
    }

    pub fn drop_window_views_of_buffer(&mut self, buf: usize) {
        self.window_views.retain(|(_, b), _| *b != buf);
    }

    /// Resolve the view of a buffer: the focused window's fork if it shows
    /// this buffer, else any window's fork, else the default view.
    pub fn view_of_buffer(&self, buf: usize) -> Option<&crate::kernel::text_engine::BufferView> {
        if let Some(win) = self.view_tree.focused_window() {
            if self.view_tree.buffer(win) == Some(buf) {
                if let Some(v) = self.window_views.get(&(win, buf)) {
                    return Some(v);
                }
            }
        }
        if let Some((_, v)) = self.window_views.iter().find(|((_, b), _)| *b == buf) {
            return Some(v);
        }
        self.views.get(&buf)
    }

    /// Mutable `view_of_buffer`.
    pub fn view_of_buffer_mut(&mut self, buf: usize) -> Option<&mut crate::kernel::text_engine::BufferView> {
        let focused = self.view_tree.focused_window();
        let focused_buf = focused.and_then(|w| self.view_tree.buffer(w));
        if let (Some(win), Some(shown)) = (focused, focused_buf) {
            if shown == buf && self.window_views.contains_key(&(win, buf)) {
                return self.window_views.get_mut(&(win, buf));
            }
        }
        let forked = self
            .window_views
            .iter()
            .find(|((_, b), _)| *b == buf)
            .map(|((w, b), _)| (*w, *b));
        match forked {
            Some(k) => self.window_views.get_mut(&k),
            None => self.views.get_mut(&buf),
        }
    }

    /// Drain every buffer's pending transactions into canonical
    /// `buffer-changed` events — one event per committed transaction, each
    /// carrying its revision transition and edit spans (docs/core-redesign.md
    /// §6).  Call before draining the event bus.
    pub fn publish_buffer_changes(&mut self) {
        let mut notes: Vec<(usize, crate::kernel::text_engine::transaction::EditTransaction)> =
            Vec::new();
        for (key, arc) in self.buffers.iter() {
            let mut buf = arc.lock().unwrap();
            if buf.has_pending_changes() {
                for txn in buf.take_pending_changes() {
                    notes.push((key, txn));
                }
            }
        }
        for (key, txn) in notes {
            let edits = txn
                .edits
                .iter()
                .map(|ce| {
                    format!(
                        "{}:{}:{}",
                        ce.edit.start(),
                        ce.edit.end(),
                        ce.edit.start() + ce.edit.inserted_len()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            self.events.emit_typed(
                crate::kernel::event::keys::events::BUFFER_CHANGED,
                crate::kernel::event::payload::BufferChangedPayload {
                    buffer_id: key.to_string(),
                    revision_before: txn.revision_before.0.to_string(),
                    revision_after: txn.revision_after.0.to_string(),
                    edits,
                },
            );
        }
    }

    /// A hash of everything the render depends on: when it is unchanged
    /// between frames there is nothing new to paint and the GUI does not
    /// need to schedule a repaint (docs/core-redesign.md §9.4).
    pub fn render_signature(&self) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x100000001b3);
        };
        for (key, arc) in self.buffers.iter() {
            let b = arc.lock().unwrap();
            mix(key as u64);
            mix(b.revision());
            mix(b.diagnostics.len() as u64);
        }
        for v in self.views.values() {
            mix(v.cursor.offset as u64);
            mix(v.scroll_offset as u64);
        }
        for v in self.window_views.values() {
            mix(v.cursor.offset as u64);
        }
        mix(self.selection.is_some() as u64);
        mix(self.editor_mode.accepts_text as u64);
        mix(self.editor_mode.minibuffer.is_some() as u64);
        if let Some(ref mb) = self.editor_mode.minibuffer {
            mix(mb.input.len() as u64);
        }
        mix(self.completion.visible as u64);
        mix(self.overlays.len() as u64);
        mix(self.events.pending_len() as u64);
        mix(self.io.terminal_mode_buf.is_some() as u64);
        h
    }

    /// Publish pending change events, then dispatch the event queue inside
    /// a Janet scope: handlers run with the editor pointer installed from
    /// this live borrow (docs/core-redesign.md §11).
    pub fn dispatch_events_scoped(&mut self) {
        self.publish_buffer_changes();
        crate::kernel::scripting::scoped_editor(self, || {
            crate::kernel::scripting::with_editor(|ed| {
                ed.events.drain_and_dispatch();
                crate::kernel::scripting::conv::nil()
            });
        });
    }

}
