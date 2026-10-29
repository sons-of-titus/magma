//! Completion's language-server bridge: request lifecycle, response
//! parsing, and merging into the popup item list.
//!
//! Split from `completion.rs` (engine: candidates, word cache, popup state)
//! to keep each file on one concern.

use crate::state::Editor;
use crate::state::completion::CompletionItem;

use crate::completion::MAX_ITEMS;

/// Ask the attached language server for completions at the cursor.  The
/// response event carries the buffer + revision; the merge on arrival
/// discards results for superseded keystrokes.
pub(super) fn request_lsp_completion(editor: &Editor, buf_id: usize, prefix: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }
    let Some(lang) = editor.lsp.language_for_buffer(buf_id) else {
        return false;
    };
    let Some(server) = editor.lsp.get(&lang) else {
        return false;
    };
    let Some(arc) = editor.buffers.get(buf_id) else {
        return false;
    };
    let (uri, position, revision) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let uri = match buf.path.as_deref() {
            Some(p) => format!("file://{p}"),
            None => return false,
        };
        let cursor = editor
            .view_of_buffer(buf_id)
            .map(|v| v.cursor.offset)
            .unwrap_or(0);
        (uri, buf.lsp_position(cursor.min(buf.len())), buf.revision())
    };
    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": position.0, "character": position.1 },
    });
    server.request("textDocument/completion", params, buf_id, revision);
    true
}

/// LSP `CompletionItemKind` → popup tag.
fn lsp_kind_name(kind: u64) -> &'static str {
    match kind {
        1 => "text",
        2 => "method",
        3 => "fn",
        4 => "ctor",
        5 => "field",
        6 => "var",
        7 => "class",
        8 => "iface",
        9 => "mod",
        10 => "prop",
        13 => "enum",
        14 => "kw",
        15 => "snip",
        20 => "member",
        21 => "const",
        22 => "struct",
        25 => "tparam",
        _ => "",
    }
}

/// Parse a `textDocument/completion` result (array or CompletionList)
/// into popup items, keeping the server's relevance order.
pub fn parse_lsp_completion_items(json: &str) -> Vec<CompletionItem> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let arr = v
        .as_array()
        .cloned()
        .or_else(|| v.get("items").and_then(|i| i.as_array()).cloned())
        .unwrap_or_default();
    arr.iter().filter_map(parse_lsp_item).collect()
}

fn parse_lsp_item(item: &serde_json::Value) -> Option<CompletionItem> {
    let label = item.get("label")?.as_str()?.to_string();
    // textEdit wins over insertText: servers use it whenever the insert
    // differs from what the user typed (method chains, auto-import).
    let insert = item
        .pointer("/textEdit/newText")
        .and_then(|t| t.as_str())
        .or_else(|| item.get("insertText").and_then(|t| t.as_str()))
        .unwrap_or(&label)
        .to_string();
    let mut detail = item
        .get("detail")
        .and_then(|d| d.as_str())
        .unwrap_or_default()
        .to_string();
    if detail.is_empty() {
        detail = item
            .pointer("/labelDetails/description")
            .and_then(|d| d.as_str())
            .unwrap_or_default()
            .to_string();
    }
    Some(CompletionItem {
        label,
        kind: item
            .get("kind")
            .and_then(|k| k.as_u64())
            .map(lsp_kind_name)
            .unwrap_or_default()
            .to_string(),
        detail,
        insert,
        filter: item
            .get("filterText")
            .and_then(|f| f.as_str())
            .unwrap_or_default()
            .to_string(),
        docs: item
            .get("documentation")
            .map(parse_documentation)
            .unwrap_or_default(),
        is_snippet: item.get("insertTextFormat").and_then(|f| f.as_u64()) == Some(2),
    })
}

/// Flatten `documentation` — a plain string or a `{content, value}` markup
/// object — to display text.
fn parse_documentation(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(o) => o
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

/// Fold server completions (already prefix-filtered by the caller) into the
/// visible popup: server items keep their order and lead; word/index items
/// that the server also returned are dropped as duplicates.
pub fn merge_lsp_items(editor: &mut Editor, items: Vec<CompletionItem>) {
    if !editor.completion.visible || items.is_empty() {
        return;
    }
    let prefix = editor.completion.prefix.clone();
    let selected = editor
        .completion
        .items
        .get(editor.completion.idx)
        .map(|i| i.label.clone());
    let prefix_matches = |i: &CompletionItem| {
        let f = if i.filter.is_empty() { &i.label } else { &i.filter };
        // Servers match case-insensitively; a case-sensitive filter would
        // drop their items mid-word ("DO" vs "door").
        f.to_lowercase().starts_with(&prefix.to_lowercase())
    };
    let mut merged: Vec<CompletionItem> = items.into_iter().filter(prefix_matches).collect();
    for existing in editor.completion.items.drain(..) {
        if !merged.iter().any(|i| i.label == existing.label) {
            merged.push(existing);
        }
        if merged.len() >= MAX_ITEMS {
            break;
        }
    }
    merged.truncate(MAX_ITEMS);
    // Keep the user's navigation when the highlighted item survives the
    // merge; a server response must not yank the selection back to the top.
    let idx = selected
        .and_then(|label| merged.iter().position(|i| i.label == label))
        .unwrap_or(0);
    editor.completion.items = merged;
    editor.completion.idx = idx;
}
