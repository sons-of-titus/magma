//! Completion popup engine — candidate collection and popup state.
//!
//! Candidates come from the language server first (kinds, signatures,
//! snippets), with buffer words and symbol indexes filling the popup while
//! the response is in flight.  The input layer calls these directly: while
//! the popup is visible it owns navigation keys, and typing a word
//! character refilters in place.
//!
//! Buffer-word collection lives in `completion_words`; the LSP bridge in
//! `completion_lsp`.

use crate::completion_lsp::request_lsp_completion;
use crate::completion_words::{
    candidate_words, cursor_line_and_col, is_word_char, word_prefix_len,
};
use crate::state::Editor;
use crate::state::completion::CompletionItem;

/// Minimum typed prefix for the automatic popup (ctrl-space triggers at 1).
const AUTO_TRIGGER_PREFIX: usize = 2;
pub(crate) const MAX_ITEMS: usize = 50;
/// Rows the popup overlay can show at once; the list scrolls in a window
/// around the selection.
pub const POPUP_ROWS: usize = 10;

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
                ..Default::default()
            });
        }
    }

    // The language server is the primary source — its items carry kinds,
    // signatures, and snippets.  Words/index symbols fill the popup
    // immediately while the response is in flight; server items lead once
    // they land (merge_lsp_items).  Per-keystroke requests supersede their
    // predecessor server-side ($/cancelRequest), so this stays cheap.
    let lsp_requested = request_lsp_completion(editor, buf_id, &prefix);
    if items.is_empty() && !lsp_requested {
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

    let mut stops: Vec<snippet::TabStop> = Vec::new();
    if let Some(view) = editor.view_of_buffer_mut(buf_id) {
        let pos = view.cursor_offset();
        let start = pos.saturating_sub(prefix.len().min(pos));
        if item.is_snippet {
            let (text, mut expanded) = snippet::expand_lsp(&item.insert);
            view.replace(start, pos, &text);
            // replace() collapses the cursor at the edit start; anchor the
            // tab stops in the buffer and park the cursor on the first one.
            for t in &mut expanded {
                t.offset += start;
            }
            if let Some(first) = expanded.first() {
                view.set_cursor(first.offset);
            }
            stops = expanded;
        } else if item.insert.starts_with(prefix.as_str()) {
            let suffix = item.insert[prefix.len()..].to_string();
            view.insert(pos, &suffix);
        } else {
            // The server's insert text does not extend the typed prefix
            // (e.g. a textEdit): replace the prefix wholesale.
            view.replace(start, pos, &item.insert);
            view.set_cursor(start + item.insert.len());
        }
    }
    if !stops.is_empty() {
        editor.snippet.active = true;
        editor.snippet.tabstops = stops;
        editor.snippet.tabstop_idx = 0;
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
}
