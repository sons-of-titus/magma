//! Language-server commands.
//!
//! These live in Rust rather than Janet because they are invoked from
//! other Janet commands (goto-definition's fallback): a Janet command
//! dispatched from inside a Janet command nests fibers, which Janet does
//! not support — the inner dispatch silently fails.

use serde_json::json;

use crate::state::Editor;

pub(super) fn register(editor: &mut Editor) {
    editor.commands.register_fn(
        "lsp-go-to-definition",
        "Ask the attached language server for the definition at the cursor",
        vec![],
        |editor, _args| {
            let buf_key = editor
                .view_tree
                .focused_window()
                .and_then(|w| editor.view_tree.buffer(w));
            let Some(buf_key) = buf_key else {
                return Ok(());
            };
            let Some(lang) = editor.lsp.language_for_buffer(buf_key) else {
                return Ok(());
            };
            let Some(server) = editor.lsp.get(&lang).cloned() else {
                return Ok(());
            };
            let Some(arc) = editor.buffers.get(buf_key) else {
                return Ok(());
            };
            let (params, revision) = {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                let Some(path) = buf.path.clone() else {
                    return Ok(());
                };
                let cursor = editor
                    .view_of_buffer(buf_key)
                    .map(|v| v.cursor.offset)
                    .unwrap_or(0);
                let (line, character) = buf.lsp_position(cursor.min(buf.len()));
                (
                    json!({
                        "textDocument": {
                            "uri": crate::semantic::lsp_proto::path_to_uri(&path)
                        },
                        "position": { "line": line, "character": character },
                    }),
                    buf.revision(),
                )
            };
            server.request("textDocument/definition", params, buf_key, revision);
            Ok(())
        },
    );
}
