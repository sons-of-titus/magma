//! Editor view-resolution, change publication, and render signatures.
//!
//! Split from `mod.rs` (file-size gate): everything that routes editor
//! access through views or publishes document changes to consumers.

use crate::state::Editor;

impl Editor {
    /// Get the focused window's view.
    pub fn focused_view(&self) -> Option<&crate::text_engine::BufferView> {
        let win = self.view_tree.focused_window()?;
        let key = self.view_tree.buffer(win)?;
        match self.window_views.get(&(win, key)) {
            Some(v) => Some(v),
            None => self.views.get(&key),
        }
    }

    /// Get the focused window's view (mutable).
    pub fn focused_view_mut(&mut self) -> Option<&mut crate::text_engine::BufferView> {
        let win = self.view_tree.focused_window()?;
        let key = self.view_tree.buffer(win)?;
        match self.window_views.get_mut(&(win, key)) {
            Some(v) => Some(v),
            None => self.views.get_mut(&key),
        }
    }

    /// Fork the current view state of `buf` into a per-window view for
    /// `win` (split-window independence, docs/core-redesign.md §3).
    pub fn fork_view_for_window(&mut self, win: crate::state::id::WindowId, buf: usize) {
        if let Some(v) = self.views.get(&buf).cloned() {
            self.window_views.insert((win, buf), v);
        }
    }

    pub fn drop_window_views_of_buffer(&mut self, buf: usize) {
        self.window_views.retain(|(_, b), _| *b != buf);
    }

    /// Resolve the view of a buffer: the focused window's fork if it shows
    /// this buffer, else any window's fork, else the default view.
    pub fn view_of_buffer(&self, buf: usize) -> Option<&crate::text_engine::BufferView> {
        if let Some(win) = self.view_tree.focused_window()
            && self.view_tree.buffer(win) == Some(buf)
                && let Some(v) = self.window_views.get(&(win, buf)) {
                    return Some(v);
                }
        if let Some((_, v)) = self.window_views.iter().find(|((_, b), _)| *b == buf) {
            return Some(v);
        }
        self.views.get(&buf)
    }

    /// Mutable `view_of_buffer`.
    pub fn view_of_buffer_mut(&mut self, buf: usize) -> Option<&mut crate::text_engine::BufferView> {
        let focused = self.view_tree.focused_window();
        let focused_buf = focused.and_then(|w| self.view_tree.buffer(w));
        if let (Some(win), Some(shown)) = (focused, focused_buf)
            && shown == buf && self.window_views.contains_key(&(win, buf)) {
                return self.window_views.get_mut(&(win, buf));
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

    /// Remove a buffer with its views and point every pane that showed it
    /// at a surviving buffer.  The one close path for buffer teardown.
    pub fn drop_buffer(&mut self, key: usize) {
        self.buffers.remove(key);
        self.views.remove(&key);
        self.drop_window_views_of_buffer(key);
        self.completion.word_cache.remove(&key);
        self.minimap_cache.remove(&key);
        // Buffer-local keys are keyed by slab slot, which the slab reuses.
        self.keymaps.clear_buffer_local(key as u64);
        let fallback = self.buffers.iter().map(|(k, _)| k).find(|&k| k != key);
        for pane in self.view_tree.panes_mut() {
            if pane.buffer_id == Some(key) {
                pane.buffer_id = fallback;
            }
        }
    }

    /// Drain every buffer's pending transactions into canonical
    /// `buffer-changed` events — one event per committed transaction, each
    /// carrying its revision transition and edit spans (docs/core-redesign.md
    /// §6).  Call before draining the event bus.
    pub fn publish_buffer_changes(&mut self) {
        let mut notes: Vec<(usize, crate::text_engine::transaction::EditTransaction)> =
            Vec::new();
        for (key, arc) in self.buffers.iter() {
            let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            if buf.has_pending_changes() {
                for txn in buf.take_pending_changes() {
                    notes.push((key, txn));
                }
            }
        }
        let mut ts_dirty: Vec<usize> = Vec::new();
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
            if self.ts_languages.contains_key(&key) {
                ts_dirty.push(key);
            }
            self.events.emit_typed(
                crate::event::keys::events::BUFFER_CHANGED,
                crate::event::payload::BufferChangedPayload {
                    buffer_id: key.to_string(),
                    revision_before: txn.revision_before.0.to_string(),
                    revision_after: txn.revision_after.0.to_string(),
                    edits,
                },
            );
        }
        // Changed buffers with a registered grammar get a lane-scheduled
        // parse; the interactive path returns immediately.
        for key in ts_dirty {
            crate::semantic::ts_schedule::schedule_tree_update(self, key);
        }
    }

    /// Publish pending change events, then dispatch the event queue inside
    /// a Janet scope: handlers run with the editor pointer installed from
    /// this live borrow (docs/core-redesign.md §11).
    pub fn dispatch_events_scoped(&mut self) {
        self.publish_buffer_changes();
        crate::script_runtime::dispatch_events(self);
    }

}
