//! Buffer-word completion candidates.
//!
//! Word collection is revision-cached per buffer: the prefix comes from the
//! cursor's line only (O(line), never a document copy), and the candidate
//! set is a `BTreeSet` queried by prefix range.  Buffers past the inline
//! limit are scanned on the background pool, chunk-by-chunk under short
//! buffer locks, and land via the revision-checked `BufferWords` event
//! (rework.md §3: no O(document) work on the keystroke path).

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::state::Editor;
use crate::state::completion::WordCache;

/// Buffers up to this size build their word set inline on the foreground;
/// larger ones go to the background pool.  The limit stays tiny: every
/// keystroke changes the revision, so anything scanned inline re-scans on
/// every keystroke — the source of the "typing slows down" degradation
/// when this limit covered 256 KB documents.
const INLINE_WORD_SCAN_LIMIT: usize = 16 * 1024;
/// Background scan chunk: one buffer lock per chunk, so the interactive
/// path never waits behind a whole-document scan.
const WORD_SCAN_CHUNK: usize = 1 << 20;

pub(crate) fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Length of the word prefix directly before the cursor — the cursor's line
/// only, so the cost is O(line) regardless of document size.
pub fn word_prefix_len(editor: &Editor) -> usize {
    let Some((line_text, col)) = cursor_line_and_col(editor) else {
        return 0;
    };
    let before = &line_text[..col.min(line_text.len())];
    let word_start = before
        .rfind(|c: char| !is_word_char(c))
        .map(|i| i + 1)
        .unwrap_or(0);
    before.len() - word_start
}

/// (line text, cursor byte column within that line) of the focused view.
pub(crate) fn cursor_line_and_col(editor: &Editor) -> Option<(String, usize)> {
    let buf_id = editor
        .view_tree
        .focused_window()
        .and_then(|wid| editor.view_tree.buffer(wid))?;
    let view = editor.view_of_buffer(buf_id)?;
    let cursor = view.cursor_offset();
    let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
    let line = buf.line_col_bytes(cursor).0;
    let start = buf.line_start_offset(line)?;
    let line_text = buf.line(line)?;
    Some((line_text, cursor - start))
}

/// Words in `text` — a hand-rolled split that keeps only word runs.
fn scan_words(text: &str, out: &mut BTreeSet<String>) {
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices().chain([(text.len(), '\0')]) {
        if is_word_char(c) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            out.insert(text[s..i].to_string());
        }
    }
}

/// The candidate word set for `buf_id` at its current revision.
///
/// Cache hit → the set.  Small buffer → build inline.  Big buffer → schedule
/// one background scan per revision and serve the previous revision's set in
/// the meantime (display-only staleness, refreshed on arrival).
pub(crate) fn candidate_words(editor: &mut Editor, buf_id: usize) -> Option<Arc<BTreeSet<String>>> {
    let arc = editor.buffers.get(buf_id)?;
    let (revision, len) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        (buf.revision(), buf.len())
    };

    if let Some(cache) = editor.completion.word_cache.get(&buf_id)
        && cache.revision == revision
    {
        return Some(cache.words.clone());
    }

    if len <= INLINE_WORD_SCAN_LIMIT {
        let mut words = BTreeSet::new();
        {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            for chunk in buf.rope_handle().chunks() {
                scan_words(chunk, &mut words);
            }
        }
        let words = Arc::new(words);
        editor.completion.word_cache.insert(
            buf_id,
            WordCache {
                revision,
                words: words.clone(),
            },
        );
        editor.completion.words_requested = None;
        return Some(words);
    }

    // Big buffer: one scan per revision, results land off-thread.
    if editor.completion.words_requested != Some((buf_id, revision))
        && let Some(bg) = editor.background.clone()
    {
        editor.completion.words_requested = Some((buf_id, revision));
        let arc = arc.clone();
        let sender = bg.sender.clone();
        // Lane P1, coalesced per buffer: typing past a queued scan
        // supersedes it.
        let coalesce = format!("words-{buf_id}");
        bg.spawn_on(
            crate::runtime::Lane::Foreground,
            Some(&coalesce),
            move || {
                let mut words = BTreeSet::new();
                let mut offset = 0usize;
                loop {
                    let (chunk, still_current) = {
                        // Re-lock per chunk: the interactive path may mutate
                        // between chunks, and a changed revision voids the scan.
                        match arc2_lock(&arc) {
                            Some(buf) => {
                                let current = buf.revision();
                                if current != revision {
                                    (String::new(), false)
                                } else if offset >= buf.len() {
                                    (String::new(), true)
                                } else {
                                    let end = (offset + WORD_SCAN_CHUNK).min(buf.len());
                                    let text = buf.slice(offset, end);
                                    offset = end;
                                    (text, true)
                                }
                            }
                            None => (String::new(), false),
                        }
                    };
                    if !still_current {
                        return;
                    }
                    scan_words(&chunk, &mut words);
                    if chunk.is_empty() {
                        break;
                    }
                }
                let _ = sender.send(crate::runtime::BackgroundEvent::BufferWords {
                    buf_key: buf_id,
                    revision,
                    words: words.into_iter().collect(),
                });
            },
        );
    }

    // Serve whatever previous revision we have while the scan runs.
    editor
        .completion
        .word_cache
        .get(&buf_id)
        .map(|c| c.words.clone())
}

/// Lock helper so the chunk loop above stays readable.
fn arc2_lock(
    arc: &std::sync::Arc<std::sync::Mutex<crate::text_engine::Buffer>>,
) -> Option<std::sync::MutexGuard<'_, crate::text_engine::Buffer>> {
    arc.lock().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::builtin;
    use crate::state::Editor;
    use crate::storage::disk::DiskFileSystem;

    #[test]
    fn midsize_buffers_never_scan_words_inline_on_the_keystroke_path() {
        // The regression: buffers under the old 256 KB inline limit rebuilt
        // the whole word set on the foreground for every keystroke.  A
        // 100 KB buffer must take the coalesced/background path instead.
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let body = "fn alpha() {}\nuse beta_widget;\n".repeat(100_000 / 26);
        let key = ed.create_buffer_from_str("big", &body);
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }

        // Drive the trigger through the same entry the insert path uses.
        if let Some(view) = ed.view_of_buffer_mut(key) {
            view.set_cursor(4); // inside "alpha"
        }
        crate::completion::trigger(&mut ed);
        let prefix_hit = ed.completion.visible;

        assert!(
            !ed.completion.word_cache.contains_key(&key),
            "a mid-size buffer must not build its word set inline"
        );
        assert!(
            !prefix_hit,
            "without a background pool the big-buffer path serves no candidates yet"
        );
    }

    #[test]
    fn tiny_buffers_still_build_their_word_set_inline() {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        let key = ed.create_buffer_from_str("small", "alpha beta gamma\n");
        if let Some(win) = ed.view_tree.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        if let Some(view) = ed.view_of_buffer_mut(key) {
            view.set_cursor(2); // inside "alpha"
        }
        crate::completion::trigger(&mut ed);
        assert!(
            ed.completion.visible,
            "the popup opens from the inline scan"
        );
        assert!(ed.completion.word_cache.contains_key(&key));
        let labels: Vec<&str> = ed
            .completion
            .items
            .iter()
            .map(|i| i.label.as_str())
            .collect();
        assert!(
            labels.contains(&"alpha"),
            "prefix-matched words surface: {labels:?}"
        );
    }
}
