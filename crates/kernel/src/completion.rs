//! Completion popup engine — candidate collection and popup state.
//!
//! Candidates are buffer words plus the semantic engine's symbol index.
//! The input layer calls these directly: while the popup is visible it owns
//! navigation keys, and typing a word character refilters in place.
//!
//! Word collection is revision-cached per buffer: the prefix comes from the
//! cursor's line only (O(line), never a document copy), and the candidate
//! set is a `BTreeSet` queried by prefix range.  Buffers past the inline
//! limit are scanned on the background pool, chunk-by-chunk under short
//! buffer locks, and land via the revision-checked `BufferWords` event
//! (rework.md §3: no O(document) work on the keystroke path).

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::completion_lsp::request_lsp_completion;
use crate::state::Editor;
use crate::state::completion::{CompletionItem, WordCache};

/// Minimum typed prefix for the automatic popup (ctrl-space triggers at 1).
const AUTO_TRIGGER_PREFIX: usize = 2;
pub(crate) const MAX_ITEMS: usize = 50;
/// Rows the popup overlay can show at once; the list scrolls in a window
/// around the selection.
pub const POPUP_ROWS: usize = 10;
/// Buffers up to this size build their word set inline on the foreground;
/// larger ones go to the background pool.  The limit stays tiny: every
/// keystroke changes the revision, so anything scanned inline re-scans on
/// every keystroke — the source of the "typing slows down" degradation
/// when this limit covered 256 KB documents.
const INLINE_WORD_SCAN_LIMIT: usize = 16 * 1024;
/// Background scan chunk: one buffer lock per chunk, so the interactive
/// path never waits behind a whole-document scan.
const WORD_SCAN_CHUNK: usize = 1 << 20;

fn is_word_char(c: char) -> bool {
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
fn cursor_line_and_col(editor: &Editor) -> Option<(String, usize)> {
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
fn candidate_words(editor: &mut Editor, buf_id: usize) -> Option<Arc<BTreeSet<String>>> {
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

/// (Re)collect candidates for the word before the cursor.  With no word
/// under the cursor this is a no-op; with no candidates it dismisses.
pub fn trigger(editor: &mut Editor) {
    let Some(buf_id) = editor
        .view_tree
        .focused_window()
        .and_then(|wid| editor.view_tree.buffer(wid))
    else {
        return;
    };
    let Some((line_text, col)) = cursor_line_and_col(editor) else {
        return;
    };

    let before = &line_text[..col.min(line_text.len())];
    let word_start = before
        .rfind(|c: char| !is_word_char(c))
        .map(|i| i + 1)
        .unwrap_or(0);
    let prefix = before[word_start..].to_string();
    if prefix.is_empty() {
        return;
    }

    // Words from the buffer (cached / backgrounded), filtered by prefix
    // range — never a full-set copy.
    let mut items: Vec<CompletionItem> = Vec::new();
    if let Some(words) = candidate_words(editor, buf_id) {
        for w in words.range(prefix.clone()..) {
            if !w.starts_with(prefix.as_str()) {
                break;
            }
            if w != &prefix && items.len() < MAX_ITEMS {
                items.push(CompletionItem::word(w.clone()));
            }
            if items.len() >= MAX_ITEMS {
                break;
            }
        }
    }

    // Augment with symbols from the semantic engine's symbol index, then
    // the project index (workspace-wide, from the persistent store).
    for sym in editor.semantic.completions_for_prefix(&prefix) {
        if sym != prefix && !items.iter().any(|i| i.label == sym) && items.len() < MAX_ITEMS {
            items.push(CompletionItem::word(sym));
        }
    }
    for (_path, sym) in editor.index.symbols_with_prefix(&prefix) {
        if sym.name != prefix
            && !items.iter().any(|i| i.label == sym.name)
            && items.len() < MAX_ITEMS
        {
            items.push(CompletionItem {
                label: sym.name.clone(),
                insert: sym.name.clone(),
                kind: sym.kind.as_str().to_string(),
                detail: _path.clone(),
            });
        }
    }

    if items.is_empty() && !request_lsp_completion(editor, buf_id, &prefix) {
        dismiss(editor);
        return;
    }
    editor.completion.prefix = prefix;
    editor.completion.items = items;
    editor.completion.idx = 0;
    editor.completion.visible = true;
}

pub fn auto_trigger_if_word(editor: &mut Editor, key: &str) {
    let word_char = key.chars().count() == 1 && key.chars().next().is_some_and(is_word_char);
    if word_char && word_prefix_len(editor) >= AUTO_TRIGGER_PREFIX {
        trigger(editor);
    }
}

/// Display-cell width of a completion row — byte length miscounts
/// multibyte labels and misplaces the popup's right edge.
pub(crate) fn row_cells(editor: &Editor, label: &str, kind: &str) -> usize {
    let widths = &editor.font_config.glyph_widths;
    let text = if kind.is_empty() {
        label.to_string()
    } else {
        format!("{label} {kind}")
    };
    text.chars()
        .map(|c| crate::render::frame::cell_width(c, widths))
        .sum()
}

pub fn next(editor: &mut Editor) {
    if editor.completion.visible && !editor.completion.items.is_empty() {
        editor.completion.idx = (editor.completion.idx + 1) % editor.completion.items.len();
    }
}

pub fn prev(editor: &mut Editor) {
    if editor.completion.visible && !editor.completion.items.is_empty() {
        editor.completion.idx = if editor.completion.idx == 0 {
            editor.completion.items.len() - 1
        } else {
            editor.completion.idx - 1
        };
    }
}

pub fn accept(editor: &mut Editor) {
    if !editor.completion.visible || editor.completion.items.is_empty() {
        return;
    }
    let Some(buf_id) = editor
        .view_tree
        .focused_window()
        .and_then(|wid| editor.view_tree.buffer(wid))
    else {
        return;
    };
    let item = editor.completion.items[editor.completion.idx].clone();
    let prefix = editor.completion.prefix.clone();

    if let Some(view) = editor.view_of_buffer_mut(buf_id) {
        let pos = view.cursor_offset();
        let start = pos.saturating_sub(prefix.len().min(pos));
        if item.insert.starts_with(prefix.as_str()) {
            let suffix = item.insert[prefix.len()..].to_string();
            view.insert(pos, &suffix);
        } else {
            // The server's insert text does not extend the typed prefix
            // (e.g. a snippet): replace the prefix wholesale.
            view.replace(start, pos, &item.insert);
        }
    }
    dismiss(editor);
}

/// The visible scrolling window of the popup list: the selection always
/// stays in view as `idx` moves, and the window clamps at both ends.
/// Returns the index of the first visible item plus the slice to render.
pub fn popup_window(items: &[CompletionItem], idx: usize) -> (usize, &[CompletionItem]) {
    let max_start = items.len().saturating_sub(POPUP_ROWS);
    let start = idx.checked_sub(POPUP_ROWS - 1).unwrap_or(0).min(max_start);
    (start, &items[start..(start + POPUP_ROWS).min(items.len())])
}

pub fn dismiss(editor: &mut Editor) {
    editor.completion.visible = false;
    editor.completion.items.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::builtin;
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
        trigger(&mut ed);
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
    fn popup_window_keeps_the_selection_visible_past_the_first_page() {
        let items: Vec<CompletionItem> = (0..25)
            .map(|i| CompletionItem::word(format!("sym{i}")))
            .collect();
        // Inside the first page the window sits at the top.
        let (start, window) = popup_window(&items, 3);
        assert_eq!(start, 0);
        assert_eq!(window.len(), 10);
        // Past the first page the window scrolls with the selection.
        let (start, window) = popup_window(&items, 12);
        assert_eq!(start, 3);
        assert_eq!(window[8].label, "sym11");
        assert_eq!(window.last().unwrap().label, "sym12");
        // The tail clamps: the window is the last POPUP_ROWS entries.
        let (start, window) = popup_window(&items, 24);
        assert_eq!(start, 15);
        assert_eq!(window.last().unwrap().label, "sym24");
        // Wrap to the top recenters at zero.
        assert_eq!(popup_window(&items, 0).0, 0);
    }

    #[test]
    fn popup_window_handles_short_lists() {
        let items: Vec<CompletionItem> = (0..3)
            .map(|i| CompletionItem::word(format!("s{i}")))
            .collect();
        let (start, window) = popup_window(&items, 2);
        assert_eq!(start, 0);
        assert_eq!(window.len(), 3);
        assert_eq!(popup_window(&[], 0), (0, &[][..]));
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
        trigger(&mut ed);
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
