//! Native `textDocument/inlayHint` lifecycle: request, parse, decorate.
//!
//! Split from nothing — this used to live in `builtins/lsp_inlay.janet`,
//! which subscribed to raw responses but had no requester, so the feature
//! never ran.  Hints render through the standard `InlineText` decoration
//! pass, one layer ("lsp-inlay"), refreshed per buffer revision.

use crate::state::Editor;

pub const LAYER: &str = "lsp-inlay";
const MAX_HINTS: usize = 400;

#[derive(Default)]
pub struct InlayState {
    /// Buffer → revision the current request was made against.  A newer
    /// revision supersedes in-flight work; an equal one dedupes re-requests.
    pub requested_revision: std::collections::HashMap<usize, u64>,
}

/// Ask the attached server for hints covering the whole document.  No-op
/// without an LSP attach or a file path, or when that revision is already
/// in flight.
pub fn request(ed: &mut Editor, buf_id: usize) {
    if ed.options.get("inlay-hints").map(|v| v.as_str()) == Some("false") {
        return;
    }
    let Some(lang) = ed.lsp.language_for_buffer(buf_id) else {
        return;
    };
    let Some(server) = ed.lsp.get(&lang) else {
        return;
    };
    let Some(arc) = ed.buffers.get(buf_id) else {
        return;
    };
    let (uri, revision, line_count, last_line_len) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = buf.path.clone() else {
            return;
        };
        let line_count = buf.line_count();
        let last_line_len = if line_count == 0 {
            0
        } else {
            let start = buf.line_start_offset(line_count - 1).unwrap_or(buf.len());
            buf.slice(start, buf.len())
                .trim_end_matches('\n')
                .chars()
                .count()
        };
        (
            crate::semantic::lsp_proto::path_to_uri(&path),
            buf.revision(),
            line_count,
            last_line_len,
        )
    };
    if ed.inlay.requested_revision.get(&buf_id) == Some(&revision) {
        return;
    }
    ed.inlay.requested_revision.insert(buf_id, revision);
    let params = serde_json::json!({
        "textDocument": { "uri": uri },
        "range": {
            "start": { "line": 0, "character": 0 },
            // Lines are 0-based: line_count is one past the last line, and
            // servers reject a range that ends beyond the document.
            "end": { "line": line_count.saturating_sub(1), "character": last_line_len },
        },
    });
    server.request("textDocument/inlayHint", params, buf_id, revision);
}

/// Parse an inlayHint result and install the buffer's hint decorations.
/// Results for superseded revisions are dropped; labels arrive as plain
/// strings or `{value}` objects.  LSP UTF-16 columns are treated as display
/// cells — the same approximation diagnostics make.
pub fn apply(ed: &mut Editor, buf_id: usize, revision: u64, result: &str) {
    let current = ed
        .buffers
        .get(buf_id)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision())
        .unwrap_or(0);
    if current > revision {
        return;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(result) else {
        return;
    };
    let Some(arr) = v.as_array() else {
        return;
    };
    let mut hints: Vec<(usize, usize, String)> = Vec::new();
    for h in arr.iter().take(MAX_HINTS) {
        let line = h["position"]["line"].as_u64().unwrap_or(0) as usize;
        let col = h["position"]["character"].as_u64().unwrap_or(0) as usize;
        let mut label = match &h["label"] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(parts) => parts
                .iter()
                .map(|p| match p {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Object(o) => o
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    _ => String::new(),
                })
                .collect::<String>(),
            serde_json::Value::Object(o) => o
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            _ => continue,
        };
        if h["paddingLeft"].as_bool() == Some(true) {
            label = format!(" {label}");
        }
        if h["paddingRight"].as_bool() == Some(true) {
            label = format!("{label} ");
        }
        if label.trim().is_empty() {
            continue;
        }
        hints.push((line, col, label));
    }
    ed.for_each_view_of_buffer_mut(buf_id, |view| {
        view.decor_clear_layer(LAYER);
        for (line, col, label) in &hints {
            view.decor_set_inline(LAYER, *line, *col, label.clone(), "comment-face".to_string());
        }
    });
    ed.frame_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::builtin;
    use crate::storage::disk::DiskFileSystem;

    fn editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        ed
    }

    #[test]
    fn apply_installs_inline_hints_and_refreshes_dedup() {
        let mut ed = editor();
        let key = ed.create_buffer_from_str("a.rs", "let x = 1;\nlet y = 2;\n");
        let revision = ed
            .buffers
            .get(key)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision())
            .unwrap_or(0);
        let result = r#"[
            {"position": {"line": 0, "character": 4}, "label": ": i32"},
            {"position": {"line": 1, "character": 4}, "label": {"value": ": f64"}}
        ]"#;
        apply(&mut ed, key, revision, result);
        let view = ed.view_of_buffer(key).unwrap();
        assert_eq!(view.decor_count_layer(LAYER), 2);
        let decors = view.decoration_layers.get(LAYER).unwrap();
        assert!(decors.iter().any(
            |d| matches!(d, crate::text_engine::Decoration::InlineText { line: 1, text, .. } if text == ": f64")
        ));
        assert!(ed.frame_dirty);
    }

    #[test]
    fn stale_revisions_are_dropped() {
        let mut ed = editor();
        let key = ed.create_buffer_from_str("a.rs", "let x = 1;\n");
        // The buffer's revision moved past the response's revision: the
        // response claims revision 0, the buffer is at its current (higher)
        // revision — wait for the fresh publish instead.
        let result = r#"[{"position": {"line": 0, "character": 0}, "label": "stale"}]"#;
        // Edit the buffer so its revision moves past the in-flight response.
        let stale_revision = ed
            .buffers
            .get(key)
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision())
            .unwrap_or(0);
        ed.view_of_buffer_mut(key).unwrap().insert(0, "x");
        apply(&mut ed, key, stale_revision, result);
        assert_eq!(
            ed.view_of_buffer(key)
                .map(|v| v.decor_count_layer(LAYER))
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn request_without_lsp_or_path_is_a_quiet_noop() {
        let mut ed = editor();
        let key = ed.create_buffer_from_str("scratch", "text\n");
        request(&mut ed, key);
        assert!(ed.inlay.requested_revision.is_empty());
        request(&mut ed, 99);
    }
}
