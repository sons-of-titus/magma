//! External file-change handling for open buffers (docs/core-redesign.md §5).
//!
//! `FileWatcher` reports disk writes; this module keeps open buffers honest:
//! an unmodified buffer auto-reverts to the disk text (cursor line/column
//! preserved, undoable), a modified buffer is never clobbered — the user
//! gets a warning instead, and saving stays an explicit choice.

use crate::state::Editor;

/// Slab key of the open buffer whose path canonicalizes to `disk_path`.
/// A deleted file cannot be canonicalized — fall back to the literal path.
pub fn buffer_at_path(ed: &Editor, disk_path: &str) -> Option<usize> {
    ed.buffers.iter().find_map(|(key, arc)| {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let p = buf.path.as_ref()?;
        let canon = std::fs::canonicalize(p)
            .ok()
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.clone());
        (canon == disk_path).then_some(key)
    })
}

/// React to a watcher event for `disk_path`.  Cheap enough for the
/// foreground thread: one canonicalize per open buffer, no I/O beyond the
/// reload read itself.
pub fn handle_file_changed(ed: &mut Editor, disk_path: &str, kind: &str) {
    let Some(buf_key) = buffer_at_path(ed, disk_path) else {
        return;
    };

    if kind == "removed" {
        ed.show_message(
            crate::state::MessageKind::Error,
            format!("{disk_path} was removed on disk"),
        );
        return;
    }

    let (dirty, name) = ed
        .buffers
        .get(buf_key)
        .map(|arc| {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            (buf.modified(), buf.name.clone())
        })
        .unwrap_or((false, String::new()));

    if dirty {
        ed.show_message(
            crate::state::MessageKind::Error,
            format!(
                "{name} changed on disk and has unsaved edits — saving will overwrite the disk file"
            ),
        );
        return;
    }

    let path = disk_path.to_string();
    match ed.background.clone() {
        Some(bg) => {
            let sender = bg.sender.clone();
            bg.spawn_blocking(move || {
                let content = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                let _ = sender
                    .send(crate::runtime::BackgroundEvent::BufferReloaded { buf_key, content });
            });
        }
        // Without a pool (tests, embedders) the read is cheap enough to do
        // inline on the caller's thread.
        None => {
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string());
            apply_reload(ed, buf_key, content);
        }
    }
}

/// Apply a completed disk read to an open buffer.  `Ok` content that
/// matches the buffer is a watcher echo of our own save — no transaction,
/// just the clean mark.
pub fn apply_reload(ed: &mut Editor, buf_key: usize, content: Result<String, String>) {
    let content = match content {
        Ok(c) => c,
        Err(e) => {
            ed.show_message(
                crate::state::MessageKind::Error,
                format!("reload failed: {e}"),
            );
            return;
        }
    };

    let identical = ed.buffers.get(buf_key).is_some_and(|arc| {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        buf.slice(0, buf.len()) == content
    });
    if identical {
        if let Some(arc) = ed.buffers.get(buf_key) {
            let rev = arc.lock().unwrap_or_else(|e| e.into_inner()).revision();
            arc.lock()
                .unwrap_or_else(|e| e.into_inner())
                .mark_saved_at(rev);
        }
        return;
    }

    // Re-check dirtiness at apply time: the event-time check happened
    // before the background read, and keystrokes landing in between must
    // not be clobbered by disk text (PM-09).
    let (dirty, name) = ed
        .buffers
        .get(buf_key)
        .map(|arc| {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            (buf.modified(), buf.name.clone())
        })
        .unwrap_or((false, String::new()));
    if dirty {
        ed.show_message(
            crate::state::MessageKind::Error,
            format!(
                "{name} changed on disk and has unsaved edits — saving will overwrite the disk file"
            ),
        );
        return;
    }

    let Some(view) = ed.view_of_buffer(buf_key) else {
        return;
    };
    let (line, column) = {
        let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
        let p = buf.position_of(view.cursor.offset);
        (p.line, p.column)
    };

    let len = view.buffer.lock().unwrap_or_else(|e| e.into_inner()).len();
    if let Some(view) = ed.view_of_buffer_mut(buf_key) {
        view.replace(0, len, &content);
    }

    // Restore the cursor by line/char-column on the new text — a
    // full-buffer replace would otherwise transform the cursor anchor to
    // the end of the replacement.
    if let Some(view) = ed.view_of_buffer_mut(buf_key) {
        let offset = {
            let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
            let l = line.min(buf.line_count().saturating_sub(1));
            let start = buf.line_start_offset(l).unwrap_or(0);
            let end = buf.line_start_offset(l + 1).unwrap_or(buf.len());
            let line_text = buf.slice(start, end);
            start
                + line_text
                    .chars()
                    .take_while(|c| *c != '\n')
                    .take(column)
                    .map(|c| c.len_utf8())
                    .sum::<usize>()
        };
        view.set_cursor(offset);
    }
    if let Some(arc) = ed.buffers.get(buf_key) {
        let rev = arc.lock().unwrap_or_else(|e| e.into_inner()).revision();
        arc.lock()
            .unwrap_or_else(|e| e.into_inner())
            .mark_saved_at(rev);
    }
    ed.frame_dirty = true;
}
